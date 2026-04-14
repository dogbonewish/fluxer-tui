use crate::api::types::PermissionOverwrite;

pub const ADMINISTRATOR: u64 = 0x8;
pub const ADD_REACTIONS: u64 = 0x40;
pub const VIEW_CHANNEL: u64 = 0x400;
pub const SEND_MESSAGES: u64 = 0x800;
pub const SEND_TTS_MESSAGES: u64 = 0x1000;
pub const MANAGE_MESSAGES: u64 = 0x2000;
pub const CHANGE_NICKNAME: u64 = 0x4000000;

/// Kept in sync with the API
#[allow(dead_code)]
pub const READ_MESSAGE_HISTORY: u64 = 0x10000;
pub const MENTION_EVERYONE: u64 = 0x20000;

const OVERWRITE_ROLE: i32 = 0;
const OVERWRITE_MEMBER: i32 = 1;

/// isreali GPT was here... Beep Boop. (joke)

pub fn compute_channel_permissions(
    user_id: &str,
    member_roles: &[String],
    guild_id: &str,
    guild_owner_id: &str,
    guild_base_permissions: u64,
    overwrites: &[PermissionOverwrite],
) -> u64 {
    if user_id == guild_owner_id {
        return u64::MAX;
    }

    let mut perms = guild_base_permissions;

    if perms & ADMINISTRATOR != 0 {
        return u64::MAX;
    }

    // @everyone overwrite
    if let Some(ow) = overwrites
        .iter()
        .find(|o| o.kind == OVERWRITE_ROLE && o.id == guild_id)
    {
        let allow = ow.allow.parse::<u64>().unwrap_or(0);
        let deny = ow.deny.parse::<u64>().unwrap_or(0);
        perms &= !deny;
        perms |= allow;
    }

    let mut role_allow: u64 = 0;
    let mut role_deny: u64 = 0;
    for ow in overwrites
        .iter()
        .filter(|o| o.kind == OVERWRITE_ROLE && o.id != guild_id)
    {
        if member_roles.iter().any(|r| r == &ow.id) {
            role_allow |= ow.allow.parse::<u64>().unwrap_or(0);
            role_deny |= ow.deny.parse::<u64>().unwrap_or(0);
        }
    }
    perms &= !role_deny;
    perms |= role_allow;

    // member-specific
    if let Some(ow) = overwrites
        .iter()
        .find(|o| o.kind == OVERWRITE_MEMBER && o.id == user_id)
    {
        let allow = ow.allow.parse::<u64>().unwrap_or(0);
        let deny = ow.deny.parse::<u64>().unwrap_or(0);
        perms &= !deny;
        perms |= allow;
    }

    if perms & ADMINISTRATOR != 0 {
        return u64::MAX;
    }

    perms
}
