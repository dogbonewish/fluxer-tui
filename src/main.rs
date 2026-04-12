mod api;
mod app;
mod auth;
mod config;
mod events;
mod permissions;
mod ui;

use crate::api::client::FluxerHttpClient;
use crate::api::gateway::{GatewayCommand, run_gateway};
use crate::api::types::{CreateMessageRequest, MessageQuery, MessageReferenceRequest};
use crate::app::{App, Focus, GatewayStatus, ServerSelection};
use crate::auth::ensure_auth;
use crate::config::{DEFAULT_API_BASE_URL, default_config_path, load_config, save_config};
use crate::events::{AppEvent, apply_event};
use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, terminal};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::time::{Duration, interval};

#[derive(Debug, Parser)]
#[command(name = "fluxer-tui")]
#[command(about = "A ratatui-based Fluxer terminal client")]
struct Args {
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    api_base_url: Option<String>,
    #[arg(long, help = "Clear saved token and exit")]
    logout: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config_path = args.config.unwrap_or(default_config_path()?);
    let mut config = load_config(&config_path)?;

    if args.logout {
        config.token = None;
        save_config(&config_path, &config)?;
        eprintln!("Logged out. Token cleared.");
        return Ok(());
    }

    if let Some(api_base_url) = args.api_base_url {
        config.api_base_url = api_base_url;
    }
    if config.api_base_url.trim().is_empty() {
        config.api_base_url = DEFAULT_API_BASE_URL.to_string();
    }

    let base_client = FluxerHttpClient::new(config.api_base_url.clone())?;
    let discovery = base_client.discover().await.unwrap_or_default();

    let webapp_url = if discovery.endpoints.webapp.is_empty() {
        "https://fluxer.app".to_string()
    } else {
        discovery.endpoints.webapp.trim_end_matches('/').to_string()
    };

    let auth = ensure_auth(&base_client, &mut config, args.token, &webapp_url).await?;
    save_config(&config_path, &config)?;

    let authed_client = base_client.with_token(auth.token.clone());
    let settings = authed_client.current_user_settings().await.ok();
    let guilds = authed_client.guilds().await.unwrap_or_default();
    let private_channels = authed_client.private_channels().await.unwrap_or_default();

    let selected_server = resolve_initial_server(&config, &guilds);
    let initial_guild_id = match &selected_server {
        ServerSelection::Guild(id) => Some(id.clone()),
        ServerSelection::DirectMessages => None,
    };

    let mut app = App::new(
        discovery.clone(),
        auth.me,
        settings,
        guilds,
        private_channels,
        selected_server,
        config.last_channel_id.clone(),
    );
    let (event_tx, mut event_rx) = unbounded_channel::<AppEvent>();
    let (gateway_cmd_tx, gateway_cmd_rx) = unbounded_channel::<GatewayCommand>();
    let gateway_url = if !discovery.endpoints.gateway.is_empty() {
        discovery.endpoints.gateway.clone()
    } else {
        authed_client.gateway_info().await.unwrap_or_default().url
    };

    let gateway_url = format!("{}/?v=1&encoding=json", gateway_url.trim_end_matches('/'));

    tokio::spawn(run_gateway(
        gateway_url,
        auth.token.clone(),
        initial_guild_id,
        gateway_cmd_rx,
        event_tx.clone(),
    ));

    schedule_needed_fetches(&mut app, authed_client.clone(), event_tx.clone());

    let mut terminal = init_terminal()?;
    let _guard = TerminalGuard;
    let mut reader = EventStream::new();
    let mut tick = interval(Duration::from_millis(100));

    loop {
        if let Err(e) = terminal.draw(|frame| ui::draw(frame, &app)) {
            eprintln!("fluxer-tui: terminal draw failed: {e}");
            break;
        }

        tokio::select! {
            maybe_event = reader.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_event
                    && key.kind == KeyEventKind::Press {
                        handle_key_event(&mut app, key, &authed_client, &event_tx, &gateway_cmd_tx);
                        schedule_needed_fetches(&mut app, authed_client.clone(), event_tx.clone());
                        ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
                    }
            }
            Some(event) = event_rx.recv() => {
                let effects = apply_event(&mut app, event);
                if let Some(token) = effects.persist_token {
                    config.token = Some(token);
                    save_config(&config_path, &config)?;
                }
                schedule_needed_fetches(&mut app, authed_client.clone(), event_tx.clone());
                ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
            }
            _ = tick.tick() => {
                schedule_needed_fetches(&mut app, authed_client.clone(), event_tx.clone());
                ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
            }
        }

        if app.should_quit || app.should_logout {
            break;
        }
    }

    if app.should_logout {
        config.token = None;
    }
    config.last_server_id = Some(app.selected_server.id());
    config.last_channel_id = app.selected_channel_id.clone();
    save_config(&config_path, &config)?;

    let _ = gateway_cmd_tx.send(GatewayCommand::Shutdown);
    Ok(())
}

fn ensure_lazy_guild_subscription(app: &mut App, gateway_cmd_tx: &UnboundedSender<GatewayCommand>) {
    if app.gateway_status != GatewayStatus::Connected {
        return;
    }
    match &app.selected_server {
        ServerSelection::DirectMessages => {
            app.gateway_lazy_guild_id = None;
        }
        ServerSelection::Guild(guild_id) => {
            if guild_id.is_empty() {
                return;
            }
            if app.gateway_lazy_guild_id.as_deref() == Some(guild_id.as_str()) {
                return;
            }
            let gid = guild_id.clone();
            let _ = gateway_cmd_tx.send(GatewayCommand::LazySubscribeGuild {
                guild_id: gid.clone(),
            });
            app.gateway_lazy_guild_id = Some(gid);
        }
    }
}

fn resolve_initial_server(
    config: &config::AppConfig,
    guilds: &[crate::api::types::GuildResponse],
) -> ServerSelection {
    match config.last_server_id.as_deref() {
        Some("@me") => ServerSelection::DirectMessages,
        Some(id) if guilds.iter().any(|guild| guild.id == id) => {
            ServerSelection::Guild(id.to_string())
        }
        _ => guilds
            .first()
            .map(|guild| ServerSelection::Guild(guild.id.clone()))
            .unwrap_or(ServerSelection::DirectMessages),
    }
}

fn handle_key_event(
    app: &mut App,
    key: KeyEvent,
    client: &FluxerHttpClient,
    event_tx: &UnboundedSender<AppEvent>,
    _gateway_cmd_tx: &UnboundedSender<GatewayCommand>,
) {
    if app.focus == Focus::Input {
        if app.mention_autocomplete.is_some() {
            match key.code {
                KeyCode::Esc => {
                    app.dismiss_mention_autocomplete();
                }
                KeyCode::Up => {
                    app.autocomplete_mention_prev();
                }
                KeyCode::Down => {
                    app.autocomplete_mention_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    app.insert_selected_mention();
                }
                KeyCode::Backspace => {
                    app.input.pop();
                    app.update_mention_filter();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.input.push(ch);
                    app.update_mention_filter();
                }
                _ => {}
            }
            return;
        }

        if app.emoji_autocomplete.is_some() {
            match key.code {
                KeyCode::Esc => {
                    app.dismiss_emoji_autocomplete();
                }
                KeyCode::Up => {
                    app.autocomplete_emoji_prev();
                }
                KeyCode::Down => {
                    app.autocomplete_emoji_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    app.insert_selected_emoji();
                }
                KeyCode::Backspace => {
                    app.input.pop();
                    app.update_emoji_filter();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.input.push(ch);
                    app.update_emoji_filter();
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc => {
                if app.reply_to.is_some() {
                    app.cancel_reply();
                } else {
                    app.focus = Focus::Channels;
                }
            }
            KeyCode::Enter => {
                if app.active_channel_is_text()
                    && app.can_send_in_active_channel()
                    && !app.input.trim().is_empty()
                {
                    let channel_id = match app.active_channel_id() {
                        Some(channel_id) => channel_id,
                        None => return,
                    };
                    let content = std::mem::take(&mut app.input);
                    let reply = app.reply_to.take();
                    app.message_scroll_from_bottom = 0;
                    spawn_send_message(
                        client.clone(),
                        event_tx.clone(),
                        channel_id,
                        content,
                        reply,
                    );
                }
            }
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.input.clear();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.input.push(ch);
                if ch == ':' {
                    app.start_emoji_autocomplete();
                } else if ch == '@' {
                    let member_fetch_pending = schedule_guild_members_fetch_for_mentions(
                        app,
                        client.clone(),
                        event_tx.clone(),
                    );
                    app.start_mention_autocomplete();
                    if member_fetch_pending && app.mention_autocomplete.is_none() {
                        app.set_status("Loading members for @mentions…");
                    }
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_logout = true;
            app.should_quit = true;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.focus = app.focus.next(),
        KeyCode::BackTab => app.focus = app.focus.previous(),
        KeyCode::Left | KeyCode::Char('h') => app.focus = app.focus.previous(),
        KeyCode::Right | KeyCode::Char('l') => app.focus = app.focus.next(),
        KeyCode::Char('i') => {
            if app.active_channel_is_text() && app.can_send_in_active_channel() {
                app.focus = Focus::Input;
            }
        }
        KeyCode::Enter => {
            if app.active_channel_is_link() {
                if let Some(channel) = app.active_channel()
                    && let Some(url) = &channel.url
                {
                    open_url_background(url);
                }
            } else if app.active_channel_is_text() && app.can_send_in_active_channel() {
                app.focus = Focus::Input;
            }
        }
        KeyCode::Esc => {
            app.selected_message_index = None;
            app.focus = Focus::Channels;
        }
        KeyCode::Up | KeyCode::Char('k') => match app.focus {
            Focus::Servers => {
                app.move_server(-1);
            }
            Focus::Channels => {
                let old_ch = app.selected_channel_id.clone();
                app.move_channel(-1);
                if old_ch != app.selected_channel_id {
                    ack_current_channel(app, client, event_tx);
                }
            }
            Focus::Messages => {
                if app.selected_message_index.is_some() {
                    app.move_selected_message(-1);
                } else {
                    app.scroll_messages_up(3);
                }
            }
            Focus::Input => {}
        },
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            Focus::Servers => {
                app.move_server(1);
            }
            Focus::Channels => {
                let old_ch = app.selected_channel_id.clone();
                app.move_channel(1);
                if old_ch != app.selected_channel_id {
                    ack_current_channel(app, client, event_tx);
                }
            }
            Focus::Messages => {
                if app.selected_message_index.is_some() {
                    app.move_selected_message(1);
                } else {
                    app.scroll_messages_down(3);
                }
            }
            Focus::Input => {}
        },
        KeyCode::PageUp => app.scroll_messages_up(18),
        KeyCode::PageDown => app.scroll_messages_down(18),
        // s = select mode
        KeyCode::Char('s') if app.focus == Focus::Messages => {
            let count = app.active_messages().len();
            if count > 0 {
                app.selected_message_index = Some(count.saturating_sub(1));
            }
        }
        // r = reply mode
        KeyCode::Char('r')
            if app.focus == Focus::Messages && app.selected_message_index.is_some() =>
        {
            app.start_reply();
        }
        // R = refresh
        KeyCode::Char('R') => {
            if let Some(channel_id) = app.active_channel_id() {
                app.loading_messages.remove(&channel_id);
                app.messages.remove(&channel_id);
                app.api_backoff_clear_channel_messages(&channel_id);
            }
            if let Some(guild_id) = app.active_guild_id() {
                app.loading_channels.remove(&guild_id);
                app.guild_members_synced.remove(&guild_id);
                app.loading_members.remove(&guild_id);
                app.api_backoff_clear_guild(&guild_id);
            }
        }
        // e = add reaction
        KeyCode::Char('e')
            if app.focus == Focus::Messages && app.selected_message_index.is_some() =>
        {
            if app.can_react_in_active_channel() {
                app.focus = Focus::Input;
                app.input = ":".to_string();
                app.start_emoji_autocomplete();
                app.set_status("Pick an emoji to react with, then press Enter");
            } else {
                app.set_status("No permission to add reactions here.");
            }
        }
        // f = forward (but is not working properly yet)
        KeyCode::Char('f')
            if app.focus == Focus::Messages && app.selected_message_index.is_some() =>
        {
            if let Some(msg) = app.selected_message() {
                app.forward_mode = true;
                app.reply_to = Some(crate::app::ReplyState {
                    channel_id: msg.channel_id.clone(),
                    message_id: msg.id.clone(),
                    author_name: crate::app::display_name(&msg.author),
                });
                app.set_status("Forward mode: switch to target channel and press Enter to forward");
            }
        }
        _ => {}
    }
}

fn schedule_needed_fetches(
    app: &mut App,
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
) {
    if let Some(guild_id) = app.active_guild_id() {
        if !app.guild_channels.contains_key(&guild_id)
            && app.api_backoff_can_try(&format!("channels:{guild_id}"))
            && app.loading_channels.insert(guild_id.clone())
        {
            spawn_guild_channels_load(client.clone(), event_tx.clone(), guild_id.clone());
        }
        if !app.guild_emojis.contains_key(&guild_id)
            && app.api_backoff_can_try(&format!("emojis:{guild_id}"))
            && app.loading_emojis.insert(guild_id.clone())
        {
            spawn_guild_emojis_load(client.clone(), event_tx.clone(), guild_id.clone());
        }
        if !app.guild_roles.contains_key(&guild_id)
            && app.api_backoff_can_try(&format!("roles:{guild_id}"))
            && app.loading_roles.insert(guild_id.clone())
        {
            spawn_guild_roles_load(client.clone(), event_tx.clone(), guild_id);
        }
    }

    if let Some(channel_id) = app.active_channel_id()
        && app.active_channel_is_text()
        && !app.messages.contains_key(&channel_id)
        && app.api_backoff_can_try(&format!("messages:{channel_id}"))
        && app.loading_messages.insert(channel_id.clone())
    {
        spawn_message_load(client.clone(), event_tx.clone(), channel_id.clone());
    }
}

fn schedule_guild_members_fetch_for_mentions(
    app: &mut App,
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
) -> bool {
    let Some(guild_id) = app.active_guild_id() else {
        return false;
    };
    if app.guild_members_synced.contains(&guild_id) {
        return false;
    }
    if !app.api_backoff_can_try(&format!("members:{guild_id}")) {
        return false;
    }
    if app.loading_members.contains(&guild_id) {
        return true;
    }
    if app.loading_members.insert(guild_id.clone()) {
        spawn_guild_members_load(client, event_tx, guild_id);
    }
    true
}

fn spawn_guild_channels_load(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
) {
    tokio::spawn(async move {
        match client.guild_channels(&guild_id).await {
            Ok(channels) => {
                let _ = event_tx.send(AppEvent::GuildChannelsLoaded { guild_id, channels });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::GuildChannelsFailed {
                    guild_id,
                    message: format!("Failed to load channels: {err}"),
                });
            }
        }
    });
}

fn spawn_guild_members_load(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
) {
    tokio::spawn(async move {
        match client.guild_members(&guild_id).await {
            Ok(members) => {
                let _ = event_tx.send(AppEvent::GuildMembersLoaded { guild_id, members });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::GuildMembersFailed {
                    guild_id,
                    message: format!("Failed to load guild members: {err}"),
                });
            }
        }
    });
}

fn spawn_message_load(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
) {
    tokio::spawn(async move {
        let query = MessageQuery {
            limit: Some(50),
            before: None,
            after: None,
            around: None,
        };

        match client.channel_messages(&channel_id, &query).await {
            Ok(messages) => {
                let _ = event_tx.send(AppEvent::MessagesLoaded {
                    channel_id,
                    messages,
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::MessagesFailed {
                    channel_id,
                    message: format!("Failed to load messages: {err}"),
                });
            }
        }
    });
}

fn spawn_guild_emojis_load(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
) {
    tokio::spawn(async move {
        match client.guild_emojis(&guild_id).await {
            Ok(emojis) => {
                let _ = event_tx.send(AppEvent::GuildEmojisLoaded { guild_id, emojis });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::GuildEmojisFailed {
                    guild_id,
                    message: format!("Failed to load emojis: {err}"),
                });
            }
        }
    });
}

fn spawn_guild_roles_load(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
) {
    tokio::spawn(async move {
        match client.guild_roles(&guild_id).await {
            Ok(roles) => {
                let _ = event_tx.send(AppEvent::GuildRolesLoaded { guild_id, roles });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::GuildRolesFailed {
                    guild_id,
                    message: format!("Failed to load roles: {err}"),
                });
            }
        }
    });
}

fn spawn_send_message(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
    content: String,
    reply: Option<crate::app::ReplyState>,
) {
    tokio::spawn(async move {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        let message_reference = reply.map(|r| MessageReferenceRequest {
            message_id: r.message_id,
            channel_id: Some(r.channel_id),
            guild_id: None,
            reference_type: Some(0), // 0 = reply
        });

        let request = CreateMessageRequest {
            content: Some(content),
            nonce: Some(nonce),
            flags: None,
            message_reference,
        };

        match client.send_message(&channel_id, &request).await {
            Ok(message) => {
                let _ = event_tx.send(AppEvent::MessageSent {
                    channel_id,
                    message: Box::new(message),
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ApiError(format!("Failed to send message: {err}")));
            }
        }
    });
}

fn ack_current_channel(
    app: &mut App,
    client: &FluxerHttpClient,
    _event_tx: &UnboundedSender<AppEvent>,
) {
    if let Some(channel_id) = app.active_channel_id()
        && app.channel_is_unread(&channel_id)
    {
        if let Some(msgs) = app.messages.get(&channel_id)
            && let Some(last) = msgs.last()
        {
            let msg_id = last.id.clone();
            let ch_id = channel_id.clone();
            let c = client.clone();
            tokio::spawn(async move {
                let _ = c.ack_message(&ch_id, &msg_id).await;
            });
        }
        app.ack_channel(&channel_id);
    }
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("failed to create terminal")
}

fn open_url_background(url: &str) {
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "start";

    let _ = std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
