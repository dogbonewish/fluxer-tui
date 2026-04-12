use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type Snowflake = String;

pub const CHANNEL_GUILD_TEXT: i32 = 0;
pub const CHANNEL_DM: i32 = 1;
pub const CHANNEL_GUILD_VOICE: i32 = 2;
pub const CHANNEL_GROUP_DM: i32 = 3;
pub const CHANNEL_GUILD_CATEGORY: i32 = 4;
pub const CHANNEL_GUILD_LINK: i32 = 998;
pub const CHANNEL_DM_PERSONAL_NOTES: i32 = 999;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WellKnownFluxerResponse {
    #[serde(default)]
    pub api_code_version: u64,
    #[serde(default)]
    pub endpoints: WellKnownEndpoints,
    #[serde(default)]
    pub features: WellKnownFeatures,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WellKnownEndpoints {
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub gateway: String,
    #[serde(default)]
    pub media: String,
    #[serde(default)]
    pub webapp: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WellKnownFeatures {
    #[serde(default)]
    pub voice_enabled: bool,
    #[serde(default)]
    pub sms_mfa_enabled: bool,
    #[serde(default)]
    pub self_hosted: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HandoffInitiateResponse {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub expires_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HandoffStatusResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayBotResponse {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub shards: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPrivateResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPartialResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub system: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: String,
    #[serde(default)]
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildMemberResponse {
    #[serde(default)]
    pub user: UserPartialResponse,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub deaf: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<String>,
    #[serde(default)]
    pub kind: i32,
    #[serde(default, rename = "type")]
    pub raw_kind: i32,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub bitrate: Option<i32>,
    #[serde(default)]
    pub user_limit: Option<i32>,
    #[serde(default)]
    pub rtc_region: Option<String>,
    #[serde(default)]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub recipients: Vec<UserPartialResponse>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionOverwrite {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub kind: i32,
    #[serde(default)]
    pub allow: String,
    #[serde(default)]
    pub deny: String,
}

impl ChannelResponse {
    pub fn channel_type(&self) -> i32 {
        if self.raw_kind != 0 || self.kind == 0 {
            self.raw_kind
        } else {
            self.kind
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageAttachmentResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub url: Option<String>,
    /// CDN proxy URL (Discord-style); prefer for opening media when present.
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedMediaResponse {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildEmojiResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub animated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildRoleResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Discord RGB (0 = default styling in clients).
    #[serde(default)]
    pub color: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReactionEmojiResponse {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub animated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageReactionResponse {
    #[serde(default)]
    pub emoji: ReactionEmojiResponse,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub me: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageReferenceResponse {
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default, rename = "type")]
    pub reference_type: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageReferenceRequest {
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub reference_type: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadStateResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub mention_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub author: UserPartialResponse,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub attachments: Vec<MessageAttachmentResponse>,
    #[serde(default)]
    pub channel_type: Option<i32>,
    #[serde(default)]
    pub embeds: Vec<MessageEmbedResponse>,
    #[serde(default)]
    pub reactions: Vec<MessageReactionResponse>,
    #[serde(default)]
    pub message_reference: Option<MessageReferenceResponse>,
    #[serde(default)]
    pub referenced_message: Option<Box<MessageResponse>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageEmbedResponse {
    #[serde(default, rename = "type")]
    pub embed_type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub author: Option<EmbedAuthorResponse>,
    #[serde(default)]
    pub footer: Option<EmbedFooterResponse>,
    #[serde(default)]
    pub fields: Vec<EmbedFieldResponse>,
    #[serde(default)]
    pub provider: Option<EmbedAuthorResponse>,
    #[serde(default)]
    pub image: Option<EmbedMediaResponse>,
    #[serde(default)]
    pub thumbnail: Option<EmbedMediaResponse>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedAuthorResponse {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedFooterResponse {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedFieldResponse {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSettingsResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub developer_mode: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadyEvent {
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub user: UserPrivateResponse,
    #[serde(default)]
    pub guilds: Vec<GuildCreateEvent>,
    #[serde(default)]
    pub private_channels: Vec<ChannelResponse>,
    #[serde(default)]
    pub users: Vec<UserPartialResponse>,
    #[serde(default)]
    pub user_settings: Option<UserSettingsResponse>,
    #[serde(default)]
    pub read_state: Vec<ReadStateResponse>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildCreateEvent {
    #[serde(flatten)]
    pub guild: GuildResponse,
    #[serde(default)]
    pub unavailable: bool,
    #[serde(default)]
    pub channels: Vec<ChannelResponse>,
    #[serde(default)]
    pub members: Vec<GuildMemberResponse>,
    #[serde(default)]
    pub voice_states: Vec<VoiceStateResponse>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildDeleteEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelBulkUpdateEvent {
    #[serde(default)]
    pub channels: Vec<ChannelResponse>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageDeleteEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub channel_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VoiceStateResponse {
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub connection_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub member: Option<GuildMemberResponse>,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub deaf: bool,
    #[serde(default)]
    pub self_mute: bool,
    #[serde(default)]
    pub self_deaf: bool,
    #[serde(default)]
    pub self_video: bool,
    #[serde(default)]
    pub self_stream: bool,
    #[serde(default)]
    pub is_mobile: bool,
    #[serde(default)]
    pub version: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthSessionChangeEvent {
    #[serde(default)]
    pub new_token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallEvent {
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub ringing: Vec<String>,
    #[serde(default)]
    pub voice_states: Vec<VoiceStateResponse>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallDeleteEvent {
    #[serde(default)]
    pub channel_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReferenceRequest>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub around: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayHelloPayload {
    #[serde(default)]
    pub heartbeat_interval: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayPayload {
    #[serde(default)]
    pub op: u8,
    #[serde(default)]
    pub d: Value,
    #[serde(default)]
    pub s: Option<u64>,
    #[serde(default)]
    pub t: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayIdentifyPayload {
    pub token: String,
    pub properties: GatewayIdentifyProperties,
    pub flags: u32,
    /// Snowflake of the guild the user is viewing. Marks that guild active for passive sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_guild_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayIdentifyProperties {
    pub os: String,
    pub browser: String,
    pub device: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayResumePayload {
    pub token: String,
    pub session_id: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageReactionAddEvent {
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub emoji: ReactionEmojiResponse,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageReactionRemoveEvent {
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub emoji: ReactionEmojiResponse,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageAckEvent {
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub message_id: String,
}

pub fn snowflake_sort_key(value: &str) -> u128 {
    value.parse::<u128>().unwrap_or_default()
}

pub fn merge_user_cache(
    cache: &mut HashMap<Snowflake, UserPartialResponse>,
    users: impl IntoIterator<Item = UserPartialResponse>,
) {
    for user in users {
        if !user.id.is_empty() {
            cache.insert(user.id.clone(), user);
        }
    }
}
