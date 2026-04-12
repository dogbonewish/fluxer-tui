use crate::app::{App, Focus};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, area: Rect, app: &App) -> Option<(u16, u16)> {
    let can_type = app.active_channel_is_text() && app.can_send_in_active_channel();
    let no_perms = app.active_channel_is_text() && !app.can_send_in_active_channel();
    let voice_only = app.active_channel_is_voice();

    let title = if voice_only {
        "Input (voice not supported)"
    } else if no_perms {
        "Input (no permission)"
    } else if app.forward_mode {
        "Forward (select channel, Enter to send)"
    } else if app.reply_to.is_some() {
        "Reply"
    } else if can_type {
        "Input"
    } else {
        "Input (disabled)"
    };

    let (content, style) = if voice_only {
        (
            "This client cannot join or use voice — text input is disabled here.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    } else if no_perms {
        (
            "You do not have permission to send messages here.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    } else if can_type {
        if app.input.is_empty() {
            let placeholder = if let Some(ref reply) = app.reply_to {
                format!("Replying to {}...", reply.author_name)
            } else {
                "Type a message...".to_string()
            };
            (
                placeholder,
                Style::default().fg(crate::ui::theme::TEXT_MUTED),
            )
        } else {
            (app.input.clone(), Style::default().fg(crate::ui::theme::TEXT))
        }
    } else {
        (
            "Select a text channel to chat.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    };

    let title_line = Line::from(vec![Span::styled(
        format!(" {title} "),
        Style::default().add_modifier(Modifier::BOLD).fg(crate::ui::theme::TEXT_DIM),
    )]);

    let focused = app.focus == Focus::Input;
    let mut blk = Block::default()
        .title(title_line)
        .borders(Borders::ALL)
        .border_style(crate::ui::theme::focused_border(focused))
        .style(Style::default().bg(crate::ui::theme::BG_SECONDARY));

    if can_type && !app.input.is_empty() {
        let char_count = app.input.chars().count();
        let max_chars = 2000;
        let count_str = format!(" {char_count}/{max_chars} ");
        let count_style = if char_count > max_chars {
            Style::default().fg(crate::ui::theme::DANGER).add_modifier(Modifier::BOLD)
        } else if char_count > max_chars - 100 {
            Style::default().fg(ratatui::style::Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(crate::ui::theme::TEXT_MUTED)
        };

        let right_title = Line::from(Span::styled(count_str, count_style))
            .alignment(ratatui::layout::Alignment::Right);
        blk = blk.title(right_title);
    }

    let paragraph = Paragraph::new(Line::from(Span::styled(content, style)))
        .block(blk);
    frame.render_widget(paragraph, area);

    if focused && can_type {
        let text_width = UnicodeWidthStr::width(app.input.as_str()) as u16;
        let x = area.x + 1 + text_width;
        let max_x = area.x + area.width.saturating_sub(2);
        Some((x.min(max_x), area.y + 1))
    } else {
        None
    }
}
