use crate::api::types::{
    CHANNEL_DM, CHANNEL_DM_PERSONAL_NOTES, CHANNEL_GROUP_DM, CHANNEL_GUILD_CATEGORY,
    CHANNEL_GUILD_LINK, CHANNEL_GUILD_TEXT, CHANNEL_GUILD_VOICE, ChannelResponse,
};
use crate::app::{App, Focus, ServerSelection, display_name};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use unicode_width::UnicodeWidthChar;

pub fn render_servers(frame: &mut Frame, area: Rect, app: &App) {
    let entries = app.server_entries();
    let items = if entries.is_empty() {
        vec![ListItem::new(
            Line::from("No servers").style(Style::default().fg(crate::ui::theme::TEXT_DIM)),
        )]
    } else {
        entries
            .iter()
            .map(|entry| {
                ListItem::new(
                    Line::from(server_label(app, entry))
                        .style(Style::default().fg(crate::ui::theme::TEXT)),
                )
            })
            .collect()
    };

    let n = items.len().max(1);
    let sel = app.server_selected_index().min(n - 1);
    let mut state = ListState::default().with_selected(Some(sel));
    let focused = app.focus == Focus::Servers;
    let list = List::new(items)
        .scroll_padding(0)
        .block(
            Block::default()
                .title(" Servers ")
                .borders(Borders::ALL)
                .border_style(crate::ui::theme::focused_border(focused))
                .style(Style::default().bg(crate::ui::theme::BG_SECONDARY)),
        )
        .highlight_style(highlight(focused))
        .highlight_symbol(if focused { " > " } else { "   " });
    frame.render_stateful_widget(list, area, &mut state);
}

pub fn render_channels(frame: &mut Frame, area: Rect, app: &App) {
    let entries = app.channel_entries();
    let max_name_width = (area.width as usize).saturating_sub(8);
    let items = if entries.is_empty() {
        let message = if let Some(guild_id) = app.active_guild_id() {
            if app.loading_channels.contains(&guild_id) {
                "Loading channels..."
            } else {
                "No channels"
            }
        } else {
            "No direct messages"
        };
        vec![ListItem::new(
            Line::from(message).style(Style::default().fg(crate::ui::theme::TEXT_DIM)),
        )]
    } else {
        entries
            .iter()
            .map(|channel| {
                let is_selected = app.selected_channel_id.as_deref() == Some(channel.id.as_str());
                ListItem::new(channel_label(app, channel, is_selected, max_name_width))
            })
            .collect()
    };

    let n = items.len().max(1);
    let sel = app.channel_selected_index().min(n - 1);
    let mut state = ListState::default().with_selected(Some(sel));
    let focused = app.focus == Focus::Channels;
    let list = List::new(items)
        .scroll_padding(0)
        .block(
            Block::default()
                .title(" Channels ")
                .borders(Borders::ALL)
                .border_style(crate::ui::theme::focused_border(focused))
                .style(Style::default().bg(crate::ui::theme::BG_SECONDARY)),
        )
        .highlight_style(highlight(focused))
        .highlight_symbol(if focused { " > " } else { "   " });
    frame.render_stateful_widget(list, area, &mut state);
}

fn server_label(app: &App, entry: &ServerSelection) -> String {
    match entry {
        ServerSelection::DirectMessages => "Direct Messages".to_string(),
        ServerSelection::Guild(id) => app
            .guilds
            .iter()
            .find(|guild| guild.id == *id)
            .map(|guild| guild.name.clone())
            .unwrap_or_else(|| id.clone()),
    }
}

fn channel_label(
    app: &App,
    channel: &ChannelResponse,
    is_selected: bool,
    max_width: usize,
) -> Line<'static> {
    if channel.channel_type() == CHANNEL_GUILD_CATEGORY {
        let name = channel.name.to_uppercase();
        let truncated = truncate_str(&name, max_width);
        return Line::from(Span::styled(
            format!("  {truncated}"),
            Style::default()
                .fg(crate::ui::theme::TEXT_MUTED)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let (icon, color) = match channel.channel_type() {
        CHANNEL_GUILD_TEXT => ("#", crate::ui::theme::TEXT_DIM),
        CHANNEL_GUILD_VOICE => ("\u{1F50A}", crate::ui::theme::VOICE_COLOR),
        CHANNEL_DM => ("@", crate::ui::theme::TEXT_DIM),
        CHANNEL_GROUP_DM => ("+", crate::ui::theme::TEXT_DIM),
        CHANNEL_DM_PERSONAL_NOTES => ("*", crate::ui::theme::TEXT_DIM),
        CHANNEL_GUILD_LINK => ("\u{1F517}", crate::ui::theme::LINK_COLOR),
        _ => ("?", crate::ui::theme::TEXT_DIM),
    };

    let is_unread = app.channel_is_unread(&channel.id);
    let mention_count = app.channel_mention_count(&channel.id);

    let mut style = Style::default().fg(color);
    if is_selected {
        style = style
            .add_modifier(Modifier::BOLD)
            .fg(crate::ui::theme::TEXT);
    } else if is_unread {
        style = style.fg(crate::ui::theme::TEXT);
    }

    let name = channel_name(app, channel);
    let indent = if channel.parent_id.is_some() && channel.guild_id.is_some() {
        "  "
    } else {
        ""
    };
    let truncated = truncate_str(&name, max_width.saturating_sub(indent.len() + 3));

    let mut spans = vec![
        Span::styled(format!("{indent}{icon} "), style),
        Span::styled(truncated, style),
    ];

    if is_unread && !is_selected {
        spans.insert(
            0,
            Span::styled("\u{25CF} ", Style::default().fg(crate::ui::theme::TEXT)),
        );
    }

    if mention_count > 0 {
        spans.push(Span::styled(
            format!(" @{mention_count}"),
            Style::default()
                .fg(crate::ui::theme::DANGER)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn channel_name(_app: &App, channel: &ChannelResponse) -> String {
    match channel.channel_type() {
        CHANNEL_DM_PERSONAL_NOTES => "Personal Notes".to_string(),
        CHANNEL_DM => channel
            .recipients
            .first()
            .map(display_name)
            .unwrap_or_else(|| "Direct Message".to_string()),
        CHANNEL_GROUP_DM => {
            if !channel.name.trim().is_empty() {
                channel.name.clone()
            } else if !channel.recipients.is_empty() {
                channel
                    .recipients
                    .iter()
                    .map(display_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                "Group DM".to_string()
            }
        }
        _ => channel.name.clone(),
    }
}

/// shorten to fit within the max terminal columns
fn truncate_str(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    if max_cols <= 1 {
        return "\u{2026}".to_string();
    }
    let budget = max_cols.saturating_sub(1);
    let mut used = 0usize;
    let mut end = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if used + w > budget {
            break;
        }
        used += w;
        end += ch.len_utf8();
    }
    if end >= s.len() {
        s.to_string()
    } else {
        format!("{}\u{2026}", &s[..end])
    }
}

fn highlight(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(crate::ui::theme::TEXT)
            .bg(crate::ui::theme::ACCENT_DIM)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(crate::ui::theme::TEXT)
            .add_modifier(Modifier::BOLD)
    }
}
