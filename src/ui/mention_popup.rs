use crate::app::{App, MentionPick};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

const POPUP_MAX_VISIBLE: u16 = 14;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(mention_state) = &app.mention_autocomplete else {
        return;
    };

    if mention_state.matches.is_empty() {
        return;
    }

    let items: Vec<ListItem> = mention_state
        .matches
        .iter()
        .map(|&idx| {
            let pick = &mention_state.pool[idx];
            match pick {
                MentionPick::User {
                    display,
                    username,
                    user_id,
                } => {
                    let label = if display != username {
                        format!("{display} ({username})")
                    } else {
                        display.clone()
                    };
                    let is_self = user_id.as_str() == app.me.id.as_str();
                    let g = app.guild_id_for_active_channel();
                    let fg = app.member_name_color(g.as_deref(), user_id.as_str(), is_self);
                    ListItem::new(Line::from(vec![
                        Span::styled("user ", Style::default().fg(crate::ui::theme::TEXT_MUTED)),
                        Span::styled(label, Style::default().fg(fg)),
                    ]))
                }
                MentionPick::Role { name, color, .. } => ListItem::new(Line::from(vec![
                    Span::styled("role ", Style::default().fg(crate::ui::theme::TEXT_MUTED)),
                    Span::styled(
                        name.clone(),
                        crate::ui::theme::role_mention_style(*color).add_modifier(Modifier::BOLD),
                    ),
                ])),
            }
        })
        .collect();

    let visible = (items.len() as u16).clamp(1, POPUP_MAX_VISIBLE);
    let popup_area = Rect {
        x: area.x,
        y: area.y.saturating_sub(visible + 2),
        width: 52.min(area.width),
        height: visible + 2,
    };

    let item_len = items.len();
    let sel = mention_state.selected_index.min(item_len.saturating_sub(1));

    let list = List::new(items)
        .scroll_padding(1)
        .block(
            Block::default()
                .title(" @ mention - user | role ")
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
