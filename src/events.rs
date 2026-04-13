use crate::api::types::{
    AuthSessionChangeEvent, CallDeleteEvent, CallEvent, ChannelBulkUpdateEvent, ChannelResponse,
    GuildCreateEvent, GuildDeleteEvent, GuildMemberResponse, GuildResponse, MessageAckEvent,
    MessageDeleteEvent, MessageReactionAddEvent, MessageReactionRemoveEvent, MessageResponse,
    ReadyEvent, TypingStartEvent, UserPrivateResponse, UserSettingsResponse, VoiceStateResponse,
};
use crate::app::{App, GatewayStatus, ImagePreviewState, ServerSelection};
use image::DynamicImage;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum AppEvent {
    GatewayStatus(GatewayStatus),
    Dispatch {
        kind: String,
        payload: Value,
    },
    GuildChannelsLoaded {
        guild_id: String,
        channels: Vec<ChannelResponse>,
    },
    GuildChannelsFailed {
        guild_id: String,
        message: String,
    },
    GuildMembersLoaded {
        guild_id: String,
        members: Vec<crate::api::types::GuildMemberResponse>,
    },
    GuildMembersFailed {
        guild_id: String,
        message: String,
    },
    MessagesLoaded {
        channel_id: String,
        messages: Vec<MessageResponse>,
    },
    MessagesFailed {
        channel_id: String,
        message: String,
    },
    MessageSent {
        channel_id: String,
        message: Box<MessageResponse>,
    },
    GuildEmojisLoaded {
        guild_id: String,
        emojis: Vec<crate::api::types::GuildEmojiResponse>,
    },
    GuildEmojisFailed {
        guild_id: String,
        message: String,
    },
    GuildRolesLoaded {
        guild_id: String,
        roles: Vec<crate::api::types::GuildRoleResponse>,
    },
    GuildRolesFailed {
        guild_id: String,
        forbidden: bool,
        message: String,
    },
    MessagesOlderLoaded {
        channel_id: String,
        messages: Vec<MessageResponse>,
    },
    MessagesOlderFailed {
        channel_id: String,
        message: String,
    },
    ApiError(String),
    SetStatus(String),
    MessageDeleted {
        channel_id: String,
        message_id: String,
    },
    NickChangeSuccess {
        guild_id: String,
        member: GuildMemberResponse,
        channel_id: String,
        prev_display: String,
        new_display: String,
    },
    ImagePreviewBytes {
        title: String,
        bytes: Vec<u8>,
    },
    ImageDecodedGif {
        title: String,
        frames: Vec<DynamicImage>,
        delays: Vec<Duration>,
    },
    ImageDecodedStatic {
        title: String,
        image: DynamicImage,
    },
    ImageDecodeFailed {
        title: String,
        bytes: Vec<u8>,
    },
    ImagePreviewReady {
        title: String,
        lines: Vec<String>,
    },
    ImagePreviewFailed {
        message: String,
    },
}

#[derive(Debug, Default)]
pub struct EventEffects {
    pub persist_token: Option<String>,
    pub chafa_fallback: Option<(String, Vec<u8>)>,
}

pub fn apply_event(
    app: &mut App,
    event: AppEvent,
    event_tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
) -> EventEffects {
    let mut effects = EventEffects::default();
    match event {
        AppEvent::GatewayStatus(status) => {
            app.gateway_status = status;
            if status != GatewayStatus::Connected {
                app.gateway_lazy_guild_id = None;
                app.clear_all_typing();
            }
        }
        AppEvent::Dispatch { kind, payload } => match kind.as_str() {
            "READY" => {
                if let Ok(ready) = serde_json::from_value::<ReadyEvent>(payload) {
                    app.clear_all_typing();
                    app.me = ready.user.clone();
                    if let Some(settings) = ready.user_settings {
                        app.user_settings = Some(settings);
                    }
                    if !ready.private_channels.is_empty() {
                        app.set_private_channels(ready.private_channels);
                    }
                    for guild in ready.guilds {
                        if guild.unavailable {
                            continue;
                        }
                        let guild_id = guild.guild.id.clone();
                        app.upsert_guild(guild.guild);
                        if !guild.channels.is_empty() {
                            app.set_guild_channels(&guild_id, guild.channels);
                        }
                        if !guild.members.is_empty() {
                            app.ingest_gateway_guild_members(&guild_id, guild.members);
                        }
                        if !guild.roles.is_empty() {
                            app.merge_guild_roles_from_gateway(&guild_id, guild.roles);
                        }

                        for voice_state in guild.voice_states {
                            app.update_voice_state(voice_state);
                        }
                    }
                    crate::api::types::merge_user_cache(&mut app.user_cache, ready.users);
                    if !ready.read_state.is_empty() {
                        app.set_read_states(ready.read_state);
                    }
                    app.gateway_lazy_guild_id = None;
                }
            }
            "RESUMED" => {
                app.gateway_lazy_guild_id = None;
            }
            "USER_UPDATE" => {
                if let Ok(user) = serde_json::from_value::<UserPrivateResponse>(payload) {
                    app.me = user;
                }
            }
            "USER_SETTINGS_UPDATE" => {
                if let Ok(settings) = serde_json::from_value::<UserSettingsResponse>(payload) {
                    app.user_settings = Some(settings);
                }
            }
            "AUTH_SESSION_CHANGE" => {
                if let Ok(auth) = serde_json::from_value::<AuthSessionChangeEvent>(payload)
                    && !auth.new_token.is_empty()
                {
                    effects.persist_token = Some(auth.new_token);
                }
            }
            "GUILD_CREATE" | "GUILD_SYNC" => {
                if let Ok(event) = serde_json::from_value::<GuildCreateEvent>(payload)
                    && !event.unavailable
                {
                    let guild_id = event.guild.id.clone();
                    app.upsert_guild(event.guild);
                    if !event.channels.is_empty() {
                        app.set_guild_channels(&guild_id, event.channels);
                    }
                    if !event.members.is_empty() {
                        app.ingest_gateway_guild_members(&guild_id, event.members);
                    }
                    if !event.roles.is_empty() {
                        app.merge_guild_roles_from_gateway(&guild_id, event.roles);
                    }
                    for voice_state in event.voice_states {
                        app.update_voice_state(voice_state);
                    }
                }
            }
            "GUILD_UPDATE" => {
                if let Ok(guild) = serde_json::from_value::<GuildResponse>(payload) {
                    app.upsert_guild(guild);
                }
            }
            "GUILD_DELETE" => {
                if let Ok(event) = serde_json::from_value::<GuildDeleteEvent>(payload)
                    && !event.unavailable
                {
                    app.remove_guild(&event.id);
                    if app.selected_server == ServerSelection::Guild(event.id) {
                        app.selected_server = ServerSelection::DirectMessages;
                        app.normalize_selection();
                    }
                }
            }
            "CHANNEL_CREATE" | "CHANNEL_UPDATE" => {
                if let Ok(channel) = serde_json::from_value::<ChannelResponse>(payload) {
                    app.upsert_channel(channel);
                }
            }
            "CHANNEL_UPDATE_BULK" => {
                if let Ok(event) = serde_json::from_value::<ChannelBulkUpdateEvent>(payload) {
                    for channel in event.channels {
                        app.upsert_channel(channel);
                    }
                }
            }
            "CHANNEL_DELETE" => {
                if let Ok(channel) = serde_json::from_value::<ChannelResponse>(payload) {
                    app.remove_channel(&channel);
                }
            }
            "MESSAGE_CREATE" => {
                if let Ok(message) = serde_json::from_value::<MessageResponse>(payload) {
                    app.clear_typing_for_message(&message.channel_id, &message.author.id);
                    if app.upsert_message(message.clone()) {
                        app.on_gateway_message_create(&message);
                    }
                }
            }
            "TYPING_START" => {
                if let Ok(ev) = serde_json::from_value::<TypingStartEvent>(payload) {
                    if !ev.channel_id.is_empty()
                        && !ev.user_id.is_empty()
                        && ev.user_id != app.me.id
                    {
                        if let (Some(gid), Some(m)) = (ev.guild_id.as_deref(), ev.member.as_ref()) {
                            app.merge_guild_member(gid, m.clone());
                        }
                        app.record_typing(&ev.channel_id, &ev.user_id);
                    }
                }
            }
            "MESSAGE_UPDATE" => {
                if let Ok(message) = serde_json::from_value::<MessageResponse>(payload) {
                    app.upsert_message(message);
                }
            }
            "MESSAGE_DELETE" => {
                if let Ok(event) = serde_json::from_value::<MessageDeleteEvent>(payload) {
                    app.remove_message(&event.channel_id, &event.id);
                }
            }
            "MESSAGE_ACK" => {
                if let Ok(event) = serde_json::from_value::<MessageAckEvent>(payload) {
                    app.read_states.insert(
                        event.channel_id,
                        crate::app::ReadState {
                            last_message_id: Some(event.message_id),
                            mention_count: event.mention_count,
                        },
                    );
                }
            }
            "MESSAGE_REACTION_ADD" => {
                if let Ok(event) = serde_json::from_value::<MessageReactionAddEvent>(payload)
                    && let Some(msgs) = app.messages.get_mut(&event.channel_id)
                    && let Some(msg) = msgs.iter_mut().find(|m| m.id == event.message_id)
                {
                    let is_me = event.user_id == app.me.id;
                    let emoji_key = reaction_emoji_key(&event.emoji);
                    if let Some(existing) = msg
                        .reactions
                        .iter_mut()
                        .find(|r| reaction_emoji_key(&r.emoji) == emoji_key)
                    {
                        existing.count += 1;
                        if is_me {
                            existing.me = true;
                        }
                    } else {
                        msg.reactions
                            .push(crate::api::types::MessageReactionResponse {
                                emoji: event.emoji,
                                count: 1,
                                me: is_me,
                            });
                    }
                }
            }
            "MESSAGE_REACTION_REMOVE" => {
                if let Ok(event) = serde_json::from_value::<MessageReactionRemoveEvent>(payload)
                    && let Some(msgs) = app.messages.get_mut(&event.channel_id)
                    && let Some(msg) = msgs.iter_mut().find(|m| m.id == event.message_id)
                {
                    let is_me = event.user_id == app.me.id;
                    let emoji_key = reaction_emoji_key(&event.emoji);
                    if let Some(existing) = msg
                        .reactions
                        .iter_mut()
                        .find(|r| reaction_emoji_key(&r.emoji) == emoji_key)
                    {
                        existing.count = existing.count.saturating_sub(1);
                        if is_me {
                            existing.me = false;
                        }
                    }
                    msg.reactions.retain(|r| r.count > 0);
                }
            }
            "VOICE_STATE_UPDATE" => {
                if let Ok(state) = serde_json::from_value::<VoiceStateResponse>(payload) {
                    app.update_voice_state(state);
                }
            }
            "CALL_CREATE" | "CALL_UPDATE" => {
                let _ = serde_json::from_value::<CallEvent>(payload);
            }
            "CALL_DELETE" => {
                let _ = serde_json::from_value::<CallDeleteEvent>(payload);
            }
            "GUILD_EMOJIS_UPDATE" => {
                #[derive(serde::Deserialize)]
                struct EmojiUpdate {
                    guild_id: String,
                    emojis: Vec<crate::api::types::GuildEmojiResponse>,
                }
                if let Ok(update) = serde_json::from_value::<EmojiUpdate>(payload) {
                    app.set_guild_emojis(&update.guild_id, update.emojis);
                }
            }
            "GUILD_ROLE_CREATE" | "GUILD_ROLE_UPDATE" => {
                #[derive(serde::Deserialize)]
                struct GuildRolePayload {
                    guild_id: String,
                    role: crate::api::types::GuildRoleResponse,
                }
                if let Ok(p) = serde_json::from_value::<GuildRolePayload>(payload) {
                    app.merge_guild_roles_from_gateway(&p.guild_id, vec![p.role]);
                }
            }
            "GUILD_ROLE_DELETE" => {
                #[derive(serde::Deserialize)]
                struct GuildRoleDeletePayload {
                    guild_id: String,
                    role_id: String,
                }
                if let Ok(p) = serde_json::from_value::<GuildRoleDeletePayload>(payload) {
                    app.remove_guild_role(&p.guild_id, &p.role_id);
                }
            }
            "GUILD_ROLE_UPDATE_BULK" => {
                #[derive(serde::Deserialize)]
                struct GuildRoleBulkPayload {
                    guild_id: String,
                    roles: Vec<crate::api::types::GuildRoleResponse>,
                }
                if let Ok(p) = serde_json::from_value::<GuildRoleBulkPayload>(payload) {
                    app.merge_guild_roles_from_gateway(&p.guild_id, p.roles);
                }
            }
            _ => {}
        },
        AppEvent::GuildChannelsLoaded { guild_id, channels } => {
            app.set_guild_channels(&guild_id, channels);
        }
        AppEvent::GuildChannelsFailed { guild_id, message } => {
            app.loading_channels.remove(&guild_id);
            app.api_backoff_after_failure(format!("channels:{guild_id}"));
            app.set_status(message);
        }
        AppEvent::GuildMembersLoaded { guild_id, members } => {
            app.set_guild_members(&guild_id, members);
            app.refresh_mention_autocomplete_after_members_load(&guild_id);
            app.guild_members_synced.insert(guild_id);
        }
        AppEvent::GuildMembersFailed { guild_id, message } => {
            app.loading_members.remove(&guild_id);
            app.api_backoff_after_failure(format!("members:{guild_id}"));
            app.set_status(message);
        }
        AppEvent::MessagesLoaded {
            channel_id,
            messages,
        } => {
            app.set_channel_messages(&channel_id, messages);
        }
        AppEvent::MessagesFailed {
            channel_id,
            message,
        } => {
            app.loading_messages.remove(&channel_id);
            app.loading_older_messages.remove(&channel_id);
            app.api_backoff_after_failure(format!("messages:{channel_id}"));
            app.set_status(message);
        }
        AppEvent::GuildEmojisLoaded { guild_id, emojis } => {
            app.set_guild_emojis(&guild_id, emojis);
        }
        AppEvent::GuildEmojisFailed { guild_id, message } => {
            app.loading_emojis.remove(&guild_id);
            app.api_backoff_after_failure(format!("emojis:{guild_id}"));
            app.set_status(message);
        }
        AppEvent::GuildRolesLoaded { guild_id, roles } => {
            app.set_guild_roles(&guild_id, roles);
        }
        AppEvent::GuildRolesFailed {
            guild_id,
            forbidden,
            message,
        } => {
            app.loading_roles.remove(&guild_id);
            if forbidden {
                app.guild_roles_forbidden.insert(guild_id.clone());
            } else {
                app.api_backoff_after_failure(format!("roles:{guild_id}"));
                app.set_status(message);
            }
        }
        AppEvent::MessagesOlderLoaded {
            channel_id,
            messages,
        } => {
            let n = messages.len();
            app.prepend_channel_messages(&channel_id, messages);
            if n < 50 {
                app.messages_older_exhausted.insert(channel_id.clone());
            }
            let bump = (n as u16).saturating_mul(2).min(160);
            app.message_scroll_from_bottom = app.message_scroll_from_bottom.saturating_add(bump);
            if n == 0 {
                app.set_status("No older messages.");
            } else {
                app.set_status(format!("Loaded {n} older message(s)."));
            }
        }
        AppEvent::MessagesOlderFailed {
            channel_id,
            message,
        } => {
            app.loading_older_messages.remove(&channel_id);
            app.set_status(message);
        }
        AppEvent::MessageSent {
            channel_id,
            message,
        } => {
            let mut message = *message;
            if message.channel_id.is_empty() {
                message.channel_id = channel_id.clone();
            }
            if message.author.id.is_empty() {
                message.author = crate::app::me_as_partial(&app.me);
            }
            let was_new = app.upsert_message(message.clone());
            if was_new {
                app.on_gateway_message_create(&message);
            }
            app.message_scroll_from_bottom = 0;
            app.forward_mode = false;
            app.edit_target = None;
            app.input.clear();
        }
        AppEvent::MessageDeleted {
            channel_id,
            message_id,
        } => {
            app.remove_message(&channel_id, &message_id);
            app.selected_message_index = None;
        }
        AppEvent::NickChangeSuccess {
            guild_id,
            member,
            channel_id,
            prev_display,
            new_display,
        } => {
            app.merge_guild_member(&guild_id, member);
            let content =
                crate::slash_commands::nick_change_system_markdown(&prev_display, &new_display);
            let id = app.allocate_local_message_snowflake(&channel_id);
            let message = MessageResponse {
                id,
                channel_id: channel_id.clone(),
                author: crate::slash_commands::fluxerbot_author(),
                message_type: crate::slash_commands::MESSAGE_TYPE_CLIENT_SYSTEM,
                tts: false,
                content,
                timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                edited_timestamp: None,
                pinned: false,
                mention_everyone: false,
                mentions: vec![],
                mention_roles: vec![],
                attachments: vec![],
                channel_type: None,
                embeds: vec![],
                reactions: vec![],
                message_reference: None,
                referenced_message: None,
                member: None,
            };
            let was_new = app.upsert_message(message.clone());
            if was_new {
                app.on_gateway_message_create(&message);
            }
            app.message_scroll_from_bottom = 0;
        }
        AppEvent::ApiError(message) => {
            app.set_status(message);
        }
        AppEvent::SetStatus(message) => {
            app.set_status(message);
        }
        AppEvent::ImagePreviewBytes { title, bytes } => {
            if !matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                return effects;
            }
            if app.image_picker.is_some() {
                let title_clone = title.clone();
                let bytes_clone = bytes.clone();
                let event_tx_clone = event_tx.clone();
                std::thread::Builder::new()
                    .name("image-decode".into())
                    .spawn(move || {
                        if let Some((frames, delays)) =
                            crate::media::decode_gif_animation(&bytes_clone)
                        {
                            let _ = event_tx_clone.send(AppEvent::ImageDecodedGif {
                                title: title_clone,
                                frames,
                                delays,
                            });
                        } else if let Ok(img) = image::load_from_memory(&bytes_clone) {
                            let _ = event_tx_clone.send(AppEvent::ImageDecodedStatic {
                                title: title_clone,
                                image: img,
                            });
                        } else {
                            let _ = event_tx_clone.send(AppEvent::ImageDecodeFailed {
                                title: title_clone,
                                bytes: bytes_clone,
                            });
                        }
                    })
                    .ok();
            } else {
                effects.chafa_fallback = Some((title, bytes));
            }
        }
        AppEvent::ImageDecodeFailed { title, bytes } => {
            if !matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                return effects;
            }
            effects.chafa_fallback = Some((title, bytes));
        }
        AppEvent::ImageDecodedGif {
            title,
            frames,
            delays,
        } => {
            if !matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                return effects;
            }
            if let Some(ref picker) = app.image_picker {
                let current_protocol = picker.new_resize_protocol(frames[0].clone());
                app.image_preview = Some(ImagePreviewState::ReadyAnimatedGif {
                    title,
                    frames,
                    delays,
                    frame_idx: 0,
                    elapsed: std::time::Duration::ZERO,
                    current_protocol,
                });
            }
        }
        AppEvent::ImageDecodedStatic { title, image } => {
            if !matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                return effects;
            }
            if let Some(ref picker) = app.image_picker {
                let protocol = picker.new_resize_protocol(image);
                app.image_preview = Some(ImagePreviewState::ReadyBitmap { title, protocol });
            }
        }
        AppEvent::ImagePreviewReady { title, lines } => {
            if matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                app.image_preview = Some(ImagePreviewState::ReadyChafa {
                    title,
                    lines,
                    scroll: 0,
                });
            }
        }
        AppEvent::ImagePreviewFailed { message } => {
            if matches!(app.image_preview, Some(ImagePreviewState::Loading { .. })) {
                app.image_preview = Some(ImagePreviewState::Failed { message });
            }
        }
    }

    effects
}

fn reaction_emoji_key(emoji: &crate::api::types::ReactionEmojiResponse) -> String {
    if let Some(id) = &emoji.id {
        id.clone()
    } else {
        emoji.name.clone()
    }
}
