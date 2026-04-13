use crate::api::types::UserPartialResponse;
use crate::permissions::{CHANGE_NICKNAME, SEND_TTS_MESSAGES};

pub const FLUXERBOT_ID: &str = "0";

pub const MESSAGE_TYPE_CLIENT_SYSTEM: i32 = 99;

#[derive(Debug, Clone, Copy)]
pub struct SlashCommandDef {
    pub name: &'static str,
    pub description: &'static str,
    pub simple_append: Option<&'static str>,
    pub requires_guild: bool,
    pub requires_channel_perm: Option<u64>,
}

pub static SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef {
        name: "/shrug",
        description: "Appends ¯\\_(ツ)_/¯ to your message.",
        simple_append: Some("¯\\_(ツ)_/¯"),
        requires_guild: false,
        requires_channel_perm: None,
    },
    SlashCommandDef {
        name: "/tableflip",
        description: "Appends (╯°□°)╯︵ ┻━┻ to your message.",
        simple_append: Some("(╯°□°)╯︵ ┻━┻"),
        requires_guild: false,
        requires_channel_perm: None,
    },
    SlashCommandDef {
        name: "/unflip",
        description: "Appends ┬─┬ ノ( ゜-゜ノ) to your message.",
        simple_append: Some("┬─┬ ノ( ゜-゜ノ)"),
        requires_guild: false,
        requires_channel_perm: None,
    },
    SlashCommandDef {
        name: "/me",
        description: "Send an action message (wraps in italics).",
        simple_append: None,
        requires_guild: false,
        requires_channel_perm: None,
    },
    SlashCommandDef {
        name: "/spoiler",
        description: "Send a spoiler message (wraps in spoiler tags).",
        simple_append: None,
        requires_guild: false,
        requires_channel_perm: None,
    },
    SlashCommandDef {
        name: "/tts",
        description: "Send a text-to-speech message.",
        simple_append: None,
        requires_guild: false,
        requires_channel_perm: Some(SEND_TTS_MESSAGES),
    },
    SlashCommandDef {
        name: "/nick",
        description: "Change your nickname in this community.",
        simple_append: None,
        requires_guild: true,
        requires_channel_perm: Some(CHANGE_NICKNAME),
    },
];

pub fn command_name_query(input: &str) -> Option<&str> {
    let line = input.lines().next()?.trim_start();
    let rest = line.strip_prefix('/')?;
    if rest.contains(' ') {
        return None;
    }
    Some(rest)
}

pub fn visible_commands(
    guild_channel: bool,
    channel_perms: u64,
) -> impl Iterator<Item = (usize, &'static SlashCommandDef)> {
    SLASH_COMMANDS.iter().enumerate().filter(move |(_, c)| {
        if c.requires_guild && !guild_channel {
            return false;
        }
        if let Some(bit) = c.requires_channel_perm {
            if channel_perms & bit == 0 {
                return false;
            }
        }
        true
    })
}

pub fn filter_command_indices(query: &str, guild_channel: bool, channel_perms: u64) -> Vec<usize> {
    let q = query.to_lowercase();
    visible_commands(guild_channel, channel_perms)
        .filter(|(_, c)| {
            let name = c.name.trim_start_matches('/').to_lowercase();
            q.is_empty() || name.starts_with(&q)
        })
        .map(|(i, _)| i)
        .take(24)
        .collect()
}

#[derive(Debug, Clone)]
pub enum OutgoingSlash {
    SendContent(String),
    SendTts(String),
    SetNick {
        guild_id: String,
        nick: Option<String>,
        prev_display: String,
        new_display: String,
    },
    Blocked(String),
    Normal,
}

pub fn resolve_outgoing_slash(
    trimmed: &str,
    guild_id: Option<&str>,
    me_username: &str,
    prev_nick_or_username: &str,
    channel_perms: u64,
) -> OutgoingSlash {
    let t = trimmed;
    for c in SLASH_COMMANDS {
        if let Some(content) = c.simple_append
            && t == c.name
        {
            return OutgoingSlash::SendContent(content.to_string());
        }
    }
    if t == "/me" {
        return OutgoingSlash::Blocked("Add text after /me (e.g. /me waves).".to_string());
    }
    if let Some(rest) = t.strip_prefix("/me ") {
        let body = rest.trim_end();
        if body.is_empty() {
            return OutgoingSlash::Blocked("Add text after /me (e.g. /me waves).".to_string());
        }
        return OutgoingSlash::SendContent(format!("_{body}_"));
    }
    if t == "/spoiler" {
        return OutgoingSlash::Blocked("Add text after /spoiler.".to_string());
    }
    if let Some(rest) = t.strip_prefix("/spoiler ") {
        let body = rest.trim_end();
        if body.is_empty() {
            return OutgoingSlash::Blocked("Add text after /spoiler.".to_string());
        }
        return OutgoingSlash::SendContent(format!("||{body}||"));
    }
    if t == "/tts" {
        return OutgoingSlash::Blocked("Add text after /tts.".to_string());
    }
    if let Some(rest) = t.strip_prefix("/tts ") {
        let body = rest.trim_end();
        if body.is_empty() {
            return OutgoingSlash::Blocked("Add text after /tts.".to_string());
        }
        if channel_perms & SEND_TTS_MESSAGES == 0 {
            return OutgoingSlash::Blocked(
                "You don’t have permission to send text-to-speech messages here.".to_string(),
            );
        }
        return OutgoingSlash::SendTts(body.to_string());
    }
    let nick_arg = if t == "/nick" {
        Some(String::new())
    } else if let Some(rest) = t.strip_prefix("/nick ") {
        Some(rest.to_string())
    } else {
        None
    };
    if let Some(arg_raw) = nick_arg {
        let guild_id = match guild_id {
            Some(g) => g.to_string(),
            None => {
                return OutgoingSlash::Blocked(
                    "You can only change your nickname in a server.".to_string(),
                );
            }
        };
        if channel_perms & CHANGE_NICKNAME == 0 {
            return OutgoingSlash::Blocked(
                "You can’t change your nickname in this channel.".to_string(),
            );
        }
        let arg = arg_raw.trim();
        let nick_opt = if arg.is_empty() {
            None
        } else {
            Some(arg.to_string())
        };
        let new_display = nick_opt
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(me_username)
            .to_string();
        return OutgoingSlash::SetNick {
            guild_id,
            nick: nick_opt,
            prev_display: prev_nick_or_username.to_string(),
            new_display,
        };
    }
    OutgoingSlash::Normal
}

pub fn nick_change_system_markdown(prev: &str, new: &str) -> String {
    format!("You changed your nickname in this community from **{prev}** to **{new}**.")
}

pub fn fluxerbot_author() -> UserPartialResponse {
    UserPartialResponse {
        id: FLUXERBOT_ID.to_string(),
        username: "Fluxerbot".to_string(),
        discriminator: "0000".to_string(),
        global_name: None,
        avatar: None,
        bot: true,
        system: true,
    }
}
