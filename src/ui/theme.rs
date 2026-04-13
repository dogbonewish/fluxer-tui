use ratatui::style::{Color, Modifier, Style};
use std::hash::{Hash, Hasher};

pub const BG: Color = Color::Rgb(30, 31, 34);
pub const BG_SECONDARY: Color = Color::Rgb(43, 45, 49);
pub const BG_TERTIARY: Color = Color::Rgb(32, 34, 37);
pub const ACCENT: Color = Color::Rgb(88, 101, 242);
pub const ACCENT_DIM: Color = Color::Rgb(71, 82, 196);
pub const VOICE_COLOR: Color = Color::Rgb(87, 242, 135);
pub const TEXT: Color = Color::Rgb(219, 222, 225);
pub const TEXT_DIM: Color = Color::Rgb(148, 155, 164);
pub const TEXT_MUTED: Color = Color::Rgb(94, 103, 114);
pub const EMOJI_UNKNOWN: Color = Color::Rgb(254, 231, 92);
pub const LINK_COLOR: Color = Color::Rgb(0, 168, 252);
pub const DANGER: Color = Color::Rgb(237, 66, 69);
/// Others typing (input bar title) — not `TEXT_MUTED` so it doesn’t match the empty placeholder.
pub const TYPING_OTHERS: Color = Color::Rgb(114, 218, 167);

pub const USERNAME_COLORS: [Color; 12] = [
    Color::Rgb(235, 69, 158),
    Color::Rgb(237, 66, 69),
    Color::Rgb(241, 196, 15),
    Color::Rgb(46, 204, 113),
    Color::Rgb(26, 188, 156),
    Color::Rgb(52, 152, 219),
    Color::Rgb(155, 89, 182),
    Color::Rgb(230, 126, 34),
    Color::Rgb(173, 20, 87),
    Color::Rgb(0, 131, 143),
    Color::Rgb(156, 204, 101),
    Color::Rgb(216, 67, 21),
];

pub fn username_color(id: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();
    USERNAME_COLORS[(hash as usize) % USERNAME_COLORS.len()]
}

pub fn self_username_color() -> Color {
    Color::Rgb(0, 229, 255)
}

pub fn rgb_pack_to_color(packed: u32) -> Color {
    Color::Rgb(
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
    )
}

pub fn role_mention_style(color: u32) -> Style {
    if color == 0 {
        Style::default()
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(rgb_pack_to_color(color))
    }
}

pub fn focused_border(focused: bool) -> Style {
    if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(TEXT_MUTED)
    }
}

pub fn gateway_status_style(status: crate::app::GatewayStatus) -> Style {
    use crate::app::GatewayStatus::*;
    match status {
        Connecting | Reconnecting => Style::default().fg(Color::Yellow),
        Connected => Style::default().fg(VOICE_COLOR),
        Disconnected => Style::default().fg(DANGER),
    }
}
