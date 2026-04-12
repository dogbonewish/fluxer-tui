use crate::api::types::{
    CHANNEL_DM, CHANNEL_DM_PERSONAL_NOTES, CHANNEL_GROUP_DM, CHANNEL_GUILD_CATEGORY,
    CHANNEL_GUILD_LINK, CHANNEL_GUILD_TEXT, CHANNEL_GUILD_VOICE, ChannelResponse,
    GuildMemberResponse, GuildResponse, MessageResponse, ReadStateResponse, Snowflake,
    UserPartialResponse, UserPrivateResponse, UserSettingsResponse, VoiceStateResponse,
    WellKnownFluxerResponse, merge_user_cache, snowflake_sort_key,
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Servers,
    Channels,
    Messages,
    Input,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Servers => Self::Channels,
            Self::Channels => Self::Messages,
            Self::Messages => Self::Input,
            Self::Input => Self::Servers,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Servers => Self::Input,
            Self::Channels => Self::Servers,
            Self::Messages => Self::Channels,
            Self::Input => Self::Messages,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerSelection {
    DirectMessages,
    Guild(String),
}

impl ServerSelection {
    pub fn id(&self) -> String {
        match self {
            Self::DirectMessages => "@me".to_string(),
            Self::Guild(id) => id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayStatus {
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
}

impl GatewayStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmojiMatch {
    pub label: String,
    pub insert: String,
    pub is_custom: bool,
}

#[derive(Debug, Clone)]
pub struct EmojiAutocomplete {
    pub matches: Vec<EmojiMatch>,
    pub selected_index: usize,
}

/// One selectable row in @ autocomplete (users vs roles are separate insert targets).
#[derive(Debug, Clone)]
pub enum MentionPick {
    User {
        user_id: String,
        display: String,
        username: String,
    },
    Role {
        role_id: String,
        name: String,
        color: u32,
    },
}

impl MentionPick {
    fn matches_filter(&self, query: &str) -> bool {
        match self {
            MentionPick::User {
                display, username, ..
            } => display.to_lowercase().contains(query) || username.to_lowercase().contains(query),
            MentionPick::Role { name, .. } => name.to_lowercase().contains(query),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MentionAutocomplete {
    /// Built once when @ opens; not rebuilt on each keystroke.
    pub pool: Vec<MentionPick>,
    /// Indices into `pool` for the current filter.
    pub matches: Vec<usize>,
    pub selected_index: usize,
}

#[derive(Debug, Clone)]
pub struct ReplyState {
    pub channel_id: String,
    pub message_id: String,
    pub author_name: String,
}

#[derive(Debug, Clone)]
pub struct ReadState {
    pub last_message_id: Option<String>,
    pub mention_count: u64,
}

#[derive(Debug, Clone)]
pub struct App {
    pub discovery: WellKnownFluxerResponse,
    pub me: UserPrivateResponse,
    pub user_settings: Option<UserSettingsResponse>,
    pub guilds: Vec<GuildResponse>,
    pub private_channels: Vec<ChannelResponse>,
    pub guild_channels: HashMap<Snowflake, Vec<ChannelResponse>>,
    pub guild_members: HashMap<Snowflake, Vec<GuildMemberResponse>>,
    pub messages: HashMap<Snowflake, Vec<MessageResponse>>,
    pub user_cache: HashMap<Snowflake, UserPartialResponse>,
    pub voice_states: HashMap<Snowflake, HashMap<Snowflake, VoiceStateResponse>>,
    pub guild_emojis: HashMap<Snowflake, Vec<crate::api::types::GuildEmojiResponse>>,
    pub guild_roles: HashMap<Snowflake, Vec<crate::api::types::GuildRoleResponse>>,
    pub emoji_autocomplete: Option<EmojiAutocomplete>,
    pub mention_autocomplete: Option<MentionAutocomplete>,
    pub selected_server: ServerSelection,
    pub selected_channel_id: Option<String>,
    pub focus: Focus,
    pub input: String,
    pub message_scroll_from_bottom: u16,
    pub selected_message_index: Option<usize>,
    pub reply_to: Option<ReplyState>,
    pub forward_mode: bool,
    pub read_states: HashMap<Snowflake, ReadState>,
    pub gateway_status: GatewayStatus,
    pub gateway_lazy_guild_id: Option<String>,
    pub status_message: String,
    pub should_quit: bool,
    pub should_logout: bool,
    pub loading_channels: HashSet<String>,
    pub loading_members: HashSet<String>,
    pub guild_members_synced: HashSet<String>,
    pub api_backoff_until: HashMap<String, Instant>,
    pub loading_messages: HashSet<String>,
    pub loading_emojis: HashSet<String>,
    pub loading_roles: HashSet<String>,
}

impl App {
    pub fn new(
        discovery: WellKnownFluxerResponse,
        me: UserPrivateResponse,
        user_settings: Option<UserSettingsResponse>,
        guilds: Vec<GuildResponse>,
        private_channels: Vec<ChannelResponse>,
        selected_server: ServerSelection,
        selected_channel_id: Option<String>,
    ) -> Self {
        let mut user_cache = HashMap::new();
        merge_user_cache(
            &mut user_cache,
            private_channels
                .iter()
                .flat_map(|channel| channel.recipients.clone()),
        );

        let mut app = Self {
            discovery,
            me,
            user_settings,
            guilds,
            private_channels,
            guild_channels: HashMap::new(),
            guild_members: HashMap::new(),
            messages: HashMap::new(),
            user_cache,
            voice_states: HashMap::new(),
            guild_emojis: HashMap::new(),
            guild_roles: HashMap::new(),
            emoji_autocomplete: None,
            mention_autocomplete: None,
            selected_server,
            selected_channel_id,
            focus: Focus::Channels,
            input: String::new(),
            message_scroll_from_bottom: 0,
            selected_message_index: None,
            reply_to: None,
            forward_mode: false,
            read_states: HashMap::new(),
            gateway_status: GatewayStatus::Disconnected,
            gateway_lazy_guild_id: None,
            status_message: String::new(),
            should_quit: false,
            should_logout: false,
            loading_channels: HashSet::new(),
            loading_members: HashSet::new(),
            guild_members_synced: HashSet::new(),
            api_backoff_until: HashMap::new(),
            loading_messages: HashSet::new(),
            loading_emojis: HashSet::new(),
            loading_roles: HashSet::new(),
        };
        app.normalize_selection();
        app
    }

    pub const API_FAILURE_BACKOFF_SECS: u64 = 180;

    pub fn api_backoff_can_try(&self, key: &str) -> bool {
        self
            .api_backoff_until
            .get(key).is_none_or(|until| Instant::now() >= *until)
    }

    pub fn api_backoff_after_failure(&mut self, key: impl Into<String>) {
        self.api_backoff_until.insert(
            key.into(),
            Instant::now() + Duration::from_secs(Self::API_FAILURE_BACKOFF_SECS),
        );
    }

    pub fn api_backoff_clear(&mut self, key: &str) {
        self.api_backoff_until.remove(key);
    }

    pub fn api_backoff_clear_guild(&mut self, guild_id: &str) {
        for prefix in ["members:", "channels:", "emojis:", "roles:"] {
            self.api_backoff_until
                .remove(&format!("{prefix}{guild_id}"));
        }
    }

    pub fn api_backoff_clear_channel_messages(&mut self, channel_id: &str) {
        self.api_backoff_until
            .remove(&format!("messages:{channel_id}"));
    }

    pub fn set_guild_emojis(
        &mut self,
        guild_id: &str,
        emojis: Vec<crate::api::types::GuildEmojiResponse>,
    ) {
        self.guild_emojis.insert(guild_id.to_string(), emojis);
        self.loading_emojis.remove(guild_id);
        self.api_backoff_clear(&format!("emojis:{guild_id}"));
    }

    pub fn set_guild_roles(
        &mut self,
        guild_id: &str,
        roles: Vec<crate::api::types::GuildRoleResponse>,
    ) {
        self.guild_roles.insert(guild_id.to_string(), roles);
        self.loading_roles.remove(guild_id);
        self.api_backoff_clear(&format!("roles:{guild_id}"));
    }

    pub fn server_entries(&self) -> Vec<ServerSelection> {
        let mut entries = vec![ServerSelection::DirectMessages];
        entries.extend(
            self.guilds
                .iter()
                .map(|guild| ServerSelection::Guild(guild.id.clone())),
        );
        entries
    }

    pub fn server_selected_index(&self) -> usize {
        self.server_entries()
            .iter()
            .position(|entry| entry == &self.selected_server)
            .unwrap_or_default()
    }

    pub fn move_server(&mut self, delta: i32) -> bool {
        let entries = self.server_entries();
        if entries.is_empty() {
            return false;
        }
        let current = self.server_selected_index() as i32;
        let next = (current + delta).clamp(0, entries.len() as i32 - 1) as usize;
        if entries[next] == self.selected_server {
            return false;
        }
        self.selected_server = entries[next].clone();
        self.selected_channel_id = None;
        self.message_scroll_from_bottom = 0;
        self.normalize_selection();
        true
    }

    pub fn channel_entries(&self) -> Vec<ChannelResponse> {
        match &self.selected_server {
            ServerSelection::DirectMessages => {
                let mut dms = self.private_channels.clone();
                dms.sort_by(|a, b| {
                    let a_key = a
                        .last_message_id
                        .as_deref()
                        .and_then(|id| id.parse::<u128>().ok())
                        .unwrap_or(0);
                    let b_key = b
                        .last_message_id
                        .as_deref()
                        .and_then(|id| id.parse::<u128>().ok())
                        .unwrap_or(0);
                    b_key.cmp(&a_key)
                });
                dms
            }
            ServerSelection::Guild(guild_id) => {
                let all = self
                    .guild_channels
                    .get(guild_id)
                    .cloned()
                    .unwrap_or_default();

                let mut categories: Vec<&ChannelResponse> = all
                    .iter()
                    .filter(|c| c.channel_type() == CHANNEL_GUILD_CATEGORY)
                    .collect();
                categories.sort_by_key(|c| c.position);

                let mut non_cat: Vec<&ChannelResponse> = all
                    .iter()
                    .filter(|c| c.channel_type() != CHANNEL_GUILD_CATEGORY)
                    .collect();
                non_cat.sort_by(|a, b| a.position.cmp(&b.position).then(a.name.cmp(&b.name)));

                let mut result: Vec<ChannelResponse> = Vec::new();

                // channels with no parent (uncategorized) come first
                let uncategorized: Vec<&ChannelResponse> = non_cat
                    .iter()
                    .filter(|c| c.parent_id.is_none())
                    .copied()
                    .collect();
                for ch in uncategorized {
                    result.push(ch.clone());
                }

                for cat in &categories {
                    result.push((*cat).clone());
                    let children: Vec<&ChannelResponse> = non_cat
                        .iter()
                        .filter(|c| c.parent_id.as_deref() == Some(cat.id.as_str()))
                        .copied()
                        .collect();
                    for ch in children {
                        result.push(ch.clone());
                    }
                }

                result
            }
        }
    }

    pub fn channel_selected_index(&self) -> usize {
        self.channel_entries()
            .iter()
            .position(|channel| Some(channel.id.as_str()) == self.selected_channel_id.as_deref())
            .unwrap_or_default()
    }

    pub fn move_channel(&mut self, delta: i32) -> bool {
        let channels = self.channel_entries();
        if channels.is_empty() {
            self.selected_channel_id = None;
            return false;
        }

        let current = self.channel_selected_index() as i32;
        let mut next = current;
        let len = channels.len() as i32;
        loop {
            next = (next + delta).clamp(0, len - 1);
            if channels[next as usize].channel_type() != CHANNEL_GUILD_CATEGORY {
                break;
            }
            if next == 0 || next == len - 1 {
                break;
            }
        }
        let next = next as usize;
        if channels[next].channel_type() == CHANNEL_GUILD_CATEGORY {
            return false;
        }
        let next_id = channels[next].id.clone();
        if self.selected_channel_id.as_deref() == Some(next_id.as_str()) {
            return false;
        }

        self.selected_channel_id = Some(next_id);
        self.message_scroll_from_bottom = 0;
        self.selected_message_index = None;
        true
    }

    pub fn active_channel(&self) -> Option<ChannelResponse> {
        let active_id = self.selected_channel_id.as_deref()?;
        self.channel_entries()
            .into_iter()
            .find(|channel| channel.id == active_id)
    }

    pub fn active_channel_id(&self) -> Option<String> {
        self.selected_channel_id.clone()
    }

    pub fn active_guild_id(&self) -> Option<String> {
        match &self.selected_server {
            ServerSelection::DirectMessages => None,
            ServerSelection::Guild(id) => Some(id.clone()),
        }
    }

    pub fn active_channel_is_text(&self) -> bool {
        self.active_channel()
            .map(|channel| {
                matches!(
                    channel.channel_type(),
                    CHANNEL_GUILD_TEXT
                        | CHANNEL_DM
                        | CHANNEL_GROUP_DM
                        | CHANNEL_DM_PERSONAL_NOTES
                        | CHANNEL_GUILD_LINK
                )
            })
            .unwrap_or(false)
    }

    pub fn active_channel_is_voice(&self) -> bool {
        self.active_channel()
            .map(|channel| channel.channel_type() == CHANNEL_GUILD_VOICE)
            .unwrap_or(false)
    }

    pub fn active_channel_is_link(&self) -> bool {
        self.active_channel()
            .map(|channel| channel.channel_type() == CHANNEL_GUILD_LINK)
            .unwrap_or(false)
    }

    pub fn active_channel_permissions(&self) -> u64 {
        let Some(channel) = self.active_channel() else {
            return u64::MAX; // DMs default to full
        };
        let Some(guild_id) = channel.guild_id.as_deref() else {
            return u64::MAX; // DMs/group DMs
        };

        let guild = self.guilds.iter().find(|g| g.id == guild_id);
        let guild_base = guild
            .and_then(|g| g.permissions.as_deref())
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(0);
        let owner_id = guild.map(|g| g.owner_id.as_str()).unwrap_or("");

        let member_roles = self
            .guild_members
            .get(guild_id)
            .and_then(|members| members.iter().find(|m| m.user.id == self.me.id))
            .map(|m| m.roles.clone())
            .unwrap_or_default();

        crate::permissions::compute_channel_permissions(
            &self.me.id,
            &member_roles,
            guild_id,
            owner_id,
            guild_base,
            &channel.permission_overwrites,
        )
    }

    pub fn can_send_in_active_channel(&self) -> bool {
        let p = self.active_channel_permissions();
        p & crate::permissions::VIEW_CHANNEL != 0 && p & crate::permissions::SEND_MESSAGES != 0
    }

    pub fn active_messages(&self) -> Vec<MessageResponse> {
        let Some(channel_id) = self.selected_channel_id.as_deref() else {
            return Vec::new();
        };
        let mut messages = self.messages.get(channel_id).cloned().unwrap_or_default();
        messages.sort_by_key(|message| snowflake_sort_key(&message.id));
        messages
    }

    pub fn scroll_messages_up(&mut self, amount: u16) {
        self.message_scroll_from_bottom = self.message_scroll_from_bottom.saturating_add(amount);
    }

    pub fn scroll_messages_down(&mut self, amount: u16) {
        self.message_scroll_from_bottom = self.message_scroll_from_bottom.saturating_sub(amount);
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    pub fn normalize_selection(&mut self) {
        let available_servers = self.server_entries();
        if !available_servers.contains(&self.selected_server) {
            self.selected_server = available_servers
                .first()
                .cloned()
                .unwrap_or(ServerSelection::DirectMessages);
        }

        let channels = self.channel_entries();
        if channels.is_empty() {
            self.selected_channel_id = None;
            return;
        }

        let selected_exists = self
            .selected_channel_id
            .as_deref()
            .map(|selected| channels.iter().any(|channel| channel.id == selected))
            .unwrap_or(false);

        if !selected_exists {
            self.selected_channel_id = channels
                .iter()
                .find(|c| c.channel_type() != CHANNEL_GUILD_CATEGORY)
                .map(|channel| channel.id.clone());
            self.message_scroll_from_bottom = 0;
        }
    }

    pub fn upsert_guild(&mut self, guild: GuildResponse) {
        if guild.id.is_empty() {
            return;
        }
        if let Some(existing) = self
            .guilds
            .iter_mut()
            .find(|existing| existing.id == guild.id)
        {
            let preserved_perms = existing.permissions.clone();
            let preserved_name = existing.name.clone();
            let preserved_owner = existing.owner_id.clone();
            *existing = guild;
            if existing.permissions.is_none() {
                existing.permissions = preserved_perms;
            }
            if existing.name.is_empty() {
                existing.name = preserved_name;
            }
            if existing.owner_id.is_empty() {
                existing.owner_id = preserved_owner;
            }
        } else {
            self.guilds.push(guild);
        }
        self.normalize_selection();
    }

    pub fn remove_guild(&mut self, guild_id: &str) {
        self.guilds.retain(|guild| guild.id != guild_id);
        self.guild_channels.remove(guild_id);
        self.guild_members.remove(guild_id);
        self.guild_members_synced.remove(guild_id);
        self.api_backoff_clear_guild(guild_id);
        self.guild_emojis.remove(guild_id);
        self.guild_roles.remove(guild_id);
        self.voice_states.remove(guild_id);
        self.normalize_selection();
    }

    pub fn set_private_channels(&mut self, channels: Vec<ChannelResponse>) {
        merge_user_cache(
            &mut self.user_cache,
            channels
                .iter()
                .flat_map(|channel| channel.recipients.clone()),
        );
        self.private_channels = channels;
        self.normalize_selection();
    }

    pub fn upsert_private_channel(&mut self, channel: ChannelResponse) {
        if let Some(existing) = self
            .private_channels
            .iter_mut()
            .find(|existing| existing.id == channel.id)
        {
            *existing = channel;
        } else {
            self.private_channels.push(channel);
        }
        self.normalize_selection();
    }

    pub fn remove_private_channel(&mut self, channel_id: &str) {
        self.private_channels
            .retain(|channel| channel.id != channel_id);
        self.normalize_selection();
    }

    pub fn set_guild_channels(&mut self, guild_id: &str, channels: Vec<ChannelResponse>) {
        merge_user_cache(
            &mut self.user_cache,
            channels
                .iter()
                .flat_map(|channel| channel.recipients.clone()),
        );
        self.guild_channels.insert(guild_id.to_string(), channels);
        self.loading_channels.remove(guild_id);
        self.api_backoff_clear(&format!("channels:{guild_id}"));
        self.normalize_selection();
    }

    pub fn upsert_channel(&mut self, channel: ChannelResponse) {
        if let Some(guild_id) = channel.guild_id.clone() {
            let entries = self.guild_channels.entry(guild_id).or_default();
            if let Some(existing) = entries
                .iter_mut()
                .find(|existing| existing.id == channel.id)
            {
                *existing = channel;
            } else {
                entries.push(channel);
            }
        } else {
            self.upsert_private_channel(channel);
            return;
        }
        self.normalize_selection();
    }

    pub fn remove_channel(&mut self, channel: &ChannelResponse) {
        if let Some(guild_id) = channel.guild_id.as_deref() {
            if let Some(entries) = self.guild_channels.get_mut(guild_id) {
                entries.retain(|entry| entry.id != channel.id);
            }
        } else {
            self.remove_private_channel(&channel.id);
            return;
        }
        self.normalize_selection();
    }

    pub fn set_guild_members(&mut self, guild_id: &str, members: Vec<GuildMemberResponse>) {
        merge_user_cache(
            &mut self.user_cache,
            members.iter().map(|member| member.user.clone()),
        );
        self.guild_members.insert(guild_id.to_string(), members);
        self.loading_members.remove(guild_id);
        self.api_backoff_clear(&format!("members:{guild_id}"));
    }

    pub fn upsert_message(&mut self, message: MessageResponse) {
        if message.channel_id.is_empty() {
            return;
        }
        merge_user_cache(&mut self.user_cache, [message.author.clone()]);

        let channel_id = message.channel_id.clone();
        let entries = self.messages.entry(channel_id).or_default();
        if let Some(existing) = entries
            .iter_mut()
            .find(|existing| existing.id == message.id)
        {
            *existing = message;
        } else {
            entries.push(message);
        }
        entries.sort_by_key(|entry| snowflake_sort_key(&entry.id));
    }

    pub fn set_channel_messages(&mut self, channel_id: &str, mut messages: Vec<MessageResponse>) {
        for message in &messages {
            merge_user_cache(&mut self.user_cache, [message.author.clone()]);
        }
        messages.sort_by_key(|message| snowflake_sort_key(&message.id));
        self.messages.insert(channel_id.to_string(), messages);
        self.loading_messages.remove(channel_id);
        self.api_backoff_clear(&format!("messages:{channel_id}"));
        self.message_scroll_from_bottom = 0;
    }

    pub fn remove_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages.get_mut(channel_id) {
            messages.retain(|message| message.id != message_id);
        }
    }

    pub fn update_voice_state(&mut self, state: VoiceStateResponse) {
        let Some(guild_id) = state.guild_id.clone() else {
            return;
        };

        if let Some(member) = state.member.clone() {
            let members = self.guild_members.entry(guild_id.clone()).or_default();
            if let Some(existing) = members
                .iter_mut()
                .find(|existing| existing.user.id == member.user.id)
            {
                *existing = member.clone();
            } else {
                members.push(member.clone());
            }
            merge_user_cache(&mut self.user_cache, [member.user]);
        }

        let guild_states = self.voice_states.entry(guild_id).or_default();
        if state.channel_id.is_none() {
            guild_states.remove(&state.user_id);
        } else {
            guild_states.insert(state.user_id.clone(), state);
        }
    }

    pub fn voice_members_for_active_channel(&self) -> Vec<String> {
        let Some(channel) = self.active_channel() else {
            return Vec::new();
        };
        let Some(guild_id) = channel.guild_id else {
            return Vec::new();
        };
        let Some(states) = self.voice_states.get(&guild_id) else {
            return Vec::new();
        };

        let mut members = states
            .values()
            .filter(|state| state.channel_id.as_deref() == Some(channel.id.as_str()))
            .map(|state| {
                let name = state
                    .member
                    .as_ref()
                    .and_then(|member| member.nick.clone())
                    .or_else(|| self.user_cache.get(&state.user_id).map(display_name))
                    .unwrap_or_else(|| state.user_id.clone());

                let mut badges = Vec::new();
                if state.self_mute {
                    badges.push("self-muted");
                }
                if state.self_deaf {
                    badges.push("self-deaf");
                }
                if state.self_stream {
                    badges.push("streaming");
                }
                if state.self_video {
                    badges.push("video");
                }

                if badges.is_empty() {
                    name
                } else {
                    format!("{name} ({})", badges.join(", "))
                }
            })
            .collect::<Vec<_>>();
        members.sort();
        members
    }
    pub fn start_emoji_autocomplete(&mut self) {
        self.emoji_autocomplete = Some(EmojiAutocomplete {
            matches: Vec::new(),
            selected_index: 0,
        });
        self.update_emoji_filter();
    }

    pub fn update_emoji_filter(&mut self) {
        if self.emoji_autocomplete.is_none() {
            return;
        }

        let query = self.input.rsplit(':').next().unwrap_or("").to_lowercase();
        let mut results: Vec<EmojiMatch> = Vec::new();

        // guild custom emojis first
        let guild_emojis: Vec<crate::api::types::GuildEmojiResponse> = self
            .active_guild_id()
            .and_then(|gid| self.guild_emojis.get(&gid))
            .cloned()
            .unwrap_or_default();

        for e in &guild_emojis {
            if query.is_empty() || e.name.to_lowercase().contains(&query) {
                let prefix = if e.animated { "a" } else { "" };
                results.push(EmojiMatch {
                    label: format!(":{}:", e.name),
                    insert: format!("<{}:{}:{}>", prefix, e.name, e.id),
                    is_custom: true,
                });
            }
            if results.len() >= 12 {
                break;
            }
        }

        // standard unicode emojis
        if results.len() < 12 {
            for emoji in emojis::iter() {
                if results.len() >= 12 {
                    break;
                }
                let name = emoji.name().to_lowercase();
                let shortcode = emoji.shortcode().unwrap_or("");
                if query.is_empty()
                    || name.contains(&query)
                    || shortcode.to_lowercase().contains(&query)
                {
                    let label_code = if !shortcode.is_empty() {
                        format!("{} :{shortcode}:", emoji.as_str())
                    } else {
                        format!("{} {}", emoji.as_str(), name)
                    };
                    results.push(EmojiMatch {
                        label: label_code,
                        insert: emoji.as_str().to_string(),
                        is_custom: false,
                    });
                }
            }
            results.truncate(12);
        }

        let auto = self.emoji_autocomplete.as_mut().unwrap();
        auto.matches = results;
        if auto.selected_index >= auto.matches.len() {
            auto.selected_index = auto.matches.len().saturating_sub(1);
        }
        if auto.matches.is_empty() {
            self.emoji_autocomplete = None;
        }
    }

    pub fn dismiss_emoji_autocomplete(&mut self) {
        self.emoji_autocomplete = None;
    }

    pub fn autocomplete_emoji_next(&mut self) {
        if let Some(auto) = &mut self.emoji_autocomplete
            && !auto.matches.is_empty()
        {
            auto.selected_index = (auto.selected_index + 1) % auto.matches.len();
        }
    }

    pub fn autocomplete_emoji_prev(&mut self) {
        if let Some(auto) = &mut self.emoji_autocomplete
            && !auto.matches.is_empty()
        {
            auto.selected_index =
                auto.selected_index.saturating_add(auto.matches.len() - 1) % auto.matches.len();
        }
    }

    pub fn insert_selected_emoji(&mut self) -> bool {
        if let Some(auto) = &self.emoji_autocomplete
            && let Some(emoji) = auto.matches.get(auto.selected_index)
            && let Some(colon_pos) = self.input.rfind(':') {
                self.input.truncate(colon_pos);
                self.input.push_str(&emoji.insert);
                self.input.push(' ');
                self.emoji_autocomplete = None;
                return true;
            }
        false
    }

    // rs

    pub fn set_read_states(&mut self, states: Vec<ReadStateResponse>) {
        for s in states {
            if !s.id.is_empty() {
                self.read_states.insert(
                    s.id,
                    ReadState {
                        last_message_id: s.last_message_id,
                        mention_count: s.mention_count,
                    },
                );
            }
        }
    }

    pub fn ack_channel(&mut self, channel_id: &str) {
        let last_msg = self
            .messages
            .get(channel_id)
            .and_then(|msgs| msgs.last())
            .map(|m| m.id.clone());
        if let Some(msg_id) = last_msg {
            self.read_states.insert(
                channel_id.to_string(),
                ReadState {
                    last_message_id: Some(msg_id),
                    mention_count: 0,
                },
            );
        }
    }

    pub fn channel_is_unread(&self, channel_id: &str) -> bool {
        let Some(rs) = self.read_states.get(channel_id) else {
            return false;
        };
        let channel_last = self.channel_last_message_id(channel_id);
        match (&rs.last_message_id, &channel_last) {
            (Some(read), Some(last)) => snowflake_sort_key(read) < snowflake_sort_key(last),
            (None, Some(_)) => true,
            _ => false,
        }
    }

    pub fn channel_mention_count(&self, channel_id: &str) -> u64 {
        self.read_states
            .get(channel_id)
            .map(|rs| rs.mention_count)
            .unwrap_or(0)
    }

    fn channel_last_message_id(&self, channel_id: &str) -> Option<String> {
        // check from cached messages first -> try from channel metadata
        if let Some(msgs) = self.messages.get(channel_id)
            && let Some(last) = msgs.last() {
                return Some(last.id.clone());
            }
        // falls back (ah myback!)
        let all_channels: Vec<&ChannelResponse> = self
            .private_channels
            .iter()
            .chain(self.guild_channels.values().flat_map(|v| v.iter()))
            .collect();
        all_channels
            .iter()
            .find(|c| c.id == channel_id)
            .and_then(|c| c.last_message_id.clone())
    }

    // ms

    pub fn move_selected_message(&mut self, delta: i32) {
        let count = self.active_messages().len();
        if count == 0 {
            self.selected_message_index = None;
            return;
        }
        let current = self
            .selected_message_index
            .unwrap_or(count.saturating_sub(1));
        let next = (current as i32 + delta).clamp(0, count as i32 - 1) as usize;
        self.selected_message_index = Some(next);
    }

    pub fn selected_message(&self) -> Option<MessageResponse> {
        let msgs = self.active_messages();
        self.selected_message_index
            .and_then(|i| msgs.get(i).cloned())
    }

    // r (as in reply)

    pub fn start_reply(&mut self) {
        if let Some(msg) = self.selected_message() {
            self.reply_to = Some(ReplyState {
                channel_id: msg.channel_id.clone(),
                message_id: msg.id.clone(),
                author_name: display_name(&msg.author),
            });
            self.focus = Focus::Input;
        }
    }

    pub fn cancel_reply(&mut self) {
        self.reply_to = None;
    }

    // ma

    pub fn start_mention_autocomplete(&mut self) {
        let pool = self.build_mention_pool();
        if pool.is_empty() {
            return;
        }
        self.mention_autocomplete = Some(MentionAutocomplete {
            pool,
            matches: Vec::new(),
            selected_index: 0,
        });
        self.update_mention_filter();
    }

    /// repopulate @ UI if it was waiting
    pub fn refresh_mention_autocomplete_after_members_load(&mut self, loaded_guild_id: &str) {
        if self.mention_autocomplete.is_none() {
            return;
        }
        if self.active_guild_id().as_deref() != Some(loaded_guild_id) {
            return;
        }
        let pool = self.build_mention_pool();
        if pool.is_empty() {
            self.mention_autocomplete = None;
            return;
        }
        if let Some(auto) = &mut self.mention_autocomplete {
            auto.pool = pool;
            auto.selected_index = 0;
        }
        self.update_mention_filter();
        if self.mention_autocomplete.is_some() {
            self.set_status("");
        }
    }

    const MENTION_FILTER_CAP: usize = 400;
    const MENTION_INITIAL_CAP: usize = 80;

    fn build_mention_pool(&self) -> Vec<MentionPick> {
        let mut users: Vec<MentionPick> = Vec::new();
        let mut roles: Vec<MentionPick> = Vec::new();
        let mut seen_users: HashSet<String> = HashSet::new();

        match &self.selected_server {
            ServerSelection::Guild(guild_id) => {
                if let Some(members) = self.guild_members.get(guild_id) {
                    for m in members {
                        if m.user.id.is_empty() || !seen_users.insert(m.user.id.clone()) {
                            continue;
                        }
                        let cached_user = self.user_cache.get(&m.user.id);
                        let username = if !m.user.username.is_empty() {
                            m.user.username.clone()
                        } else {
                            cached_user.map(|u| u.username.clone()).unwrap_or_default()
                        };
                        let base_display = if !m.user.username.is_empty() {
                            display_name(&m.user)
                        } else {
                            cached_user.map(display_name).unwrap_or_default()
                        };
                        let nick_display = m
                            .nick
                            .clone()
                            .filter(|n| !n.trim().is_empty())
                            .unwrap_or(base_display);
                        if username.is_empty() && nick_display.is_empty() {
                            continue;
                        }
                        users.push(MentionPick::User {
                            user_id: m.user.id.clone(),
                            display: nick_display,
                            username,
                        });
                    }
                }
                if let Some(rs) = self.guild_roles.get(guild_id) {
                    let mut seen_roles: HashSet<String> = HashSet::new();
                    for r in rs {
                        if r.id.is_empty() || !seen_roles.insert(r.id.clone()) {
                            continue;
                        }
                        let name = if r.name.trim().is_empty() {
                            continue;
                        } else {
                            r.name.clone()
                        };
                        roles.push(MentionPick::Role {
                            role_id: r.id.clone(),
                            name,
                            color: r.color,
                        });
                    }
                }
                roles.sort_by(|a, b| match (a, b) {
                    (MentionPick::Role { name: na, .. }, MentionPick::Role { name: nb, .. }) => {
                        na.to_lowercase().cmp(&nb.to_lowercase())
                    }
                    _ => std::cmp::Ordering::Equal,
                });
            }
            ServerSelection::DirectMessages => {
                if let Some(ch) = self.active_channel() {
                    for r in &ch.recipients {
                        if r.id.is_empty() || !seen_users.insert(r.id.clone()) {
                            continue;
                        }
                        users.push(MentionPick::User {
                            user_id: r.id.clone(),
                            display: display_name(r),
                            username: r.username.clone(),
                        });
                    }
                }
            }
        }

        if let Some(channel_id) = self.selected_channel_id.as_deref() {
            let guild_id = self.active_guild_id();
            if let Some(msgs) = self.messages.get(channel_id) {
                for msg in msgs {
                    if msg.author.id.is_empty() || !seen_users.insert(msg.author.id.clone()) {
                        continue;
                    }
                    let nick = guild_id.as_ref().and_then(|gid| {
                        self.guild_members.get(gid).and_then(|mems| {
                            mems.iter()
                                .find(|m| m.user.id == msg.author.id)
                                .and_then(|m| m.nick.clone())
                        })
                    });
                    let base_display = display_name(&msg.author);
                    let display = nick
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or(base_display.clone());
                    let username = if !msg.author.username.is_empty() {
                        msg.author.username.clone()
                    } else {
                        self.user_cache
                            .get(&msg.author.id)
                            .map(|u| u.username.clone())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| base_display.clone())
                    };
                    if username.is_empty() && display.is_empty() {
                        continue;
                    }
                    users.push(MentionPick::User {
                        user_id: msg.author.id.clone(),
                        display,
                        username,
                    });
                }
            }
        }

        users.sort_by(|a, b| match (a, b) {
            (MentionPick::User { display: da, .. }, MentionPick::User { display: db, .. }) => {
                da.to_lowercase().cmp(&db.to_lowercase())
            }
            _ => std::cmp::Ordering::Equal,
        });

        let mut pool = users;
        pool.extend(roles);
        pool
    }

    pub fn update_mention_filter(&mut self) {
        let Some(auto) = &mut self.mention_autocomplete else {
            return;
        };

        let query = self.input.rsplit('@').next().unwrap_or("").to_lowercase();

        auto.matches = if query.is_empty() {
            (0..auto.pool.len().min(Self::MENTION_INITIAL_CAP)).collect()
        } else {
            auto.pool
                .iter()
                .enumerate()
                .filter(|(_, p)| p.matches_filter(&query))
                .map(|(i, _)| i)
                .take(Self::MENTION_FILTER_CAP)
                .collect()
        };

        if auto.selected_index >= auto.matches.len() {
            auto.selected_index = auto.matches.len().saturating_sub(1);
        }
        if auto.matches.is_empty() {
            self.mention_autocomplete = None;
        }
    }

    pub fn dismiss_mention_autocomplete(&mut self) {
        self.mention_autocomplete = None;
    }

    pub fn autocomplete_mention_next(&mut self) {
        if let Some(auto) = &mut self.mention_autocomplete
            && !auto.matches.is_empty()
        {
            auto.selected_index = (auto.selected_index + 1) % auto.matches.len();
        }
    }

    pub fn autocomplete_mention_prev(&mut self) {
        if let Some(auto) = &mut self.mention_autocomplete
            && !auto.matches.is_empty()
        {
            auto.selected_index =
                auto.selected_index.saturating_add(auto.matches.len() - 1) % auto.matches.len();
        }
    }

    pub fn insert_selected_mention(&mut self) -> bool {
        if let Some(auto) = &self.mention_autocomplete
            && let Some(&pool_idx) = auto.matches.get(auto.selected_index)
            && let Some(pick) = auto.pool.get(pool_idx)
            && let Some(at_pos) = self.input.rfind('@')
        {
            self.input.truncate(at_pos);
            match pick {
                MentionPick::User { user_id, .. } => {
                    self.input.push_str(&format!("<@{user_id}> "));
                }
                MentionPick::Role { role_id, .. } => {
                    self.input.push_str(&format!("<@&{role_id}> "));
                }
            }
            self.mention_autocomplete = None;
            return true;
        }
        false
    }

    // permission(orn) helpers

    pub fn can_react_in_active_channel(&self) -> bool {
        self.active_channel_permissions() & crate::permissions::ADD_REACTIONS != 0
    }
}

pub fn me_as_partial(me: &UserPrivateResponse) -> UserPartialResponse {
    UserPartialResponse {
        id: me.id.clone(),
        username: me.username.clone(),
        discriminator: me.discriminator.clone(),
        global_name: me.global_name.clone(),
        avatar: me.avatar.clone(),
        bot: me.bot,
        system: me.system,
    }
}

pub fn display_name(user: &UserPartialResponse) -> String {
    user.global_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            if user.discriminator.is_empty() {
                user.username.clone()
            } else {
                format!("{}#{}", user.username, user.discriminator)
            }
        })
}
