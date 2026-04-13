use crate::app::{App, Focus};
use crate::ui::input_word_wrap;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

fn input_span_style() -> Style {
    Style::default().fg(crate::ui::theme::TEXT)
}

fn typing_line_with_dots(phrase: &str, dots: &str) -> String {
    let base = phrase.strip_suffix("...").unwrap_or(phrase);
    format!("{base}{dots}")
}

pub fn input_display_row_count(app: &App, inner_width: u16) -> u16 {
    let can_type = app.active_channel_is_text() && app.can_send_in_active_channel();
    if !can_type {
        return 1;
    }
    if !app.input.is_empty() {
        return input_word_wrap::wrapped_row_count(&app.input, inner_width, input_span_style());
    }
    let mut n = 1u16;
    if app.others_typing_phrase().is_some() {
        n = n.saturating_add(1);
    }
    n
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) -> Option<(u16, u16)> {
    let can_type = app.active_channel_is_text() && app.can_send_in_active_channel();
    let no_perms = app.active_channel_is_text() && !app.can_send_in_active_channel();
    let voice_only = app.active_channel_is_voice();

    let title = if voice_only {
        "Input (voice not supported)"
    } else if no_perms {
        "Input (no permission)"
    } else if app.edit_target.is_some() {
        "Edit message"
    } else if app.forward_mode {
        "Forward (select channel, Enter to send)"
    } else if app.reply_to.is_some() {
        "Reply"
    } else if can_type {
        "Input"
    } else {
        "Input (disabled)"
    };

    let placeholder: Option<String> = if voice_only || no_perms || !can_type {
        None
    } else if app.input.is_empty() {
        Some(if let Some(ref reply) = app.reply_to {
            if app.forward_mode {
                format!("Forward from {} - optional note, Enter to send", reply.author_name)
            } else {
                format!("Replying to {}...", reply.author_name)
            }
        } else {
            "Type a message…  ( / for commands )".to_string()
        })
    } else {
        None
    };

    let (content, style) = if voice_only {
        (
            "This client cannot join or use voice - text input is disabled here.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    } else if no_perms {
        (
            "You do not have permission to send messages here.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    } else if can_type && !app.input.is_empty() {
        (String::new(), Style::default().fg(crate::ui::theme::TEXT))
    } else if can_type {
        (
            placeholder.clone().unwrap_or_default(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    } else {
        (
            "Select a text channel to chat.".to_string(),
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        )
    };

    let others_typing = app.others_typing_phrase();
    let typing_dots: &'static str = if others_typing.is_some() {
        match app.input_bar_anim_phase % 4 {
            0 => "",
            1 => ".",
            2 => "..",
            _ => "...",
        }
    } else {
        ""
    };

    let title_line = Line::from(Span::styled(
        format!(" {title}"),
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(crate::ui::theme::TEXT_DIM),
    ));

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

    let paragraph = if can_type && !app.input.is_empty() {
        let style = Style::default().fg(crate::ui::theme::TEXT);
        let lines: Vec<Line> = app
            .input
            .split('\n')
            .map(|l| Line::from(Span::styled(l.to_string(), style)))
            .collect();
        Paragraph::new(Text::from(lines))
    } else if can_type
        && app.input.is_empty()
        && let (Some(phrase), Some(ph)) = (others_typing.as_ref(), placeholder.as_ref())
    {
        let typing_style = Style::default()
            .fg(crate::ui::theme::TYPING_OTHERS)
            .add_modifier(Modifier::ITALIC);
        let typing_text = typing_line_with_dots(phrase, typing_dots);
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(typing_text, typing_style)),
            Line::from(Span::styled(ph.clone(), style)),
        ]))
    } else {
        Paragraph::new(Line::from(Span::styled(content, style)))
    }
    .block(blk)
    .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);

    if focused && can_type && !app.input.is_empty() {
        let inner_w = area.width.saturating_sub(2).max(1);
        let (col, row) = input_word_wrap::eol_cursor_col_row(&app.input, inner_w, input_span_style());
        let max_x = area.x + area.width.saturating_sub(2);
        let x = (area.x + 1 + col).min(max_x);
        let y = area.y + 1 + row;
        Some((x, y))
    } else if focused && can_type {
        let extra = if app.input.is_empty() && app.others_typing_phrase().is_some() {
            1u16
        } else {
            0
        };
        Some((area.x + 1, area.y + 1 + extra))
    } else {
        None
    }
}
