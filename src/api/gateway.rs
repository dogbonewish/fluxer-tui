use crate::api::types::{
    GatewayHelloPayload, GatewayIdentifyPayload, GatewayIdentifyProperties, GatewayPayload,
    GatewayResumePayload, ReadyEvent,
};
use crate::app::GatewayStatus;
use crate::events::AppEvent;
use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{Duration, MissedTickBehavior, interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

const OP_DISPATCH: u8 = 0;
const OP_HEARTBEAT: u8 = 1;
const OP_IDENTIFY: u8 = 2;
const OP_RESUME: u8 = 6;
const OP_RECONNECT: u8 = 7;
const OP_INVALID_SESSION: u8 = 9;
const OP_HELLO: u8 = 10;
const OP_HEARTBEAT_ACK: u8 = 11;
const OP_LAZY_REQUEST: u8 = 14;

#[derive(Debug, Clone)]
pub enum GatewayCommand {
    /// User-account sessions: subscribe so MESSAGE_CREATE, TYPING_START, etc. are delivered (see fluxer session_passive).
    LazySubscribeGuild { guild_id: String },
    Shutdown,
}

// close codes the server sends that mean "stop trying"
// - dogbone
fn is_fatal_close_code(code: u16) -> bool {
    matches!(code,
        4004 // AUTHENTICATION_FAILED
        | 4010 // INVALID_SHARD
        | 4011 // SHARDING_REQUIRED
        | 4012 // INVALID_API_VERSION
    )
}

/// what the server told us when it closed the connection
#[derive(Debug, Clone)]
struct GatewayClose {
    code: u16,
    reason: String,
}

pub async fn run_gateway(
    endpoint: String,
    token: String,
    initial_guild_id: Option<String>,
    mut command_rx: UnboundedReceiver<GatewayCommand>,
    event_tx: UnboundedSender<AppEvent>,
) -> Result<()> {
    let mut resume_session_id: Option<String> = None;
    let mut last_sequence: u64 = 0;

    loop {
        let status = if resume_session_id.is_some() {
            GatewayStatus::Reconnecting
        } else {
            GatewayStatus::Connecting
        };
        let _ = event_tx.send(AppEvent::GatewayStatus(status));

        let connection = connect_async(endpoint.as_str()).await;
        let (stream, _) = match connection {
            Ok(ok) => ok,
            Err(err) => {
                let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Disconnected));
                let _ = event_tx.send(AppEvent::ApiError(format!("Gateway connect failed: {err}")));
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let outcome = run_connection(
            stream,
            &token,
            initial_guild_id.clone(),
            &mut resume_session_id,
            &mut last_sequence,
            &mut command_rx,
            &event_tx,
        )
        .await;

        match outcome {
            Ok(ConnectionOutcome::Shutdown) => {
                let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Disconnected));
                break;
            }
            Ok(ConnectionOutcome::Fatal(reason)) => {
                let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Disconnected));
                let _ = event_tx.send(AppEvent::ApiError(format!("Gateway fatal: {reason}")));
                break;
            }
            Ok(ConnectionOutcome::Reconnect { clear_resume }) => {
                if clear_resume {
                    resume_session_id = None;
                    last_sequence = 0;
                }
                let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Disconnected));
                sleep(Duration::from_secs(2)).await;
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Disconnected));
                let _ = event_tx.send(AppEvent::ApiError(format!("Gateway error: {err}")));
                sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Ok(())
}

enum ConnectionOutcome {
    Reconnect { clear_resume: bool },
    Shutdown,
    Fatal(String),
}

async fn run_connection(
    stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    token: &str,
    initial_guild_id: Option<String>,
    resume_session_id: &mut Option<String>,
    last_sequence: &mut u64,
    command_rx: &mut UnboundedReceiver<GatewayCommand>,
    event_tx: &UnboundedSender<AppEvent>,
) -> Result<ConnectionOutcome> {
    let (mut write, mut read) = stream.split();
    let hello = wait_for_hello(&mut read).await?;

    if let Some(session_id) = resume_session_id.as_ref() {
        let payload = GatewayResumePayload {
            token: token.to_string(),
            session_id: session_id.clone(),
            seq: *last_sequence,
        };
        send_payload(&mut write, OP_RESUME, &payload).await?;
    } else {
        let payload = GatewayIdentifyPayload {
            token: token.to_string(),
            properties: GatewayIdentifyProperties {
                os: std::env::consts::OS.to_string(),
                browser: "fluxer-tui".to_string(),
                device: "fluxer-tui".to_string(),
            },
            flags: 0,
            initial_guild_id: initial_guild_id.filter(|id| !id.trim().is_empty()),
        };
        send_payload(&mut write, OP_IDENTIFY, &payload).await?;
    }

    let _ = event_tx.send(AppEvent::GatewayStatus(GatewayStatus::Connected));

    let heartbeat_ms = hello.heartbeat_interval.max(1_000);
    let mut heartbeat = interval(Duration::from_millis(heartbeat_ms));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                send_payload(&mut write, OP_HEARTBEAT, &json!(*last_sequence)).await?;
            }
            command = command_rx.recv() => {
                match command {
                    Some(GatewayCommand::LazySubscribeGuild { guild_id }) => {
                        if !guild_id.is_empty() {
                            let d = json!({
                                "subscriptions": {
                                    guild_id: { "active": true, "sync": true }
                                }
                            });
                            if let Err(e) = send_op_json(&mut write, OP_LAZY_REQUEST, d).await {
                                let _ = event_tx.send(AppEvent::ApiError(format!(
                                    "lazy subscribe failed: {e}"
                                )));
                            }
                        }
                    }
                    Some(GatewayCommand::Shutdown) | None => {
                        let _ = write.close().await;
                        return Ok(ConnectionOutcome::Shutdown);
                    }
                }
            }
            message = read.next() => {
                let Some(message) = message else {
                    return Ok(ConnectionOutcome::Reconnect { clear_resume: false });
                };
                let message = message.context("gateway stream read failed")?;

                // handle close frames before trying to extract text
                if let Message::Close(frame) = &message {
                    let close = extract_close(frame);
                    let _ = event_tx.send(AppEvent::ApiError(
                        format!("Gateway closed: {} ({})", close.reason, close.code)
                    ));
                    if is_fatal_close_code(close.code) {
                        return Ok(ConnectionOutcome::Fatal(
                            format!("{} ({})", close.reason, close.code)
                        ));
                    }
                    // 4007 INVALID_SEQ means clear resume state
                    let clear = close.code == 4007;
                    return Ok(ConnectionOutcome::Reconnect { clear_resume: clear });
                }

                let Some(text) = websocket_text(&message)? else {
                    continue;
                };

                let payload: GatewayPayload = serde_json::from_str(&text)
                    .with_context(|| format!("failed to parse gateway payload: {text}"))?;

                if let Some(sequence) = payload.s {
                    *last_sequence = sequence;
                }

                match payload.op {
                    OP_DISPATCH => {
                        if let Some(kind) = payload.t.clone() {
                            if kind == "READY"
                                && let Ok(ready) = serde_json::from_value::<ReadyEvent>(payload.d.clone()) {
                                    *resume_session_id = Some(ready.session_id);
                                }
                            let _ = event_tx.send(AppEvent::Dispatch {
                                kind,
                                payload: payload.d,
                            });
                        }
                    }
                    OP_HEARTBEAT => {
                        send_payload(&mut write, OP_HEARTBEAT, &json!(*last_sequence)).await?;
                    }
                    OP_HEARTBEAT_ACK => {}
                    OP_RECONNECT => {
                        return Ok(ConnectionOutcome::Reconnect { clear_resume: false });
                    }
                    OP_INVALID_SESSION => {
                        let resumable = payload.d.as_bool().unwrap_or(false);
                        if !resumable {
                            *resume_session_id = None;
                            *last_sequence = 0;
                        }
                        return Ok(ConnectionOutcome::Reconnect { clear_resume: !resumable });
                    }
                    OP_HELLO => {}
                    other => {
                        let _ = event_tx.send(AppEvent::ApiError(format!(
                            "Unhandled gateway opcode {other}"
                        )));
                    }
                }
            }
        }
    }
}

async fn wait_for_hello(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<GatewayHelloPayload> {
    loop {
        let message = read
            .next()
            .await
            .ok_or_else(|| anyhow!("gateway closed before HELLO"))?
            .context("failed reading gateway HELLO")?;

        // if server closes before HELLO, surface the actual reason
        if let Message::Close(frame) = &message {
            let close = extract_close(frame);
            return Err(anyhow!("server closed before HELLO: {} (code {})", close.reason, close.code));
        }

        let Some(text) = websocket_text(&message)? else {
            continue;
        };
        let payload: GatewayPayload =
            serde_json::from_str(&text).context("failed to decode HELLO payload")?;
        if payload.op == OP_HELLO {
            return serde_json::from_value(payload.d).context("failed to decode HELLO body");
        }
    }
}

fn extract_close(frame: &Option<CloseFrame>) -> GatewayClose {
    match frame {
        Some(f) => GatewayClose {
            code: f.code.into(),
            reason: f.reason.to_string(),
        },
        None => GatewayClose {
            code: 1000,
            reason: "no reason given".to_string(),
        },
    }
}

fn websocket_text(message: &Message) -> Result<Option<String>> {
    let text = match message {
        Message::Text(text) => Some(text.to_string()),
        Message::Binary(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        Message::Ping(_) | Message::Pong(_) => None,
        Message::Close(_) => None,
        Message::Frame(_) => None,
    };
    Ok(text)
}

async fn send_payload<T>(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    op: u8,
    data: &T,
) -> Result<()>
where
    T: Serialize,
{
    let payload = serde_json::to_string(&json!({
        "op": op,
        "d": data,
    }))
    .context("failed to encode gateway payload")?;
    write
        .send(Message::Text(payload.into()))
        .await
        .context("failed to send gateway payload")
}

type WsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

async fn send_op_json(write: &mut WsWrite, op: u8, d: Value) -> Result<()> {
    let payload = serde_json::to_string(&json!({ "op": op, "d": d }))
        .context("failed to encode gateway payload")?;
    write
        .send(Message::Text(payload.into()))
        .await
        .context("failed to send gateway payload")
}
