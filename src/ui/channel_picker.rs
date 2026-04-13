use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(picker) = &app.channel_picker else {
        return;
    };

    frame.render_widget(Clear, area);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(12),
            Constraint::Percentage(76),
            Constraint::Percentage(12),
        ])
        .split(area);
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(8),
            Constraint::Percentage(84),
            Constraint::Percentage(8),
        ])
        .split(outer[1]);
    let popup = mid[1];
    frame.render_widget(Clear, popup);

    let visible = (popup.height.saturating_sub(4)).max(1) as usize;
    let total = picker.filtered.len();
    let mut items: Vec<ListItem> = Vec::new();
    if total == 0 {
        items.push(ListItem::new(Line::from(Span::styled(
            "No matching channels (Backspace to edit filter)",
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ))));
        let list = List::new(items).block(
            Block::default()
                .title(Line::from(Span::styled(
                    " Channels (Ctrl+K) ",
                    Style::default()
                        .fg(crate::ui::theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                )))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM)),
        );
        frame.render_widget(list, popup);
        return;
    }

    let sel_vis = picker.selected.min(total.saturating_sub(1));
    let start = sel_vis.saturating_sub(visible / 2);
    let start = start.min(total.saturating_sub(visible));
    let end = (start + visible).min(total);

    for fi in start..end {
        let idx = picker.filtered[fi];
        let entry = &picker.entries[idx];
        let is_sel = fi == sel_vis;
        let style = if is_sel {
            Style::default()
                .fg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(crate::ui::theme::BG_TERTIARY)
        } else {
            Style::default().fg(crate::ui::theme::TEXT)
        };
        items.push(ListItem::new(Line::from(Span::styled(entry.label.clone(), style))));
    }

    let title = format!(
        " Channels (Ctrl+K) - \u{2315} {} ",
        if picker.query.is_empty() {
            "type to filter".to_string()
        } else {
            picker.query.clone()
        }
    );
    let list = List::new(items).block(
        Block::default()
            .title(Line::from(Span::styled(
                title,
                Style::default()
                    .fg(crate::ui::theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM)),
    );
    frame.render_widget(list, popup);

    let foot = Paragraph::new(Line::from(Span::styled(
        "↑↓ Enter - open   Esc - cancel   Backspace - edit filter",
        Style::default().fg(crate::ui::theme::TEXT_MUTED),
    )))
    .alignment(Alignment::Center);
    let foot_area = Rect {
        x: popup.x,
        y: popup.y + popup.height.saturating_sub(1),
        width: popup.width,
        height: 1,
    };
    frame.render_widget(foot, foot_area);
}
