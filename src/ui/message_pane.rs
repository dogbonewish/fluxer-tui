use crate::api::types::{
    CHANNEL_DM, CHANNEL_DM_PERSONAL_NOTES, CHANNEL_GROUP_DM, CHANNEL_GUILD_TEXT, ChannelResponse,
    MessageEmbedResponse,
};
use crate::app::{App, Focus, display_name};
use crate::ui::message_markdown;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn clip_url_for_display(url: &str, max_chars: usize) -> String {
    let t = url.trim();
    if t.is_empty() {
        return String::new();
    }
    let n = t.chars().count();
    if n <= max_chars {
        return t.to_string();
    }
    let take = max_chars.saturating_sub(1);
    format!("{}…", t.chars().take(take).collect::<String>())
}

fn embed_display_label(embed: &MessageEmbedResponse) -> (String, bool) {
    let original = embed
        .url
        .as_ref()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"));
    let t = embed.embed_type.as_str();
    let is_gif = matches!(t, "gifv")
        || original.is_some_and(|u| u.contains("tenor.com") || u.to_lowercase().ends_with(".gif"))
        || embed
            .image
            .as_ref()
            .and_then(|m| m.url.as_ref().or(m.proxy_url.as_ref()))
            .is_some_and(|u| u.to_lowercase().ends_with(".gif"));
    let label = if let Some(u) = original {
        clip_url_for_display(u, 72)
    } else {
        let proxy = embed
            .image
            .as_ref()
            .and_then(|m| m.proxy_url.clone().or_else(|| m.url.clone()))
            .or_else(|| {
                embed
                    .thumbnail
                    .as_ref()
                    .and_then(|m| m.proxy_url.clone().or_else(|| m.url.clone()))
            });
        if let Some(u) = proxy {
            clip_url_for_display(&u, 72)
        } else {
            String::new()
        }
    };
    (label, is_gif)
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

fn message_was_edited(message: &crate::api::types::MessageResponse) -> bool {
    message
        .edited_timestamp
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
}

fn edited_span() -> Span<'static> {
    Span::styled(
        "(edited) ",
        Style::default()
            .fg(crate::ui::theme::TEXT_DIM)
            .add_modifier(Modifier::ITALIC),
    )
}

fn sel_prefix_span(is_selected: bool) -> Span<'static> {
    if is_selected {
        Span::styled("\u{25B6} ", Style::default().fg(crate::ui::theme::ACCENT))
    } else {
        Span::raw("  ")
    }
}

fn sel_prefix_cols(is_selected: bool) -> usize {
    let s = if is_selected { "\u{25B6} " } else { "  " };
    UnicodeWidthStr::width(s)
}

fn truncate_to_display_width(s: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max_w {
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

fn reply_context_line(
    is_selected_msg: bool,
    header_style: Style,
    text_w: u16,
    lead: &'static str,
    body: &str,
    body_style: Style,
) -> Line<'static> {
    let tw = text_w.max(1) as usize;
    let prefix_cols = sel_prefix_cols(is_selected_msg);
    let lead_w = UnicodeWidthStr::width(lead);
    let budget = tw.saturating_sub(prefix_cols).saturating_sub(lead_w).max(1);
    let truncated = truncate_to_display_width(body, budget);
    Line::from(vec![
        sel_prefix_span(is_selected_msg),
        Span::styled(lead, Style::default().fg(crate::ui::theme::TEXT_MUTED)),
        Span::styled(truncated, body_style),
    ])
    .style(header_style)
}

fn push_reply_message_header(
    is_selected_msg: bool,
    header_style: Style,
    message: &crate::api::types::MessageResponse,
    author: &str,
    name_color: ratatui::style::Color,
    lines: &mut Vec<Line<'static>>,
    clock_12h: bool,
) {
    let timestamp = format_timestamp(&message.timestamp, clock_12h);
    let mut header_spans = vec![sel_prefix_span(is_selected_msg)];
    header_spans.push(Span::styled(
        format!("[{timestamp}] "),
        Style::default().fg(crate::ui::theme::TEXT_DIM),
    ));
    if message_was_edited(message) {
        header_spans.push(edited_span());
    }
    header_spans.push(Span::styled(
        author.to_string(),
        Style::default().fg(name_color).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::from(header_spans).style(header_style));
}

fn referenced_body_preview(ref_msg: &crate::api::types::MessageResponse) -> String {
    let c = ref_msg.content.trim();
    if !c.is_empty() {
        let flat: String = c.chars().filter(|&x| x != '\n' && x != '\r').collect();
        let count = flat.chars().count();
        let mut s: String = flat.chars().take(72).collect();
        if count > 72 {
            s.push('…');
        }
        s
    } else if !ref_msg.attachments.is_empty() {
        let n = ref_msg.attachments.len();
        if n == 1 {
            format!("[file: {}]", ref_msg.attachments[0].filename)
        } else {
            format!("[{n} attachments]")
        }
    } else if !ref_msg.embeds.is_empty() {
        "[embed]".to_string()
    } else {
        "(no text)".to_string()
    }
}

fn push_fluxer_client_system_message(
    app: &App,
    lines: &mut Vec<Line<'static>>,
    message: &crate::api::types::MessageResponse,
    is_selected_msg: bool,
) {
    let header_style = if is_selected_msg {
        Style::default().bg(crate::ui::theme::BG_TERTIARY)
    } else {
        Style::default()
    };
    let box_fg = crate::ui::theme::TEXT_MUTED;
    let ts = format_timestamp(&message.timestamp, app.ui_settings.clock_12h);

    lines.push(
        Line::from(vec![
            sel_prefix_span(is_selected_msg),
            Span::styled("╭─ ", Style::default().fg(box_fg)),
            Span::styled(
                "Fluxerbot",
                Style::default()
                    .fg(crate::ui::theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" [SYSTEM]", Style::default().fg(crate::ui::theme::ACCENT)),
            Span::styled(
                format!("  \u{2014} {ts}"),
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ),
        ])
        .style(header_style),
    );

    lines.push(
        Line::from(vec![
            sel_prefix_span(is_selected_msg),
            Span::styled("┃", Style::default().fg(box_fg)),
        ])
        .style(header_style),
    );

    for row in message_markdown::content_lines(&message.content, app) {
        let mut r = vec![
            sel_prefix_span(is_selected_msg),
            Span::styled("┃ ", Style::default().fg(box_fg)),
        ];
        if row.is_empty() {
            r.push(Span::raw(" "));
        } else {
            r.extend(row);
        }
        lines.push(Line::from(r).style(header_style));
    }

    lines.push(
        Line::from(vec![
            sel_prefix_span(is_selected_msg),
            Span::styled("┃ ", Style::default().fg(box_fg)),
            Span::styled(
                "\u{1F441}\u{FE0F} ",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ),
            Span::styled(
                "only you can see this message. ",
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ),
            Span::styled(
                "dismiss",
                Style::default()
                    .fg(crate::ui::theme::LINK_COLOR)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ])
        .style(header_style),
    );

    lines.push(
        Line::from(vec![
            sel_prefix_span(is_selected_msg),
            Span::styled("╰─", Style::default().fg(box_fg)),
        ])
        .style(header_style),
    );
}

fn build_message_lines(
    app: &App,
    messages: &[crate::api::types::MessageResponse],
    text_w: u16,
) -> (Vec<Line<'static>>, Vec<(usize, usize)>) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line_ranges = vec![(0usize, 0usize); messages.len()];
    let mut prev_author_id: Option<&str> = None;
    let mut prev_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

    for (idx, message) in messages.iter().enumerate() {
        let is_selected_msg = app.selected_message_index == Some(idx);
        let cur_ts = message
            .timestamp
            .parse::<chrono::DateTime<chrono::Utc>>()
            .ok();

        if message.message_type == crate::slash_commands::MESSAGE_TYPE_CLIENT_SYSTEM
            && message.author.id == crate::slash_commands::FLUXERBOT_ID
        {
            if idx > 0 {
                lines.push(Line::from(""));
            }
            let block_start = lines.len();
            push_fluxer_client_system_message(app, &mut lines, message, is_selected_msg);
            line_ranges[idx] = (block_start, lines.len());
            prev_author_id = Some(message.author.id.as_str());
            prev_timestamp = cur_ts;
            continue;
        }

        let gid = app.guild_id_for_channel(message.channel_id.as_str());
        let author = app.shown_name_for_user(gid.as_deref(), &message.author);

        let is_self = message.author.id == app.me.id;
        let name_color = app.member_name_color(gid.as_deref(), &message.author.id, is_self);

        let has_reply_rail =
            message.referenced_message.is_some() || message.message_reference.is_some();
        let same_author = prev_author_id == Some(message.author.id.as_str());
        let within_group = same_author
            && !has_reply_rail
            && match (prev_timestamp, cur_ts) {
                (Some(prev), Some(cur)) => (cur - prev).num_minutes().abs() < 5,
                _ => false,
            };

        let header_style = if is_selected_msg {
            Style::default().bg(crate::ui::theme::BG_TERTIARY)
        } else {
            Style::default()
        };

        if idx > 0 && !within_group {
            lines.push(Line::from(""));
        }

        let block_start = lines.len();

        let tw = text_w.max(1);

        if let Some(ref_msg) = message.referenced_message.as_deref() {
            let ref_author = app.shown_name_for_user(gid.as_deref(), &ref_msg.author);
            let preview = referenced_body_preview(ref_msg);
            let ctx_body = format!("@{ref_author} - {preview}");
            lines.push(reply_context_line(
                is_selected_msg,
                header_style,
                tw,
                "\u{21AA} ",
                &ctx_body,
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ));
            push_reply_message_header(
                is_selected_msg,
                header_style,
                message,
                &author,
                name_color,
                &mut lines,
                app.ui_settings.clock_12h,
            );
        } else if let Some(mref) = &message.message_reference {
            let (ctx_body, body_style) = if mref.reference_type == 1 {
                (
                    "Forwarded",
                    Style::default()
                        .fg(crate::ui::theme::TEXT_MUTED)
                        .add_modifier(Modifier::ITALIC),
                )
            } else {
                (
                    "(original message unavailable)",
                    Style::default().fg(crate::ui::theme::TEXT_DIM),
                )
            };
            lines.push(reply_context_line(
                is_selected_msg,
                header_style,
                tw,
                "\u{21AA} ",
                ctx_body,
                body_style,
            ));
            push_reply_message_header(
                is_selected_msg,
                header_style,
                message,
                &author,
                name_color,
                &mut lines,
                app.ui_settings.clock_12h,
            );
        } else if within_group && !is_selected_msg {
            // grouped
        } else if within_group && is_selected_msg {
            let timestamp = format_timestamp(&message.timestamp, app.ui_settings.clock_12h);
            let mut hdr = vec![
                sel_prefix_span(is_selected_msg),
                Span::styled(
                    format!("[{timestamp}] "),
                    Style::default().fg(crate::ui::theme::TEXT_DIM),
                ),
            ];
            if message_was_edited(message) {
                hdr.push(edited_span());
            }
            lines.push(Line::from(hdr).style(header_style));
        } else {
            let timestamp = format_timestamp(&message.timestamp, app.ui_settings.clock_12h);
            let mut header_spans = vec![sel_prefix_span(is_selected_msg)];
            header_spans.push(Span::styled(
                format!("[{timestamp}] "),
                Style::default().fg(crate::ui::theme::TEXT_DIM),
            ));
            if message_was_edited(message) {
                header_spans.push(edited_span());
            }
            header_spans.push(Span::styled(
                author.clone(),
                Style::default().fg(name_color).add_modifier(Modifier::BOLD),
            ));
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
                let (label, is_gif) = embed_display_label(embed);
                if !label.is_empty() {
                    let mut spans = vec![Span::styled("\u{2502} ", Style::default().fg(bar_color))];
                    if is_gif {
                        spans.push(Span::styled(
                            "[GIF] ",
                            Style::default()
                                .fg(crate::ui::theme::ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    spans.push(Span::styled(
                        label,
                        Style::default()
                            .fg(crate::ui::theme::LINK_COLOR)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    lines.push(Line::from(spans));
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

        line_ranges[idx] = (block_start, lines.len());
    }

    (lines, line_ranges)
}

fn paragraph_line_heights(lines: &[Line<'static>], text_w: u16) -> Vec<u16> {
    lines
        .iter()
        .map(|line| {
            Paragraph::new(Text::from(vec![line.clone()]))
                .wrap(Wrap { trim: false })
                .line_count(text_w) as u16
        })
        .collect()
}

/// When a message is selected, adjust `message_scroll_from_bottom` so the selection stays in view.
pub fn scroll_for_selected_message(
    app: &App,
    text_w: u16,
    pane_visible: u16,
    current_scroll_from_bottom: u16,
) -> Option<u16> {
    let idx = app.selected_message_index?;
    let messages = app.active_messages();
    if messages.is_empty() || idx >= messages.len() {
        return None;
    }
    if app
        .selected_channel_id
        .as_ref()
        .is_some_and(|id| app.loading_messages.contains(id))
    {
        return None;
    }

    let welcome: Vec<Line<'static>> = app
        .active_channel()
        .map(|ch| channel_welcome_lines(app, &ch))
        .unwrap_or_default();

    let (body, line_ranges) = build_message_lines(app, &messages, text_w);

    let welcome_body_gap: usize = if welcome.is_empty() { 0 } else { 1 };
    let gap_lines: Vec<Line<'static>> = (0..welcome_body_gap).map(|_| Line::from("")).collect();

    let mut core_lines: Vec<Line<'static>> =
        Vec::with_capacity(welcome.len() + gap_lines.len() + body.len());
    core_lines.extend(welcome.iter().cloned());
    core_lines.extend(gap_lines.iter().cloned());
    core_lines.extend(body.iter().cloned());

    let core_heights = paragraph_line_heights(&core_lines, text_w);
    let content_rows: u32 = core_heights.iter().map(|&h| h as u32).sum();
    let filler_top_n = (pane_visible as u32).saturating_sub(content_rows) as usize;

    let mut lines: Vec<Line<'static>> = (0..filler_top_n).map(|_| Line::from("")).collect();
    let welcome_len = welcome.len();
    let gap_len = gap_lines.len();
    lines.extend(welcome);
    lines.extend(gap_lines);
    lines.extend(body);

    let heights = paragraph_line_heights(&lines, text_w);
    let mut cum: Vec<u32> = Vec::with_capacity(heights.len() + 1);
    cum.push(0);
    for &h in &heights {
        cum.push(cum.last().copied().unwrap_or(0) + h as u32);
    }
    let total = *cum.last().unwrap_or(&0);
    let pane = pane_visible as u32;
    if total <= pane {
        return Some(0);
    }

    let max_scroll = total.saturating_sub(pane);
    let scroll = (current_scroll_from_bottom as u32).min(max_scroll);
    let mut top = total.saturating_sub(pane).saturating_sub(scroll);

    let body_offset = filler_top_n + welcome_len + gap_len;
    let (b0, b1) = line_ranges.get(idx).copied().unwrap_or((0, 0));
    let lo = body_offset + b0;
    let hi = body_offset + b1;
    if hi >= cum.len() {
        return None;
    }
    let rs = cum[lo];
    let re = cum[hi];

    if re.saturating_sub(rs) > pane {
        top = rs;
    } else {
        if rs < top {
            top = rs;
        }
        let view_bottom = top.saturating_add(pane);
        if re > view_bottom {
            top = re.saturating_sub(pane);
        }
    }

    top = top.min(max_scroll);
    let new_scroll = total.saturating_sub(pane).saturating_sub(top);
    Some(new_scroll.min(max_scroll) as u16)
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

    let body: Vec<Line<'static>> = if loading {
        vec![Line::from(Span::styled(
            "Loading messages...",
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ))]
    } else if messages.is_empty() {
        Vec::new()
    } else {
        build_message_lines(app, &messages, text_w).0
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

    let core_heights = paragraph_line_heights(&core_lines, text_w);
    let content_rows: u16 = core_heights.iter().sum();
    let filler_top_n = pane_visible.saturating_sub(content_rows) as usize;

    let mut lines: Vec<Line<'static>> = (0..filler_top_n).map(|_| Line::from("")).collect();
    lines.extend(welcome);
    lines.extend(gap_lines);
    lines.extend(body);

    let heights = paragraph_line_heights(&lines, text_w);
    let total_display_rows: u16 = heights.iter().sum();

    let max_scroll = total_display_rows.saturating_sub(pane_visible);
    let scroll_from_bottom = app.message_scroll_from_bottom.min(max_scroll);
    let top = total_display_rows.saturating_sub(pane_visible.saturating_add(scroll_from_bottom));

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block.clone())
        .wrap(Wrap { trim: false })
        .scroll((top, 0));
    frame.render_widget(paragraph, area);
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

fn format_timestamp(raw: &str, clock_12h: bool) -> String {
    use chrono::{DateTime, Local, Utc};

    if let Ok(dt) = raw.parse::<DateTime<Utc>>() {
        let local = dt.with_timezone(&Local);
        let now = Local::now();
        if clock_12h {
            if local.date_naive() == now.date_naive() {
                let h = local.format("%I").to_string();
                let h = h.trim_start_matches('0');
                let h = if h.is_empty() { "12" } else { h };
                return format!("{h}{}", local.format(":%M %p"));
            }
            let h = local.format("%I").to_string();
            let h = h.trim_start_matches('0');
            let h = if h.is_empty() { "12" } else { h };
            let tail = local.format(":%M %p").to_string();
            return format!("{}{h}{tail}", local.format("%m/%d "));
        }
        if local.date_naive() == now.date_naive() {
            return local.format("%H:%M").to_string();
        }
        return local.format("%m/%d %H:%M").to_string();
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
