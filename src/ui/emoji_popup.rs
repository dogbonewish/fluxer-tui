use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(emoji_state) = &app.emoji_autocomplete else {
        return;
    };

    let items: Vec<ListItem> = emoji_state
        .matches
        .iter()
        .map(|e| {
            let line = if e.is_custom {
                Line::from(e.label.clone()).style(Style::default().fg(crate::ui::theme::ACCENT))
            } else {
                Line::from(e.label.clone())
            };
            ListItem::new(line)
        })
        .collect();

    if items.is_empty() {
        return;
    }

    let item_len = items.len();
    let sel = emoji_state.selected_index.min(item_len.saturating_sub(1));

    let item_count = item_len.min(12) as u16;
    let popup_width = 40.min(area.width);
    let popup_area = Rect {
        x: area.x,
        y: area.y.saturating_sub(item_count + 2),
        width: popup_width,
        height: item_count + 2,
    };

    let list = List::new(items)
        .scroll_padding(0)
        .block(
            Block::default()
                .title(" Emojis ")
                .borders(Borders::ALL)
                .border_style(crate::ui::theme::focused_border(true)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default().with_selected(Some(sel));

    frame.render_widget(Clear, popup_area);
    frame.render_stateful_widget(list, popup_area, &mut state);
}
