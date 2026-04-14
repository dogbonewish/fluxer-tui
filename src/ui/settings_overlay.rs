use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(Clear, area);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(area);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(32),
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
    let panel = Style::default()
        .fg(crate::ui::theme::TEXT)
        .bg(crate::ui::theme::BG_SECONDARY);

    let clock_on = app.ui_settings.clock_12h;
    let clock_primary = if clock_on {
        ("12-hour", "AM / PM")
    } else {
        ("24-hour", "00:00 – 23:59")
    };
    let clock_alt = if clock_on {
        ("24-hour", "00:00 – 23:59")
    } else {
        ("12-hour", "AM / PM")
    };
    let typing_on = app.ui_settings.show_typing_indicators;
    let typing_primary = if typing_on {
        ("On", "Show activity in the current channel")
    } else {
        ("Off", "Hide activity in the current channel")
    };
    let typing_alt = if typing_on {
        ("Off", "Hide activity in the current channel")
    } else {
        ("On", "Show activity in the current channel")
    };
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("  ", text),
        Span::styled("fluxer-tui", accent),
        Span::styled(" · preferences", dim),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  INTERFACE",
        accent_soft.add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "  ───────────────────────────────────────────",
        accent_soft,
    )]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled("  Message timestamps", dim)]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", text),
        Span::styled(
            if app.settings_cursor == 0 {
                "▸ "
            } else {
                "  "
            },
            if app.settings_cursor == 0 {
                accent
            } else {
                muted
            },
        ),
        Span::styled(
            format!("{}  ·  {}", clock_primary.0, clock_primary.1),
            panel.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ", text),
        Span::styled(format!("{}  ·  {}", clock_alt.0, clock_alt.1), muted),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled("  Typing indicators", dim)]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", text),
        Span::styled(
            if app.settings_cursor == 1 {
                "▸ "
            } else {
                "  "
            },
            if app.settings_cursor == 1 {
                accent
            } else {
                muted
            },
        ),
        Span::styled(
            format!("{}  ·  {}", typing_primary.0, typing_primary.1),
            panel.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    ", text),
        Span::styled(format!("{}  ·  {}", typing_alt.0, typing_alt.1), muted),
    ]));
    let block = Block::default()
        .title(Line::from(Span::styled(" Settings ", accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, content);

    let hint = Paragraph::new(Line::from(vec![Span::styled(
        "↑/↓ move  ·  Space / Enter toggle  ·  Esc / q close",
        muted,
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(hint, body[1]);
}
