use crate::api::types::{
    MESSAGE_NOTIFICATIONS_ALL_MESSAGES, MESSAGE_NOTIFICATIONS_NO_MESSAGES,
    MESSAGE_NOTIFICATIONS_ONLY_MENTIONS,
};
use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

const MUTE_LABELS: [&str; 7] = [
    "Off",
    "15 minutes",
    "1 hour",
    "3 hours",
    "8 hours",
    "24 hours",
    "Until I turn it back on",
];

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(Clear, area);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(42),
            Constraint::Length(2),
        ])
        .split(outer[1]);

    let popup = mid[1];
    frame.render_widget(Clear, popup);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(popup);

    let content = body[0];
    let accent = Style::default()
        .fg(crate::ui::theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let accent_soft = Style::default().fg(crate::ui::theme::ACCENT_DIM);
    let text = Style::default().fg(crate::ui::theme::TEXT);
    let dim = Style::default().fg(crate::ui::theme::TEXT_DIM);
    let muted = Style::default().fg(crate::ui::theme::TEXT_MUTED);
    let strong = Style::default()
        .fg(crate::ui::theme::TEXT)
        .add_modifier(Modifier::BOLD);

    let Some(settings) = app.selected_server_notification_settings() else {
        let paragraph = Paragraph::new("Select a community to edit its notification settings.")
            .block(
                Block::default()
                    .title(Line::from(Span::styled(" Notifications ", accent)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM)),
            )
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, content);
        return;
    };

    let current_level = app
        .selected_server_guild_id()
        .map(|guild_id| app.resolved_message_notifications_for_guild(&guild_id))
        .unwrap_or(MESSAGE_NOTIFICATIONS_ALL_MESSAGES);
    let mute_index = app.current_server_mute_choice_index().unwrap_or(0);
    let timed_mute = settings
        .mute_config
        .as_ref()
        .and_then(|config| config.end_time.as_deref())
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|ts| {
            ts.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string()
        });

    let rows = [
        (
            "Mute notifications",
            MUTE_LABELS[mute_index].to_string(),
            timed_mute.unwrap_or_else(|| {
                "Unread dots are suppressed unless you are mentioned".to_string()
            }),
        ),
        (
            "Notification level",
            match current_level {
                MESSAGE_NOTIFICATIONS_ONLY_MENTIONS => "Only @mentions",
                MESSAGE_NOTIFICATIONS_NO_MESSAGES => "Nothing",
                _ => "All Messages",
            }
            .to_string(),
            "Controls whether unread-only channels still bubble up".to_string(),
        ),
        (
            "Suppress @everyone / @here",
            bool_label(settings.suppress_everyone).to_string(),
            "Role pings can still get through unless role suppression is also on".to_string(),
        ),
        (
            "Suppress role mentions",
            bool_label(settings.suppress_roles).to_string(),
            "Stops role-based pings from counting as mentions".to_string(),
        ),
        (
            "Hide muted channels",
            bool_label(settings.hide_muted_channels).to_string(),
            "Muted channels disappear from the sidebar unless selected".to_string(),
        ),
        (
            "Mobile push",
            bool_label(settings.mobile_push).to_string(),
            "Server-side setting for Fluxer mobile push behavior".to_string(),
        ),
    ];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("  ", text),
        Span::styled(app.selected_server_name(), accent),
        Span::styled(" · notification settings", dim),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  COMMUNITY",
        accent_soft.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  ───────────────────────────────────────────",
        accent_soft,
    )]));
    lines.push(Line::from(""));

    for (index, (label, value, detail)) in rows.iter().enumerate() {
        lines.push(Line::from(vec![Span::styled(format!("  {label}"), dim)]));
        lines.push(Line::from(vec![
            Span::styled("  ", text),
            Span::styled(
                if app.server_notification_cursor == index {
                    "▸ "
                } else {
                    "  "
                },
                if app.server_notification_cursor == index {
                    accent
                } else {
                    muted
                },
            ),
            Span::styled(value.clone(), strong),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    ", text),
            Span::styled(detail.clone(), muted),
        ]));
        if index != rows.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    let block = Block::default()
        .title(Line::from(Span::styled(" Notifications ", accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));

    let paragraph = Paragraph::new(Text::from(lines.clone()))
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    let inner_h = content.height.saturating_sub(2).max(1);
    let line_count = paragraph.line_count(content.width).max(1) as u16;
    let max_scroll = line_count.saturating_sub(inner_h);
    let scroll_y = app.server_notification_scroll.min(max_scroll);

    let mut scrollable = paragraph;
    if max_scroll > 0 {
        scrollable = scrollable.scroll((scroll_y, 0));
    }

    frame.render_widget(scrollable, content);

    let hint_text = if max_scroll > 0 {
        " ↑/↓ move · PgUp/PgDn scroll · ←/→ / Space change · Esc / Enter / q close "
    } else {
        "↑/↓ move  ·  ←/→ / Space change  ·  Esc / Enter / q close"
    };
    let hint = Paragraph::new(Line::from(vec![Span::styled(hint_text, muted)]))
        .alignment(Alignment::Center);
    frame.render_widget(hint, body[1]);
}

fn bool_label(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}
