use crate::api::types::{
    CHANNEL_DM, CHANNEL_DM_PERSONAL_NOTES, CHANNEL_GROUP_DM, CHANNEL_GUILD_TEXT, ChannelResponse,
    EmbedMediaResponse, MessageEmbedResponse,
};
use crate::app::{App, Focus, display_name};
use crate::ui::message_markdown;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

#[derive(Debug)]
struct TerminalLinkSlot {
    line_index: usize,
    url: String,
    kind: TerminalLinkKind,
}

#[derive(Debug, Clone, Copy)]
enum TerminalLinkKind {
    Attachment,
    Embed,
}

/// kinda broken rn i will work on it later 👍
fn embed_open_link_target(embed: &MessageEmbedResponse, allow_plain_url: bool) -> Option<String> {
    let pick = |m: &EmbedMediaResponse| m.proxy_url.clone().or_else(|| m.url.clone());
    embed
        .image
        .as_ref()
        .and_then(pick)
        .or_else(|| embed.thumbnail.as_ref().and_then(pick))
        .or_else(|| {
            let t = embed.embed_type.as_str();
            if matches!(t, "image" | "gifv" | "video") || allow_plain_url {
                embed.url.clone()
            } else {
                None
            }
        })
}

fn patch_terminal_hyperlinks(
    buf: &mut Buffer,
    start_x: u16,
    y: u16,
    area_right: u16,
    url: &str,
    visible: &str,
    style: Style,
) {
    let chars: Vec<char> = visible.chars().collect();
    let mut i = 0usize;
    let mut col = start_x;
    while i < chars.len() {
        let end = (i + 2).min(chars.len());
        let chunk: String = chars[i..end].iter().collect();
        let cell_text = format!("\x1b]8;;{url}\x07{chunk}\x1b]8;;\x07");
        if col.saturating_add(2) <= area_right {
            buf[(col, y)].set_symbol(&cell_text).set_style(style);
            col = col.saturating_add(2);
        }
        i += 2;
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.active_channel_is_voice() {
        render_voice(frame, area, app);
    } else if app.active_channel_is_link() {
        render_link(frame, area, app);
    } else {
        render_messages(frame, area, app);
    }
}

/// in the beginning there was GOD Just kidding it was HAMPLER.
fn channel_welcome_label(channel: &ChannelResponse) -> String {
    match channel.channel_type() {
        CHANNEL_GUILD_TEXT => format!("#{}", channel.name),
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
        _ => {
            if channel.guild_id.is_some() && !channel.name.is_empty() {
                format!("#{}", channel.name)
            } else {
                channel.name.clone()
            }
        }
    }
}

/// praise satan
fn channel_welcome_lines(_app: &App, channel: &ChannelResponse) -> Vec<Line<'static>> {
    let label = channel_welcome_label(channel);
    let genesis =
        format!("In the beginning, there was nothing. Then, there was {label}. And it was good.");
    vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Welcome to ",
                Style::default()
                    .fg(crate::ui::theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                label.clone(),
                Style::default()
                    .fg(crate::ui::theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            genesis,
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        )),
        Line::from(""),
    ]
}

fn build_message_lines(
    app: &App,
    messages: &[crate::api::types::MessageResponse],
) -> (Vec<Line<'static>>, Vec<TerminalLinkSlot>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut link_slots: Vec<TerminalLinkSlot> = Vec::new();
    let mut prev_author_id: Option<&str> = None;
    let mut prev_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

    for (idx, message) in messages.iter().enumerate() {
        let is_selected_msg = app.selected_message_index == Some(idx);
        let author = display_name(&message.author);

        let is_self = message.author.id == app.me.id;
        let name_color = if is_self {
            crate::ui::theme::self_username_color()
        } else {
            crate::ui::theme::username_color(&message.author.id)
        };

        let cur_ts = message
            .timestamp
            .parse::<chrono::DateTime<chrono::Utc>>()
            .ok();
        let has_ref = message.referenced_message.is_some() || message.message_reference.is_some();
        let same_author = prev_author_id == Some(message.author.id.as_str());
        let within_group = same_author
            && !has_ref
            && match (prev_timestamp, cur_ts) {
                (Some(prev), Some(cur)) => (cur - prev).num_minutes().abs() < 5,
                _ => false,
            };

        if let Some(ref_msg) = &message.referenced_message {
            let ref_author = display_name(&ref_msg.author);
            let preview: String = ref_msg.content.chars().take(80).collect();
            let suffix = if ref_msg.content.len() > 80 {
                "..."
            } else {
                ""
            };
            let mut preview_spans = vec![Span::styled(
                format!("  \u{21B3} {ref_author}: "),
                Style::default().fg(crate::ui::theme::TEXT_MUTED),
            )];
            preview_spans.extend(message_markdown::parse_message_spans(
                &format!("{preview}{suffix}"),
                app,
            ));
            lines.push(Line::from(preview_spans));
        } else if let Some(ref mref) = message.message_reference
            && mref.reference_type == 1
        {
            lines.push(Line::from(vec![
                Span::styled(
                    "  \u{21B3} ",
                    Style::default().fg(crate::ui::theme::TEXT_MUTED),
                ),
                Span::styled(
                    "forwarded",
                    Style::default()
                        .fg(crate::ui::theme::TEXT_MUTED)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        let sel_marker = if is_selected_msg { "\u{25B6} " } else { "" };
        let header_style = if is_selected_msg {
            Style::default().bg(crate::ui::theme::BG_TERTIARY)
        } else {
            Style::default()
        };

        if within_group && !is_selected_msg {
            // grouped
        } else if within_group && is_selected_msg {
            let timestamp = format_timestamp(&message.timestamp);
            lines.push(
                Line::from(vec![
                    Span::styled(
                        sel_marker.to_string(),
                        Style::default().fg(crate::ui::theme::ACCENT),
                    ),
                    Span::styled(
                        format!("[{timestamp}] "),
                        Style::default().fg(crate::ui::theme::TEXT_DIM),
                    ),
                ])
                .style(header_style),
            );
        } else {
            if idx > 0 && !within_group {
                lines.push(Line::from(""));
            }
            let timestamp = format_timestamp(&message.timestamp);
            let header_spans = vec![
                Span::styled(
                    sel_marker.to_string(),
                    Style::default().fg(crate::ui::theme::ACCENT),
                ),
                Span::styled(
                    format!("[{timestamp}] "),
                    Style::default().fg(crate::ui::theme::TEXT_DIM),
                ),
                Span::styled(
                    author.clone(),
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
            ];
            lines.push(Line::from(header_spans).style(header_style));
        }

        if !message.content.trim().is_empty() {
            for m_line in message_markdown::content_lines(&message.content, app) {
                if m_line.is_empty() {
                    lines.push(Line::from(" "));
                } else {
                    lines.push(Line::from(m_line));
                }
            }
        }

        prev_author_id = Some(&message.author.id);
        prev_timestamp = cur_ts;

        for attachment in &message.attachments {
            let size_str = match attachment.size {
                Some(s) if s < 1024 => format!("{} B", s),
                Some(s) if s < 1024 * 1024 => format!("{:.1} KB", s as f64 / 1024.0),
                Some(s) => format!("{:.1} MB", s as f64 / (1024.0 * 1024.0)),
                None => "unknown".to_string(),
            };
            let mime = attachment.content_type.as_deref().unwrap_or("unknown");
            lines.push(Line::from(vec![
                Span::styled(
                    "\u{1F4CE} ",
                    Style::default().fg(crate::ui::theme::ACCENT_DIM),
                ),
                Span::styled(
                    attachment.filename.clone(),
                    Style::default()
                        .fg(crate::ui::theme::LINK_COLOR)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(
                    format!(" [{mime} \u{00B7} {size_str}]"),
                    Style::default().fg(crate::ui::theme::TEXT_DIM),
                ),
            ]));
            let href = attachment
                .proxy_url
                .as_deref()
                .or(attachment.url.as_deref());
            if let Some(href) = href {
                lines.push(Line::from(Span::styled(
                    "  open link",
                    Style::default()
                        .fg(crate::ui::theme::LINK_COLOR)
                        .add_modifier(Modifier::UNDERLINED),
                )));
                link_slots.push(TerminalLinkSlot {
                    line_index: lines.len() - 1,
                    url: href.to_string(),
                    kind: TerminalLinkKind::Attachment,
                });
            }
        }

        for embed in &message.embeds {
            let has_content = embed.title.is_some()
                || embed.description.is_some()
                || embed.author.is_some()
                || !embed.fields.is_empty();
            let bar_color = embed
                .color
                .map(|c| {
                    ratatui::style::Color::Rgb(
                        ((c >> 16) & 0xFF) as u8,
                        ((c >> 8) & 0xFF) as u8,
                        (c & 0xFF) as u8,
                    )
                })
                .unwrap_or(crate::ui::theme::ACCENT_DIM);

            if !has_content {
                let href = embed_open_link_target(embed, true);
                if let Some(ref u) = href {
                    lines.push(Line::from(Span::styled(
                        "\u{2502} open link",
                        Style::default()
                            .fg(crate::ui::theme::LINK_COLOR)
                            .add_modifier(Modifier::UNDERLINED),
                    )));
                    link_slots.push(TerminalLinkSlot {
                        line_index: lines.len() - 1,
                        url: u.clone(),
                        kind: TerminalLinkKind::Embed,
                    });
                }
                continue;
            }

            if let Some(author) = &embed.author {
                lines.push(Line::from(vec![
                    Span::styled("\u{2502} ", Style::default().fg(bar_color)),
                    Span::styled(
                        author.name.clone(),
                        Style::default().fg(crate::ui::theme::TEXT_DIM),
                    ),
                ]));
            }
            if let Some(title) = &embed.title {
                let mut title_spans =
                    vec![Span::styled("\u{2502} ", Style::default().fg(bar_color))];
                let base = if embed.url.is_some() {
                    Style::default()
                        .fg(crate::ui::theme::LINK_COLOR)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(crate::ui::theme::TEXT)
                        .add_modifier(Modifier::BOLD)
                };
                for s in message_markdown::parse_message_spans(title, app) {
                    title_spans.push(Span::styled(s.content.to_string(), base.patch(s.style)));
                }
                lines.push(Line::from(title_spans));
            }
            if let Some(desc) = &embed.description {
                for row in message_markdown::content_lines(desc, app) {
                    let mut r = vec![Span::styled("\u{2502} ", Style::default().fg(bar_color))];
                    if row.is_empty() {
                        r.push(Span::raw(" "));
                    } else {
                        r.extend(row);
                    }
                    lines.push(Line::from(r));
                }
            }
            for field in &embed.fields {
                lines.push(Line::from(vec![
                    Span::styled("\u{2502} ", Style::default().fg(bar_color)),
                    Span::styled(
                        field.name.clone(),
                        Style::default()
                            .fg(crate::ui::theme::TEXT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                for row in message_markdown::content_lines(&field.value, app) {
                    let mut r = vec![Span::styled("\u{2502} ", Style::default().fg(bar_color))];
                    if row.is_empty() {
                        r.push(Span::raw(" "));
                    } else {
                        r.extend(row);
                    }
                    lines.push(Line::from(r));
                }
            }
            if let Some(footer) = &embed.footer {
                lines.push(Line::from(vec![
                    Span::styled("\u{2502} ", Style::default().fg(bar_color)),
                    Span::styled(
                        footer.text.clone(),
                        Style::default().fg(crate::ui::theme::TEXT_MUTED),
                    ),
                ]));
            }
            let href = embed_open_link_target(embed, false);
            if let Some(u) = href {
                lines.push(Line::from(Span::styled(
                    "\u{2502} open link",
                    Style::default()
                        .fg(crate::ui::theme::LINK_COLOR)
                        .add_modifier(Modifier::UNDERLINED),
                )));
                link_slots.push(TerminalLinkSlot {
                    line_index: lines.len() - 1,
                    url: u,
                    kind: TerminalLinkKind::Embed,
                });
            }
        }

        if !message.reactions.is_empty() {
            let mut reaction_spans: Vec<Span<'static>> = Vec::new();
            for reaction in &message.reactions {
                let emoji_str = if reaction.emoji.id.is_some() {
                    format!(":{}:", reaction.emoji.name)
                } else {
                    reaction.emoji.name.clone()
                };
                let style = if reaction.me {
                    Style::default()
                        .fg(crate::ui::theme::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(crate::ui::theme::TEXT_DIM)
                };
                reaction_spans.push(Span::styled(
                    format!(" {emoji_str} {}", reaction.count),
                    style,
                ));
                reaction_spans.push(Span::raw(" "));
            }
            lines.push(Line::from(reaction_spans));
        }
    }

    (lines, link_slots)
}

fn render_messages(frame: &mut Frame, area: Rect, app: &App) {
    let block = block("Messages", app.focus == Focus::Messages);
    let inner = block.inner(area);
    let text_w = inner.width.max(1);

    let pane_visible = area.height.saturating_sub(2).max(1);

    let welcome: Vec<Line<'static>> = app
        .active_channel()
        .map(|ch| channel_welcome_lines(app, &ch))
        .unwrap_or_default();

    let messages = app.active_messages();
    let loading = app
        .selected_channel_id
        .as_ref()
        .is_some_and(|id| app.loading_messages.contains(id));

    let (body, link_slots_body): (Vec<Line<'static>>, Vec<TerminalLinkSlot>) = if loading {
        (
            vec![Line::from(Span::styled(
                "Loading messages...",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ))],
            Vec::new(),
        )
    } else if messages.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        build_message_lines(app, &messages)
    };

    let welcome_body_gap: usize = if welcome.is_empty() { 0 } else { 1 };
    let gap_lines: Vec<Line<'static>> = (0..welcome_body_gap).map(|_| Line::from("")).collect();
    let w = welcome.len();
    let g = gap_lines.len();
    let m = body.len();

    let mut core_lines: Vec<Line<'static>> = Vec::with_capacity(w + g + m);
    core_lines.extend(welcome.iter().cloned());
    core_lines.extend(gap_lines.iter().cloned());
    core_lines.extend(body.iter().cloned());

    let core_heights: Vec<u16> = core_lines
        .iter()
        .map(|line| {
            Paragraph::new(Text::from(vec![line.clone()]))
                .wrap(Wrap { trim: false })
                .line_count(text_w) as u16
        })
        .collect();
    let content_rows: u16 = core_heights.iter().sum();
    let filler_top_n = pane_visible.saturating_sub(content_rows) as usize;

    let body_offset = filler_top_n + w + g;

    let link_slots: Vec<TerminalLinkSlot> = link_slots_body
        .into_iter()
        .map(|mut s| {
            s.line_index += body_offset;
            s
        })
        .collect();

    let mut lines: Vec<Line<'static>> = (0..filler_top_n).map(|_| Line::from("")).collect();
    lines.extend(welcome);
    lines.extend(gap_lines);
    lines.extend(body);

    let heights: Vec<u16> = lines
        .iter()
        .map(|line| {
            Paragraph::new(Text::from(vec![line.clone()]))
                .wrap(Wrap { trim: false })
                .line_count(text_w) as u16
        })
        .collect();
    let total_display_rows: u16 = heights.iter().sum();

    let max_scroll = total_display_rows.saturating_sub(pane_visible);
    let scroll_from_bottom = app.message_scroll_from_bottom.min(max_scroll);
    let top = total_display_rows.saturating_sub(pane_visible.saturating_add(scroll_from_bottom));

    let mut prefix = vec![0usize; lines.len() + 1];
    for i in 0..lines.len() {
        prefix[i + 1] = prefix[i] + heights[i] as usize;
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block.clone())
        .wrap(Wrap { trim: false })
        .scroll((top, 0));
    frame.render_widget(paragraph, area);

    let top_us = top as usize;
    let vis = pane_visible as usize;
    let link_style = Style::default()
        .fg(crate::ui::theme::LINK_COLOR)
        .add_modifier(Modifier::UNDERLINED);

    let buf = frame.buffer_mut();
    let area_right = inner.right();
    for slot in &link_slots {
        let wrow = prefix[slot.line_index];
        if wrow < top_us || wrow >= top_us + vis {
            continue;
        }
        let y = inner.y + (wrow - top_us) as u16;
        let (visible, x0) = match slot.kind {
            TerminalLinkKind::Attachment => ("  open link", inner.x),
            TerminalLinkKind::Embed => ("\u{2502} open link", inner.x),
        };
        patch_terminal_hyperlinks(buf, x0, y, area_right, &slot.url, visible, link_style);
    }
}

fn render_voice(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(channel) = app.active_channel() {
        lines.push(Line::styled(
            format!("\u{1F50A} {}", channel.name),
            Style::default()
                .fg(crate::ui::theme::VOICE_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Voice is view-only in this client: you cannot join, transmit, or hear audio here.",
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ));
        lines.push(Line::styled(
            "Below is who appears connected from gateway state (informational only).",
            Style::default().fg(crate::ui::theme::TEXT_MUTED),
        ));
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Members",
            Style::default()
                .fg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));

        let members = app.voice_members_for_active_channel();
        if members.is_empty() {
            lines.push(Line::styled(
                "Nobody listed.",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ));
        } else {
            for member in members {
                lines.push(Line::styled(
                    format!("  {member}"),
                    Style::default().fg(crate::ui::theme::TEXT),
                ));
            }
        }
    } else {
        lines.push(Line::styled(
            "Select a voice channel.",
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block("Voice (read-only)", app.focus == Focus::Messages));
    frame.render_widget(paragraph, area);
}

fn render_link(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(channel) = app.active_channel() {
        lines.push(Line::styled(
            format!("\u{1F517} {}", channel.name),
            Style::default()
                .fg(crate::ui::theme::LINK_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(""));
        if let Some(url) = &channel.url {
            lines.push(Line::styled(
                url.clone(),
                Style::default().fg(crate::ui::theme::ACCENT),
            ));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Press Enter to open this link in your browser.",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ));
        } else {
            lines.push(Line::styled(
                "This link channel has no URL set.",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ));
        }
    } else {
        lines.push(Line::styled(
            "Select a channel.",
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ));
    }

    let paragraph =
        Paragraph::new(Text::from(lines)).block(block("Link", app.focus == Focus::Messages));
    frame.render_widget(paragraph, area);
}

fn format_timestamp(raw: &str) -> String {
    use chrono::{DateTime, Local, Utc};

    if let Ok(dt) = raw.parse::<DateTime<Utc>>() {
        let local = dt.with_timezone(&Local);
        let now = Local::now();
        if local.date_naive() == now.date_naive() {
            return local.format("%H:%M").to_string();
        } else {
            return local.format("%m/%d %H:%M").to_string();
        }
    }

    if raw.len() >= 16 {
        raw[11..16].to_string()
    } else {
        raw.to_string()
    }
}

fn block(title: &str, focused: bool) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(crate::ui::theme::focused_border(focused))
        .style(Style::default().bg(crate::ui::theme::BG))
}
