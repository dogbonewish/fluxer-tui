mod api;
mod app;
mod auth;
mod config;
mod events;
mod media;
mod permissions;
mod slash_commands;
mod ui;

use crate::api::client::{ApiError, FluxerHttpClient};
use crate::api::gateway::{GatewayCommand, run_gateway};
use crate::api::types::{CreateMessageRequest, MessageQuery, MessageReferenceRequest};
use crate::app::{
    App, Focus, GatewayStatus, ImagePreviewState, ServerSelection, display_name, me_as_partial,
};
use crate::auth::ensure_auth;
use crate::config::{
    AppConfig, DEFAULT_API_BASE_URL, default_config_path, load_config, save_config,
};
use crate::events::{AppEvent, apply_event};
use crate::media::{MessagePreviewMedia, first_message_preview_media};
use anyhow::{Context, Error as AnyhowError, Result};
use clap::Parser;
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, terminal};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use reqwest::StatusCode;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::time::{Duration, interval};

fn err_is_http_status(err: &AnyhowError, want: StatusCode) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<ApiError>()
            .is_some_and(|e| matches!(e, ApiError::Response { status, .. } if *status == want))
    })
}

#[derive(Debug, Parser)]
#[command(name = "fluxer-tui", version = env!("CARGO_PKG_VERSION"))]
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
        config.ui.clone(),
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
    app.image_picker = ratatui_image::picker::Picker::from_query_stdio().ok();
    let mut reader = EventStream::new();
    let mut tick = interval(Duration::from_millis(100));

    loop {
        if let Err(e) = terminal.draw(|frame| ui::draw(frame, &mut app)) {
            eprintln!("fluxer-tui: terminal draw failed: {e}");
            break;
        }

        match app.image_preview.as_mut() {
            Some(ImagePreviewState::ReadyBitmap { protocol, .. }) => {
                if let Some(Err(err)) = protocol.last_encoding_result() {
                    app.set_status(format!("Image preview: {err}"));
                }
            }
            Some(ImagePreviewState::ReadyAnimatedGif {
                current_protocol, ..
            }) => {
                if let Some(Err(err)) = current_protocol.last_encoding_result() {
                    app.set_status(format!("Image preview: {err}"));
                }
            }
            _ => {}
        }

        tokio::select! {
            maybe_event = reader.next() => {
                if let Some(Ok(ev)) = maybe_event {
                    match ev {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            handle_key_event(
                                &mut app,
                                key,
                                &authed_client,
                                &event_tx,
                                &gateway_cmd_tx,
                                config_path.as_path(),
                                &mut config,
                            );
                            schedule_needed_fetches(
                                &mut app,
                                authed_client.clone(),
                                event_tx.clone(),
                            );
                            ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
                        }
                        Event::Paste(text) => {
                            handle_paste_event(&mut app, &text, &authed_client, &event_tx);
                            schedule_needed_fetches(
                                &mut app,
                                authed_client.clone(),
                                event_tx.clone(),
                            );
                            ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
                        }
                        _ => {}
                    }
                }
            }
            Some(event) = event_rx.recv() => {
                let effects = apply_event(&mut app, event, &event_tx);
                if let Some(token) = effects.persist_token {
                    config.token = Some(token);
                    save_config(&config_path, &config)?;
                }
                if let Some((title, bytes)) = effects.chafa_fallback {
                    let (cols, rows) = app.chafa_preview_cells;
                    spawn_image_chafa_fallback(event_tx.clone(), title, bytes, cols, rows);
                }
                schedule_needed_fetches(&mut app, authed_client.clone(), event_tx.clone());
                ensure_lazy_guild_subscription(&mut app, &gateway_cmd_tx);
            }
            _ = tick.tick() => {
                app.advance_image_preview_animation(Duration::from_millis(100));
                app.prune_stale_typing();
                app.expire_status_if_needed();
                if app.others_typing_anim_active() {
                    app.input_bar_anim_slow = app.input_bar_anim_slow.saturating_add(1);
                    if app.input_bar_anim_slow >= 2 {
                        app.input_bar_anim_slow = 0;
                        app.input_bar_anim_phase = (app.input_bar_anim_phase + 1) % 4;
                    }
                } else {
                    app.input_bar_anim_slow = 0;
                    app.input_bar_anim_phase = 0;
                }
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

const INPUT_MAX_CHARS: usize = 2000;

fn normalize_pasted_text(s: &str) -> String {
    s.replace('\r', "")
}

fn sanitize_pasted_char(ch: char) -> Option<char> {
    match ch {
        '\n' => Some('\n'),
        '\t' => Some(' '),
        c if c.is_control() => None,
        c => Some(c),
    }
}

fn delete_word_backward(buf: &mut String) {
    if buf.is_empty() {
        return;
    }
    let chars: Vec<char> = buf.chars().collect();
    let mut i = chars.len();
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    buf.clear();
    for ch in chars.into_iter().take(i) {
        buf.push(ch);
    }
}

fn push_chars_respecting_limit(out: &mut String, text: &str, max: usize) {
    let room = max.saturating_sub(out.chars().count());
    if room == 0 {
        return;
    }
    let mut n = 0usize;
    for ch in text.chars() {
        if n >= room {
            break;
        }
        if let Some(c) = sanitize_pasted_char(ch) {
            out.push(c);
            n += 1;
        }
    }
}

fn handle_paste_event(
    app: &mut App,
    text: &str,
    client: &FluxerHttpClient,
    event_tx: &UnboundedSender<AppEvent>,
) {
    let text = normalize_pasted_text(text);
    if text.is_empty() {
        return;
    }

    if app.show_help {
        return;
    }

    if app.channel_picker.is_some() {
        if let Some(p) = app.channel_picker.as_mut() {
            for ch in text.chars() {
                match ch {
                    '\n' | '\r' => {}
                    '\t' => p.query.push(' '),
                    c if !c.is_control() => p.query.push(c),
                    _ => {}
                }
            }
            app.filter_channel_picker();
        }
        return;
    }

    if app.focus != Focus::Input {
        return;
    }

    if app.mention_autocomplete.is_some() {
        push_chars_respecting_limit(&mut app.input, &text, INPUT_MAX_CHARS);
        app.update_mention_filter();
        return;
    }

    if app.emoji_autocomplete.is_some() {
        push_chars_respecting_limit(&mut app.input, &text, INPUT_MAX_CHARS);
        app.update_emoji_filter();
        return;
    }

    if app.command_autocomplete.is_some() {
        push_chars_respecting_limit(&mut app.input, &text, INPUT_MAX_CHARS);
        app.sync_command_autocomplete();
        return;
    }

    push_chars_respecting_limit(&mut app.input, &text, INPUT_MAX_CHARS);

    if app.input.ends_with(':') {
        app.start_emoji_autocomplete();
    } else if app.input.ends_with('@') {
        let member_fetch_pending =
            schedule_guild_members_fetch_for_mentions(app, client.clone(), event_tx.clone());
        app.start_mention_autocomplete();
        if member_fetch_pending && app.mention_autocomplete.is_none() {
            app.set_status("Loading members for @mentions…");
        }
    }
    app.sync_command_autocomplete();
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

fn persist_ui_settings(path: &Path, cfg: &mut AppConfig, app: &App) {
    cfg.ui = app.ui_settings.clone();
    let _ = save_config(path, cfg);
}

fn handle_key_event(
    app: &mut App,
    key: KeyEvent,
    client: &FluxerHttpClient,
    event_tx: &UnboundedSender<AppEvent>,
    _gateway_cmd_tx: &UnboundedSender<GatewayCommand>,
    config_path: &Path,
    config: &mut AppConfig,
) {
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.show_help = false,
            KeyCode::Up | KeyCode::Char('k') => {
                app.help_scroll = app.help_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.help_scroll = app.help_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                app.help_scroll = app.help_scroll.saturating_sub(12);
            }
            KeyCode::PageDown => {
                app.help_scroll = app.help_scroll.saturating_add(12);
            }
            _ => {}
        }
        return;
    }

    if matches!(key.code, KeyCode::F(2)) {
        app.show_settings = !app.show_settings;
        if app.show_settings {
            app.settings_cursor = 0;
            app.show_server_notifications = false;
            app.dismiss_image_preview();
        }
        return;
    }

    if app.show_settings {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.show_settings = false,
            KeyCode::Up | KeyCode::Char('k') => {
                app.settings_cursor = app.settings_cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.settings_cursor = (app.settings_cursor + 1).min(App::UI_SETTINGS_LAST_ROW);
            }
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Left => {
                app.toggle_settings_selection();
                persist_ui_settings(config_path, config, app);
            }
            _ => {}
        }
        return;
    }

    if app.show_server_notifications {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.show_server_notifications = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.server_notification_cursor =
                    app.server_notification_cursor.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.server_notification_cursor = (app.server_notification_cursor + 1)
                    .min(App::SERVER_NOTIFICATION_LAST_ROW);
            }
            KeyCode::PageUp => {
                app.server_notification_scroll = app.server_notification_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                app.server_notification_scroll = app.server_notification_scroll.saturating_add(1);
            }
            KeyCode::Home => {
                app.server_notification_scroll = 0;
            }
            KeyCode::End => {
                app.server_notification_scroll = u16::MAX;
            }
            KeyCode::Left => {
                if let Some((guild_id, patch)) = app.cycle_server_notification_setting(-1) {
                    spawn_user_guild_settings_update(
                        client.clone(),
                        event_tx.clone(),
                        guild_id,
                        patch,
                    );
                }
            }
            KeyCode::Right | KeyCode::Char(' ') => {
                if let Some((guild_id, patch)) = app.cycle_server_notification_setting(1) {
                    spawn_user_guild_settings_update(
                        client.clone(),
                        event_tx.clone(),
                        guild_id,
                        patch,
                    );
                }
            }
            _ => {}
        }
        return;
    }

    if app.image_preview.is_some() {
        let chafa_scroll = matches!(
            app.image_preview,
            Some(ImagePreviewState::ReadyChafa { .. })
        );
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.dismiss_image_preview(),
            KeyCode::Up | KeyCode::Char('k') if chafa_scroll => app.image_preview_scroll(-1),
            KeyCode::Down | KeyCode::Char('j') if chafa_scroll => app.image_preview_scroll(1),
            KeyCode::PageUp if chafa_scroll => app.image_preview_scroll(-12),
            KeyCode::PageDown if chafa_scroll => app.image_preview_scroll(12),
            _ => {}
        }
        return;
    }

    if matches!(key.code, KeyCode::F(1)) {
        app.open_help();
        return;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H'))
        && app.focus != Focus::Input
        && app.channel_picker.is_none()
    {
        app.open_help();
        return;
    }

    if app.channel_picker.is_some() {
        match key.code {
            KeyCode::Esc => app.dismiss_channel_picker(),
            KeyCode::Up => app.channel_picker_prev(),
            KeyCode::Down => app.channel_picker_next(),
            KeyCode::Enter => {
                let old_channel_id = app.selected_channel_id.clone();
                if app.channel_picker_confirm() {
                    ack_channel_if_unread(app, client, old_channel_id.as_deref());
                }
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = app.channel_picker.as_mut() {
                    delete_word_backward(&mut p.query);
                    app.filter_channel_picker();
                }
            }
            KeyCode::Char('h') | KeyCode::Char('H')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(p) = app.channel_picker.as_mut() {
                    delete_word_backward(&mut p.query);
                    app.filter_channel_picker();
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = app.channel_picker.as_mut() {
                    p.query.pop();
                    app.filter_channel_picker();
                }
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(p) = app.channel_picker.as_mut() {
                    p.query.push(ch);
                    app.filter_channel_picker();
                }
            }
            _ => {}
        }
        return;
    }

    let block_ctrl_nav = app.emoji_autocomplete.is_some()
        || app.mention_autocomplete.is_some()
        || app.command_autocomplete.is_some();
    if key.modifiers.contains(KeyModifiers::CONTROL) && !block_ctrl_nav {
        match key.code {
            KeyCode::Char('n') => {
                let old = app.selected_channel_id.clone();
                app.move_channel_wrapping(1);
                if old != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old.as_deref());
                }
                return;
            }
            KeyCode::Char('p') => {
                let old = app.selected_channel_id.clone();
                app.move_channel_wrapping(-1);
                if old != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old.as_deref());
                }
                return;
            }
            KeyCode::Char('k') => {
                app.open_channel_picker();
                return;
            }
            KeyCode::Char('e') => {
                if app.focus == Focus::Messages
                    && app.selected_message_index.is_some()
                    && let Some(msg) = app.selected_message()
                    && app.can_edit_message(&msg)
                {
                    app.start_edit_message(msg);
                    app.focus = Focus::Input;
                    app.set_status("Editing - Enter to save, Esc to cancel");
                    return;
                }
            }
            KeyCode::Char('d') => {
                if app.focus == Focus::Messages
                    && app.selected_message_index.is_some()
                    && let Some(msg) = app.selected_message()
                    && app.can_delete_message(&msg)
                {
                    let ch = msg.channel_id.clone();
                    let mid = msg.id.clone();
                    spawn_delete_message(client.clone(), event_tx.clone(), ch, mid);
                    app.selected_message_index = None;
                    return;
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if app.focus == Focus::Messages {
                    if let Some(msg) = app.selected_message() {
                        match first_message_preview_media(&msg) {
                            Some(MessagePreviewMedia::Image { url, label }) => {
                                app.start_image_preview_loading(label.clone());
                                spawn_image_preview(client.clone(), event_tx.clone(), url, label);
                            }
                            Some(MessagePreviewMedia::Video { url, label }) => {
                                app.set_status(format!("Fetching {label}…"));
                                spawn_open_video(client.clone(), event_tx.clone(), url, label);
                            }
                            None => {
                                app.set_status(
                                    "No image or video attachment or embed on this message.",
                                );
                            }
                        }
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    if key.modifiers.contains(KeyModifiers::ALT)
        && matches!(key.code, KeyCode::Char('a') | KeyCode::Char('A'))
        && !block_ctrl_nav
    {
        if let Some((srv, cid)) = app.next_channel_with_activity() {
            let old_ch = app.selected_channel_id.clone();
            app.selected_server = srv;
            app.selected_channel_id = Some(cid);
            app.normalize_selection();
            app.message_scroll_from_bottom = 0;
            app.selected_message_index = None;
            if old_ch != app.selected_channel_id {
                ack_channel_if_unread(app, client, old_ch.as_deref());
            }
            app.set_status("Jumped to channel with activity.");
        } else {
            app.set_status("No other channels with unread or mention activity.");
        }
        return;
    }

    if app.focus == Focus::Input {
        if app.command_autocomplete.is_some() {
            match key.code {
                KeyCode::Esc => {
                    app.dismiss_command_autocomplete();
                }
                KeyCode::Up => {
                    app.autocomplete_command_prev();
                }
                KeyCode::Down => {
                    app.autocomplete_command_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    app.insert_selected_slash_command();
                }
                KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    delete_word_backward(&mut app.input);
                    app.sync_command_autocomplete();
                }
                KeyCode::Char('h') | KeyCode::Char('H')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    delete_word_backward(&mut app.input);
                    app.sync_command_autocomplete();
                }
                KeyCode::Backspace => {
                    app.input.pop();
                    app.sync_command_autocomplete();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.input.push(ch);
                    app.sync_command_autocomplete();
                }
                _ => {}
            }
            return;
        }

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
                KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    delete_word_backward(&mut app.input);
                    app.update_mention_filter();
                }
                KeyCode::Char('h') | KeyCode::Char('H')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    delete_word_backward(&mut app.input);
                    app.update_mention_filter();
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
                    if app.reaction_target.is_some() {
                        app.reaction_target = None;
                        app.input.clear();
                        app.focus = Focus::Messages;
                    }
                    app.dismiss_emoji_autocomplete();
                }
                KeyCode::Up => {
                    app.autocomplete_emoji_prev();
                }
                KeyCode::Down => {
                    app.autocomplete_emoji_next();
                }
                KeyCode::Tab | KeyCode::Enter => {
                    if app.reaction_target.is_some() {
                        if let Some((ch, msg, emoji)) = app.confirm_reaction_emoji() {
                            spawn_add_reaction(client.clone(), event_tx.clone(), ch, msg, emoji);
                            app.focus = Focus::Messages;
                        }
                    } else {
                        app.insert_selected_emoji();
                    }
                }
                KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    delete_word_backward(&mut app.input);
                    app.update_emoji_filter();
                }
                KeyCode::Char('h') | KeyCode::Char('H')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    delete_word_backward(&mut app.input);
                    app.update_emoji_filter();
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
                app.dismiss_command_autocomplete();
                if app.reaction_target.is_some() {
                    app.reaction_target = None;
                    app.dismiss_emoji_autocomplete();
                    app.input.clear();
                    app.focus = Focus::Messages;
                } else if app.reply_to.is_some() || app.edit_target.is_some() {
                    app.cancel_reply();
                } else {
                    app.focus = Focus::Channels;
                }
            }
            KeyCode::Up => {
                app.focus = Focus::Messages;
            }
            KeyCode::Enter => {
                if app.edit_target.is_some() {
                    if app.input.trim().is_empty() {
                        app.set_status("Edited message cannot be empty.");
                        return;
                    }
                    if let Some(et) = app.edit_target.clone() {
                        let content = app.input.clone();
                        app.message_scroll_from_bottom = 0;
                        spawn_edit_message(
                            client.clone(),
                            event_tx.clone(),
                            et.channel_id,
                            et.message_id,
                            content,
                        );
                    }
                    return;
                }
                let is_forward = app.forward_mode;
                let has_ref = app.reply_to.is_some();
                let allow_send = !app.input.trim().is_empty() || (is_forward && has_ref);
                if app.active_channel_is_text() && app.can_send_in_active_channel() && allow_send {
                    let channel_id = match app.active_channel_id() {
                        Some(channel_id) => channel_id,
                        None => return,
                    };
                    let trimmed = app.input.trim().to_string();
                    let guild_id = app.active_channel().and_then(|c| c.guild_id.clone());
                    let prev_nick = guild_id
                        .as_ref()
                        .map(|g| app.self_nick_or_username_in_guild(g.as_str()))
                        .unwrap_or_else(|| display_name(&me_as_partial(&app.me)));
                    let ch_perms = app.active_channel_permissions();
                    let resolved = crate::slash_commands::resolve_outgoing_slash(
                        &trimmed,
                        guild_id.as_deref(),
                        &app.me.username,
                        &prev_nick,
                        ch_perms,
                    );
                    if let crate::slash_commands::OutgoingSlash::Blocked(msg) = &resolved {
                        app.set_status(msg.clone());
                        return;
                    }
                    if let crate::slash_commands::OutgoingSlash::SetNick {
                        guild_id,
                        nick,
                        prev_display,
                        new_display,
                    } = resolved
                    {
                        app.forward_mode = false;
                        let _ = std::mem::take(&mut app.input);
                        app.reply_to = None;
                        app.message_scroll_from_bottom = 0;
                        spawn_nick_change(
                            client.clone(),
                            event_tx.clone(),
                            guild_id,
                            nick,
                            channel_id,
                            prev_display,
                            new_display,
                        );
                        return;
                    }
                    let (content_to_send, tts) = match resolved {
                        crate::slash_commands::OutgoingSlash::Normal => (trimmed, false),
                        crate::slash_commands::OutgoingSlash::SendContent(c) => (c, false),
                        crate::slash_commands::OutgoingSlash::SendTts(c) => (c, true),
                        _ => unreachable!(),
                    };
                    app.forward_mode = false;
                    let _ = std::mem::take(&mut app.input);
                    let reply = app.reply_to.take();
                    app.message_scroll_from_bottom = 0;
                    spawn_send_message(
                        client.clone(),
                        event_tx.clone(),
                        channel_id,
                        content_to_send,
                        reply,
                        is_forward,
                        tts,
                    );
                }
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                delete_word_backward(&mut app.input);
                app.sync_command_autocomplete();
            }
            KeyCode::Char('h') | KeyCode::Char('H')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                delete_word_backward(&mut app.input);
                app.sync_command_autocomplete();
            }
            KeyCode::Backspace => {
                app.input.pop();
                app.sync_command_autocomplete();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.dismiss_command_autocomplete();
                app.input.clear();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.input.push(ch);
                if ch == '/' {
                    app.sync_command_autocomplete();
                } else if ch == ':' {
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
                app.sync_command_autocomplete();
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
        KeyCode::Char('n')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(app.focus, Focus::Servers | Focus::Channels) =>
        {
            app.show_settings = false;
            app.open_server_notification_settings();
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
                let old_ch = app.selected_channel_id.clone();
                app.move_server(-1);
                if old_ch != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old_ch.as_deref());
                }
            }
            Focus::Channels => {
                let old_ch = app.selected_channel_id.clone();
                app.move_channel(-1);
                if old_ch != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old_ch.as_deref());
                }
            }
            Focus::Messages => {
                if app.selected_message_index.is_some() {
                    app.move_selected_message(-1);
                } else {
                    app.scroll_messages_up(3);
                    maybe_auto_load_older_messages(app, client, event_tx);
                }
            }
            Focus::Input => {}
        },
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            Focus::Servers => {
                let old_ch = app.selected_channel_id.clone();
                app.move_server(1);
                if old_ch != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old_ch.as_deref());
                }
            }
            Focus::Channels => {
                let old_ch = app.selected_channel_id.clone();
                app.move_channel(1);
                if old_ch != app.selected_channel_id {
                    ack_channel_if_unread(app, client, old_ch.as_deref());
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
        KeyCode::PageUp => {
            app.scroll_messages_up(18);
            if app.selected_message_index.is_none() {
                maybe_auto_load_older_messages(app, client, event_tx);
            }
        }
        KeyCode::PageDown => app.scroll_messages_down(18),
        // s = select mode
        KeyCode::Char('s') if app.focus == Focus::Messages => {
            let count = app.active_messages().len();
            if count > 0 {
                app.selected_message_index = Some(count.saturating_sub(1));
                app.clamp_scroll_to_selected_message();
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
                app.messages_older_exhausted.remove(&channel_id);
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
                if let (Some(msg), Some(ch_id)) = (app.selected_message(), app.active_channel_id())
                {
                    app.reaction_target = Some((ch_id, msg.id.clone()));
                }
                app.focus = Focus::Input;
                app.input = ":".to_string();
                app.start_emoji_autocomplete();
                app.set_status("Pick an emoji, Enter to react (Esc to cancel)");
            } else {
                app.set_status("No permission to add reactions here.");
            }
        }
        // f = forward (but is not working properly yet)
        KeyCode::Char('f')
            if app.focus == Focus::Messages && app.selected_message_index.is_some() =>
        {
            if let Some(msg) = app.selected_message() {
                app.edit_target = None;
                app.forward_mode = true;
                let src_guild = app.guild_id_for_channel(&msg.channel_id);
                app.reply_to = Some(crate::app::ReplyState {
                    channel_id: msg.channel_id.clone(),
                    message_id: msg.id.clone(),
                    author_name: app.shown_name_for_user(src_guild.as_deref(), &msg.author),
                    source_guild_id: src_guild,
                });
                app.set_status("Forward: pick channel (Ctrl+K), type optional note, Enter to send");
            }
        }
        KeyCode::Char('[') if app.focus == Focus::Messages => {
            try_load_older_messages(app, client, event_tx);
        }
        _ => {}
    }
}

fn schedule_needed_fetches(
    app: &mut App,
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
) {
    for guild_id in app.guilds.iter().map(|guild| guild.id.clone()) {
        if !app.guild_channels.contains_key(&guild_id)
            && app.api_backoff_can_try(&format!("channels:{guild_id}"))
            && app.loading_channels.insert(guild_id.clone())
        {
            spawn_guild_channels_load(client.clone(), event_tx.clone(), guild_id);
        }
    }

    let active_guild_id = app
        .guild_id_for_active_channel()
        .or_else(|| app.active_guild_id());
    if let Some(guild_id) = active_guild_id {
        if !app.guild_emojis.contains_key(&guild_id)
            && app.api_backoff_can_try(&format!("emojis:{guild_id}"))
            && app.loading_emojis.insert(guild_id.clone())
        {
            spawn_guild_emojis_load(client.clone(), event_tx.clone(), guild_id.clone());
        }
        if !app.guild_roles.contains_key(&guild_id)
            && !app.guild_roles_forbidden.contains(&guild_id)
            && app.api_backoff_can_try(&format!("roles:{guild_id}"))
            && app.loading_roles.insert(guild_id.clone())
        {
            spawn_guild_roles_load(client.clone(), event_tx.clone(), guild_id.clone());
        }
        if !app.guild_members_synced.contains(&guild_id)
            && !app.loading_members.contains(&guild_id)
            && app.api_backoff_can_try(&format!("members:{guild_id}"))
            && app.loading_members.insert(guild_id.clone())
        {
            spawn_guild_members_load(client.clone(), event_tx.clone(), guild_id.clone());
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
    let Some(guild_id) = app.guild_id_for_active_channel() else {
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
                let forbidden = err_is_http_status(&err, StatusCode::FORBIDDEN);
                let _ = event_tx.send(AppEvent::GuildRolesFailed {
                    guild_id,
                    forbidden,
                    message: format!("Failed to load roles: {err}"),
                });
            }
        }
    });
}

fn try_load_older_messages(
    app: &mut App,
    client: &FluxerHttpClient,
    event_tx: &UnboundedSender<AppEvent>,
) {
    let Some(channel_id) = app.active_channel_id() else {
        return;
    };
    if !app.active_channel_is_text() {
        return;
    }
    if app.messages_older_exhausted.contains(&channel_id) {
        return;
    }
    if app.loading_older_messages.contains(&channel_id)
        || app.loading_messages.contains(&channel_id)
    {
        return;
    }
    let Some(oldest_id) = app.active_oldest_message_id() else {
        return;
    };
    if app.loading_older_messages.insert(channel_id.clone()) {
        spawn_message_load_older(client.clone(), event_tx.clone(), channel_id, oldest_id);
    }
}

fn maybe_auto_load_older_messages(
    app: &mut App,
    client: &FluxerHttpClient,
    event_tx: &UnboundedSender<AppEvent>,
) {
    if app.should_auto_load_history_on_scroll_up() {
        try_load_older_messages(app, client, event_tx);
    }
}

fn spawn_message_load_older(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
    before: String,
) {
    tokio::spawn(async move {
        let query = MessageQuery {
            limit: Some(50),
            before: Some(before),
            after: None,
            around: None,
        };

        match client.channel_messages(&channel_id, &query).await {
            Ok(messages) => {
                let _ = event_tx.send(AppEvent::MessagesOlderLoaded {
                    channel_id,
                    messages,
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::MessagesOlderFailed {
                    channel_id,
                    message: format!("Failed to load older messages: {err}"),
                });
            }
        }
    });
}

fn spawn_add_reaction(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
    message_id: String,
    emoji: String,
) {
    tokio::spawn(async move {
        match client.add_reaction(&channel_id, &message_id, &emoji).await {
            Ok(()) => {}
            Err(err) => {
                let _ = event_tx.send(AppEvent::ApiError(format!("Failed to add reaction: {err}")));
            }
        }
    });
}

fn spawn_edit_message(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
    message_id: String,
    content: String,
) {
    tokio::spawn(async move {
        match client
            .edit_message(&channel_id, &message_id, &content)
            .await
        {
            Ok(message) => {
                let ch = message.channel_id.clone();
                let _ = event_tx.send(AppEvent::MessageSent {
                    channel_id: ch,
                    message: Box::new(message),
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ApiError(format!("Failed to edit message: {err}")));
            }
        }
    });
}

fn spawn_open_video(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    url: String,
    label: String,
) {
    tokio::spawn(async move {
        let result: anyhow::Result<()> = async {
            let bytes = client
                .fetch_url_bytes(&url)
                .await
                .with_context(|| format!("download video ({url})"))?;
            let label_for_tmp = label.clone();
            let path = tokio::task::spawn_blocking(move || {
                crate::media::write_temp_video_bytes(&label_for_tmp, &bytes)
            })
            .await
            .context("temp file task")??;
            tokio::task::spawn_blocking(move || crate::media::open_file_path(&path))
                .await
                .context("open task")??;
            Ok(())
        }
        .await;

        let msg = match result {
            Ok(()) => format!("Opened {label}"),
            Err(e) => format!("Couldn't open video: {e:#}"),
        };
        let _ = event_tx.send(AppEvent::SetStatus(msg));
    });
}

fn spawn_image_preview(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    url: String,
    title: String,
) {
    tokio::spawn(async move {
        match client.fetch_url_bytes(&url).await {
            Ok(bytes) => {
                let _ = event_tx.send(AppEvent::ImagePreviewBytes { title, bytes });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ImagePreviewFailed {
                    message: format!("{err:#}"),
                });
            }
        }
    });
}

fn spawn_image_chafa_fallback(
    event_tx: UnboundedSender<AppEvent>,
    title: String,
    bytes: Vec<u8>,
    cols: u16,
    rows: u16,
) {
    tokio::spawn(async move {
        match crate::media::chafa_from_bytes(&bytes, cols, rows).await {
            Ok(lines) => {
                let _ = event_tx.send(AppEvent::ImagePreviewReady { title, lines });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ImagePreviewFailed {
                    message: format!("{err:#}"),
                });
            }
        }
    });
}

fn spawn_delete_message(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    channel_id: String,
    message_id: String,
) {
    tokio::spawn(async move {
        match client.delete_message(&channel_id, &message_id).await {
            Ok(()) => {
                let _ = event_tx.send(AppEvent::MessageDeleted {
                    channel_id,
                    message_id,
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ApiError(format!(
                    "Failed to delete message: {err}"
                )));
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
    is_forward: bool,
    tts: bool,
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
            guild_id: r.source_guild_id,
            reference_type: Some(if is_forward { 1 } else { 0 }),
        });

        let request = CreateMessageRequest {
            content: if content.is_empty() {
                None
            } else {
                Some(content)
            },
            nonce: Some(nonce),
            flags: None,
            tts: if tts { Some(true) } else { None },
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

fn spawn_nick_change(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
    nick: Option<String>,
    channel_id: String,
    prev_display: String,
    new_display: String,
) {
    tokio::spawn(async move {
        let nick_ref = nick.as_deref();
        match client
            .patch_current_guild_member_nick(&guild_id, nick_ref)
            .await
        {
            Ok(member) => {
                let _ = event_tx.send(AppEvent::NickChangeSuccess {
                    guild_id,
                    member,
                    channel_id,
                    prev_display,
                    new_display,
                });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::ApiError(format!(
                    "Failed to change nickname: {err}"
                )));
            }
        }
    });
}

fn spawn_user_guild_settings_update(
    client: FluxerHttpClient,
    event_tx: UnboundedSender<AppEvent>,
    guild_id: String,
    patch: crate::api::types::UserGuildSettingsPatch,
) {
    tokio::spawn(async move {
        match client
            .update_user_guild_settings(Some(guild_id.as_str()), &patch)
            .await
        {
            Ok(settings) => {
                let _ = event_tx.send(AppEvent::UserGuildSettingsUpdated { settings });
            }
            Err(err) => {
                let _ = event_tx.send(AppEvent::SetStatus(format!(
                    "Failed to update notification settings: {err}"
                )));
            }
        }
    });
}

fn ack_channel_if_unread(app: &mut App, client: &FluxerHttpClient, channel_id: Option<&str>) {
    if let Some(channel_id) = channel_id
        && (app.channel_is_unread(channel_id) || app.channel_mention_count(channel_id) > 0)
    {
        if let Some(msg_id) = app.channel_last_message_id(channel_id) {
            let ch_id = channel_id.to_string();
            let c = client.clone();
            tokio::spawn(async move {
                let _ = c.ack_message(&ch_id, &msg_id).await;
            });
        }
        app.ack_channel(channel_id);
    }
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)
        .context("failed to enter alternate screen")?;
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
        let _ = execute!(stdout, DisableBracketedPaste, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
