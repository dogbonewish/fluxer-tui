use crate::api::types::{
    AuthSessionChangeEvent, CallDeleteEvent, CallEvent, ChannelBulkUpdateEvent, ChannelResponse,
    GuildCreateEvent, GuildDeleteEvent, GuildResponse, MessageAckEvent, MessageDeleteEvent,
    MessageReactionAddEvent, MessageReactionRemoveEvent, MessageResponse, ReadyEvent,
    UserPrivateResponse, UserSettingsResponse, VoiceStateResponse,
};
use crate::app::{App, GatewayStatus, ServerSelection};
use serde_json::Value;

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
        message: String,
    },
    ApiError(String),
}

#[derive(Debug, Default)]
pub struct EventEffects {
    pub persist_token: Option<String>,
}

pub fn apply_event(app: &mut App, event: AppEvent) -> EventEffects {
    let mut effects = EventEffects::default();
    match event {
        AppEvent::GatewayStatus(status) => {
            app.gateway_status = status;
            if status != GatewayStatus::Connected {
                app.gateway_lazy_guild_id = None;
            }
            app.set_status(format!("Gateway {}", status.label()));
        }
        AppEvent::Dispatch { kind, payload } => match kind.as_str() {
            "READY" => {
                if let Ok(ready) = serde_json::from_value::<ReadyEvent>(payload) {
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
                        let member_count = guild.members.len();
                        if member_count > 0 {
                            app.set_guild_members(&guild_id, guild.members);
                        }
                        if member_count > 1 {
                            app.guild_members_synced.insert(guild_id);
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
                    app.set_status("Gateway READY");
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
                    let member_count = event.members.len();
                    if member_count > 0 {
                        app.set_guild_members(&guild_id, event.members);
                    }
                    if member_count > 1 {
                        app.guild_members_synced.insert(guild_id);
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
            "MESSAGE_CREATE" | "MESSAGE_UPDATE" => {
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
                            mention_count: 0,
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
        AppEvent::GuildRolesFailed { guild_id, message } => {
            app.loading_roles.remove(&guild_id);
            app.api_backoff_after_failure(format!("roles:{guild_id}"));
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
            app.upsert_message(message);
            app.message_scroll_from_bottom = 0;
        }
        AppEvent::ApiError(message) => {
            app.set_status(message);
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
