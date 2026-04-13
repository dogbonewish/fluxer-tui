use crate::app::{App, CommandAutocomplete};
use crate::slash_commands::SLASH_COMMANDS;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const POPUP_MAX_VISIBLE: u16 = 10;
const POPUP_W: u16 = 64;

fn truncate_desc(s: &str, max_w: usize) -> String {
    if max_w == 0 || UnicodeWidthStr::width(s) <= max_w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if w + cw > max_w.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(CommandAutocomplete {
        matches,
        selected_index,
    }) = &app.command_autocomplete
    else {
        return;
    };

    if matches.is_empty() {
        return;
    }

    const NAME_COL: usize = 14;
    let desc_budget = POPUP_W.saturating_sub(4) as usize - NAME_COL;

    let items: Vec<ListItem> = matches
        .iter()
        .map(|&cmd_i| {
            let c = &SLASH_COMMANDS[cmd_i];
            let desc = truncate_desc(c.description, desc_budget);
            let mut name = c.name.to_string();
            let nw = UnicodeWidthStr::width(name.as_str());
            if nw < NAME_COL {
                name.push_str(&" ".repeat(NAME_COL - nw));
            }
            ListItem::new(Line::from(vec![
                Span::styled(
                    name,
                    Style::default()
                        .fg(crate::ui::theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(crate::ui::theme::TEXT_MUTED)),
            ]))
        })
        .collect();

    let visible = (items.len() as u16).clamp(1, POPUP_MAX_VISIBLE);
    let popup_area = Rect {
        x: area.x,
        y: area.y.saturating_sub(visible + 2),
        width: POPUP_W.min(area.width),
        height: visible + 2,
    };

    let item_len = items.len();
    let sel = (*selected_index).min(item_len.saturating_sub(1));

    let list = List::new(items)
        .scroll_padding(1)
        .block(
            Block::default()
                .title(" / commands ")
                .borders(Borders::ALL)
                .border_style(crate::ui::theme::focused_border(true)),
        )
        .highlight_style(
            Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default().with_selected(Some(sel));

    frame.render_widget(Clear, popup_area);
    frame.render_stateful_widget(list, popup_area, &mut state);
}
