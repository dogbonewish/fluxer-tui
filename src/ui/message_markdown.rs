//! Fluxer-style message markup for the TUI: **bold**, `code`, __underline__, ~~strike~~,
//! ||spoiler||, #- subtext, # headings, blockquotes, and ::: admonition fences.
//! Its not very good but eh, its something! i will improve it in the future :3
//! half of this does not properly work, so i'll have to fix it :(

use crate::app::App;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Delim {
    Bold,
    Italic,
    Code,
    Under,
    Strike,
    Spoiler,
}

impl Delim {
    fn open(self) -> &'static str {
        match self {
            Delim::Bold => "**",
            Delim::Italic => "*",
            Delim::Code => "`",
            Delim::Under => "__",
            Delim::Strike => "~~",
            Delim::Spoiler => "||",
        }
    }

    fn close(self) -> &'static str {
        self.open()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Admonition {
    Warning,
    Important,
    Note,
}

pub fn content_lines(content: &str, app: &App) -> Vec<Vec<Span<'static>>> {
    let mut out: Vec<Vec<Span<'static>>> = Vec::new();
    let mut fence: Option<Admonition> = None;

    for line in content.lines() {
        let t = line.trim();
        if t == ":::" {
            fence = None;
            out.push(vec![Span::raw("")]);
            continue;
        }
        if let Some(kind) = parse_admonition_open(t) {
            fence = Some(kind);
            out.push(admonition_title_line(kind));
            continue;
        }

        let bar_color = fence.map(admonition_bar_color);
        out.push(format_line(line, app, bar_color));
    }

    out
}

fn parse_admonition_open(trimmed: &str) -> Option<Admonition> {
    let rest = trimmed.strip_prefix(":::")?.trim();
    if rest.is_empty() {
        return None;
    }
    match rest.to_ascii_lowercase().as_str() {
        "warning" | "warn" => Some(Admonition::Warning),
        "important" | "caution" => Some(Admonition::Important),
        "note" | "info" => Some(Admonition::Note),
        _ => None,
    }
}

fn admonition_title_line(kind: Admonition) -> Vec<Span<'static>> {
    let (label, color) = match kind {
        Admonition::Warning => (" WARNING ", crate::ui::theme::DANGER),
        Admonition::Important => (" IMPORTANT ", crate::ui::theme::ACCENT),
        Admonition::Note => (" NOTE ", crate::ui::theme::TEXT_DIM),
    };
    vec![Span::styled(
        format!("━━{label}━━"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )]
}

fn admonition_bar_color(kind: Admonition) -> ratatui::style::Color {
    match kind {
        Admonition::Warning => crate::ui::theme::DANGER,
        Admonition::Important => crate::ui::theme::ACCENT,
        Admonition::Note => crate::ui::theme::TEXT_DIM,
    }
}

#[derive(Clone, Copy)]
enum QuoteCallout {
    Warning,
    Important,
    Note,
}

fn starts_ic(haystack: &str, pat: &str) -> bool {
    let hb = haystack.as_bytes();
    let pb = pat.as_bytes();
    hb.len() >= pb.len() && hb[..pb.len()].eq_ignore_ascii_case(pb)
}

fn detect_quote_callout(body: &str) -> Option<QuoteCallout> {
    let t = body.trim_start();
    if starts_ic(t, "[!WARNING]") || starts_ic(t, "**WARNING") {
        return Some(QuoteCallout::Warning);
    }
    if starts_ic(t, "[!IMPORTANT]") || starts_ic(t, "**IMPORTANT") || starts_ic(t, "IMPORTANT:") {
        return Some(QuoteCallout::Important);
    }
    if starts_ic(t, "[!NOTE]") || starts_ic(t, "**NOTE") || starts_ic(t, "NOTE:") {
        return Some(QuoteCallout::Note);
    }
    None
}

fn strip_callout_tag(body: &str, kind: QuoteCallout) -> &str {
    let t = body.trim_start();
    let pats: &[&str] = match kind {
        QuoteCallout::Warning => &["[!WARNING]", "**WARNING:**", "**WARNING**"],
        QuoteCallout::Important => &[
            "[!IMPORTANT]",
            "**IMPORTANT:**",
            "**IMPORTANT**",
            "IMPORTANT:",
        ],
        QuoteCallout::Note => &["[!NOTE]", "**NOTE:**", "**NOTE**", "NOTE:"],
    };
    for p in pats {
        if starts_ic(t, p) {
            return t[p.len()..].trim_start();
        }
    }
    t
}

fn quote_callout_bar_fg(kind: QuoteCallout) -> Color {
    match kind {
        QuoteCallout::Warning => crate::ui::theme::DANGER,
        QuoteCallout::Important => crate::ui::theme::ACCENT,
        QuoteCallout::Note => crate::ui::theme::VOICE_COLOR,
    }
}

fn quote_callout_badge(kind: QuoteCallout) -> Span<'static> {
    match kind {
        QuoteCallout::Warning => Span::styled(
            " ! WARNING ",
            Style::default()
                .fg(Color::Black)
                .bg(crate::ui::theme::DANGER)
                .add_modifier(Modifier::BOLD),
        ),
        QuoteCallout::Important => Span::styled(
            " ! IMPORTANT ",
            Style::default()
                .fg(Color::Black)
                .bg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        QuoteCallout::Note => Span::styled(
            " NOTE ",
            Style::default()
                .fg(Color::Black)
                .bg(crate::ui::theme::TEXT_DIM)
                .add_modifier(Modifier::BOLD),
        ),
    }
}

fn quote_callout_body_base(kind: QuoteCallout) -> Style {
    match kind {
        QuoteCallout::Warning => Style::default()
            .fg(crate::ui::theme::TEXT)
            .add_modifier(Modifier::BOLD),
        QuoteCallout::Important => Style::default()
            .fg(crate::ui::theme::ACCENT)
            .add_modifier(Modifier::BOLD),
        QuoteCallout::Note => Style::default()
            .fg(crate::ui::theme::TEXT)
            .add_modifier(Modifier::BOLD),
    }
}

fn format_line(
    line: &str,
    app: &App,
    admon_bar: Option<ratatui::style::Color>,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    if let Some(c) = admon_bar {
        spans.push(Span::styled("┃ ", Style::default().fg(c)));
    }

    let (lead, mut rest) = split_leading_ws(line);
    if !lead.is_empty() {
        spans.push(Span::raw(lead.to_string()));
    }

    if let Some((_q_prefix, q_body)) = strip_blockquote(rest) {
        if let Some(callout) = detect_quote_callout(q_body) {
            let bar = quote_callout_bar_fg(callout);
            spans.push(Span::styled(
                "> ",
                Style::default().fg(bar).add_modifier(Modifier::BOLD),
            ));
            spans.push(quote_callout_badge(callout));
            spans.push(Span::raw(" "));
            let after_tag = strip_callout_tag(q_body, callout);
            let base = quote_callout_body_base(callout);
            for s in parse_message_spans(after_tag, app) {
                spans.push(Span::styled(s.content.to_string(), base.patch(s.style)));
            }
            return spans;
        }

        let qc = quote_accent_color(q_body).unwrap_or(crate::ui::theme::TEXT_MUTED);
        spans.push(Span::styled("> ", Style::default().fg(qc)));
        rest = q_body;
    }

    if let Some(body) = rest.strip_prefix("-# ") {
        spans.push(Span::styled(
            "-# ",
            Style::default()
                .fg(crate::ui::theme::TEXT_MUTED)
                .add_modifier(Modifier::ITALIC),
        ));
        spans.extend(parse_message_spans(body, app));
        return spans;
    }

    if let Some((level, body)) = strip_atx_heading(rest) {
        let hashes = format!("{} ", "#".repeat(level));
        spans.push(Span::styled(
            hashes,
            Style::default()
                .fg(crate::ui::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.extend(parse_message_spans(body, app));
        return spans;
    }

    spans.extend(parse_message_spans(rest, app));
    spans
}

fn split_leading_ws(s: &str) -> (&str, &str) {
    let t = s.trim_start();
    let n = s.len() - t.len();
    (&s[..n], t)
}

fn strip_blockquote(s: &str) -> Option<(&'static str, &str)> {
    let t = s.trim_start();
    if !t.starts_with('>') {
        return None;
    }
    let after = t.strip_prefix('>')?;
    let body = after.strip_prefix(' ').unwrap_or(after);
    Some(("> ", body))
}

fn quote_accent_color(body: &str) -> Option<ratatui::style::Color> {
    let u = body.trim_start().to_uppercase();
    if u.starts_with("**WARNING") || u.starts_with("WARNING") || u.contains("[!WARNING]") {
        return Some(crate::ui::theme::DANGER);
    }
    if u.starts_with("**IMPORTANT") || u.starts_with("IMPORTANT") || u.contains("[!IMPORTANT]") {
        return Some(crate::ui::theme::ACCENT);
    }
    if u.starts_with("**NOTE") || u.starts_with("NOTE") || u.contains("[!NOTE]") {
        return Some(crate::ui::theme::VOICE_COLOR);
    }
    None
}

fn strip_atx_heading(s: &str) -> Option<(usize, &str)> {
    let bytes = s.as_bytes();
    let mut n = 0usize;
    while n < bytes.len() && n < 3 && bytes[n] == b'#' {
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let after = s.get(n..)?;
    let after = after.strip_prefix(' ')?;
    Some((n, after))
}

pub fn parse_message_spans(text: &str, app: &App) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            match chars.peek() {
                Some(&'#') => {
                    chars.next();
                    let id = collect_until_close(&mut chars);
                    if !id.is_empty() {
                        flush_markdown_buffer(&mut buf, &mut spans, app, Style::default());
                        let name = resolve_channel_name(app, &id);
                        spans.push(Span::styled(
                            format!("#{name}"),
                            Style::default().fg(crate::ui::theme::LINK_COLOR),
                        ));
                    } else {
                        buf.push('<');
                        buf.push('#');
                    }
                }
                Some(&'@') => {
                    chars.next();
                    let is_role = chars.peek() == Some(&'&');
                    let is_nick = chars.peek() == Some(&'!');
                    if is_role || is_nick {
                        chars.next();
                    }
                    let id = collect_until_close(&mut chars);
                    if !id.is_empty() {
                        flush_markdown_buffer(&mut buf, &mut spans, app, Style::default());
                        if is_role {
                            spans.push(role_mention_span(app, &id));
                        } else {
                            let name = resolve_user_name(app, &id);
                            let is_self = id == app.me.id;
                            let g = app.guild_id_for_active_channel();
                            let fg = app.member_name_color(g.as_deref(), id.as_str(), is_self);
                            spans.push(Span::styled(format!("@{name}"), Style::default().fg(fg)));
                        }
                    } else {
                        buf.push('<');
                        buf.push('@');
                        if is_role {
                            buf.push('&');
                        }
                        if is_nick {
                            buf.push('!');
                        }
                    }
                }
                Some(&'t') => {
                    chars.next();
                    if chars.peek() == Some(&':') {
                        chars.next();
                        let inner = collect_until_close(&mut chars);
                        if !inner.is_empty() {
                            flush_markdown_buffer(&mut buf, &mut spans, app, Style::default());
                            let formatted = format_discord_timestamp(&inner);
                            spans.push(Span::styled(
                                formatted,
                                Style::default().fg(crate::ui::theme::TEXT_DIM),
                            ));
                        } else {
                            buf.push_str("<t:");
                        }
                    } else {
                        buf.push('<');
                        buf.push('t');
                    }
                }
                Some(&':') | Some(&'a') => {
                    let mut is_animated = false;
                    if chars.peek() == Some(&'a') {
                        let a = chars.next().unwrap();
                        if chars.peek() == Some(&':') {
                            is_animated = true;
                            chars.next();
                        } else {
                            buf.push(ch);
                            buf.push(a);
                            continue;
                        }
                    } else {
                        chars.next();
                    }

                    let mut inner = String::new();
                    let mut valid = false;
                    for c in chars.by_ref() {
                        if c == '>' {
                            valid = true;
                            break;
                        }
                        inner.push(c);
                    }

                    if valid && inner.contains(':') {
                        flush_markdown_buffer(&mut buf, &mut spans, app, Style::default());
                        let parts: Vec<&str> = inner.split(':').collect();
                        let name_idx = if parts.len() >= 2 { parts.len() - 2 } else { 0 };
                        let emoji_name = format!(":{}:", parts.get(name_idx).unwrap_or(&"unknown"));
                        let style = if is_animated {
                            Style::default()
                                .fg(crate::ui::theme::ACCENT)
                                .add_modifier(Modifier::ITALIC)
                        } else {
                            Style::default().fg(crate::ui::theme::EMOJI_UNKNOWN)
                        };
                        spans.push(Span::styled(emoji_name, style));
                    } else {
                        buf.push('<');
                        if is_animated {
                            buf.push('a');
                        }
                        buf.push(':');
                        buf.push_str(&inner);
                    }
                }
                _ => {
                    buf.push(ch);
                }
            }
        } else {
            buf.push(ch);
        }
    }

    flush_markdown_buffer(&mut buf, &mut spans, app, Style::default());
    spans
}

fn collect_until_close(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut result = String::new();
    for c in chars.by_ref() {
        if c == '>' {
            return result;
        }
        result.push(c);
    }
    String::new()
}

enum SpecialMatch<'a> {
    Delim {
        idx: usize,
        delim: Delim,
    },
    Link {
        idx: usize,
        end: usize,
        label: &'a str,
    },
    Autolink {
        idx: usize,
        end: usize,
        url: &'a str,
    },
}

impl SpecialMatch<'_> {
    fn idx(&self) -> usize {
        match self {
            Self::Delim { idx, .. } | Self::Link { idx, .. } | Self::Autolink { idx, .. } => *idx,
        }
    }
}

fn is_italic_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| ch.is_whitespace() || (!ch.is_alphanumeric() && ch != '_'))
}

fn can_open_italic(text: &str, idx: usize) -> bool {
    let prev = text[..idx].chars().next_back();
    let next = text[idx + 1..].chars().next();
    next.is_some_and(|ch| !ch.is_whitespace() && ch != '*') && is_italic_boundary(prev)
}

fn can_close_italic(text: &str, idx: usize) -> bool {
    let prev = text[..idx].chars().next_back();
    let next = text[idx + 1..].chars().next();
    prev.is_some_and(|ch| !ch.is_whitespace() && ch != '*') && is_italic_boundary(next)
}

fn next_delim(rest: &str) -> Option<(usize, Delim)> {
    let mut best: Option<(usize, Delim)> = None;
    // Note: order matters - check ** BEFORE *, so ** doesn't get caught as two *
    for (pat, d) in [
        ("**", Delim::Bold),
        ("__", Delim::Under),
        ("~~", Delim::Strike),
        ("||", Delim::Spoiler),
        ("`", Delim::Code),
    ] {
        if let Some(i) = rest.find(pat) {
            best = Some(match best {
                None => (i, d),
                Some((j, _)) if i < j => (i, d),
                Some(b) => b,
            });
        }
    }
    for (idx, ch) in rest.char_indices() {
        if ch != '*' {
            continue;
        }
        if rest.as_bytes().get(idx.saturating_sub(1)) == Some(&b'*')
            || rest.as_bytes().get(idx + 1) == Some(&b'*')
            || !can_open_italic(rest, idx)
        {
            continue;
        }
        best = Some(match best {
            None => (idx, Delim::Italic),
            Some((j, _)) if idx < j => (idx, Delim::Italic),
            Some(existing) => existing,
        });
        break;
    }
    best
}

fn flush_markdown_buffer(buf: &mut String, spans: &mut Vec<Span<'static>>, app: &App, base: Style) {
    if buf.is_empty() {
        return;
    }
    let text = std::mem::take(buf);
    parse_markdown_segments(&text, spans, app, base);
}

fn find_link(text: &str) -> Option<(usize, usize, &str)> {
    for (open, ch) in text.char_indices() {
        if ch != '[' {
            continue;
        }
        if text[..open].chars().next_back() == Some('!') {
            continue;
        }
        let close_bracket = text[open + 1..].find(']')? + open + 1;
        if text.as_bytes().get(close_bracket + 1) != Some(&b'(') {
            continue;
        }
        let url_start = close_bracket + 2;
        let url_end = text[url_start..].find(')')? + url_start;
        if url_start == url_end {
            continue;
        }
        return Some((open, url_end + 1, &text[open + 1..close_bracket]));
    }
    None
}

fn find_autolink(text: &str) -> Option<(usize, usize, &str)> {
    for (open, ch) in text.char_indices() {
        if ch != '<' {
            continue;
        }
        let url_start = open + 1;
        let url_end = text[url_start..].find('>')? + url_start;
        let url = &text[url_start..url_end];
        if url.starts_with("http://") || url.starts_with("https://") {
            return Some((open, url_end + 1, url));
        }
    }
    None
}

fn next_special(rest: &str) -> Option<SpecialMatch<'_>> {
    let mut best = next_delim(rest).map(|(idx, delim)| SpecialMatch::Delim { idx, delim });

    if let Some((idx, end, label)) = find_link(rest)
        && best.as_ref().is_none_or(|current| idx < current.idx())
    {
        best = Some(SpecialMatch::Link { idx, end, label });
    }

    if let Some((idx, end, url)) = find_autolink(rest)
        && best.as_ref().is_none_or(|current| idx < current.idx())
    {
        best = Some(SpecialMatch::Autolink { idx, end, url });
    }

    best
}

fn find_matching_italic_close(text: &str) -> Option<usize> {
    for (idx, ch) in text.char_indices() {
        if ch != '*'
            || text.as_bytes().get(idx.saturating_sub(1)) == Some(&b'*')
            || text.as_bytes().get(idx + 1) == Some(&b'*')
            || !can_close_italic(text, idx)
        {
            continue;
        }
        return Some(idx);
    }
    None
}

fn parse_markdown_segments(text: &str, spans: &mut Vec<Span<'static>>, app: &App, base: Style) {
    let mut rest = text;
    while !rest.is_empty() {
        let Some(next) = next_special(rest) else {
            flush_text_with_emoji_str(rest, spans, base);
            return;
        };
        let idx = next.idx();
        if idx > 0 {
            flush_text_with_emoji_str(&rest[..idx], spans, base);
        }

        match next {
            SpecialMatch::Link { end, label, .. } => {
                let link_style = Style::default().fg(crate::ui::theme::LINK_COLOR);
                for s in parse_message_spans(label, app) {
                    spans.push(Span::styled(
                        s.content.to_string(),
                        s.style.patch(link_style),
                    ));
                }
                rest = &rest[end..];
                continue;
            }
            SpecialMatch::Autolink { end, url, .. } => {
                spans.push(Span::styled(
                    url.to_string(),
                    base.patch(Style::default().fg(crate::ui::theme::LINK_COLOR)),
                ));
                rest = &rest[end..];
                continue;
            }
            SpecialMatch::Delim { delim, .. } => {
                let open = delim.open();
                rest = &rest[idx + open.len()..];
                let close_idx = match delim {
                    Delim::Italic => find_matching_italic_close(rest),
                    _ => rest.find(delim.close()),
                };
                let Some(close_idx) = close_idx else {
                    flush_text_with_emoji_str(open, spans, base);
                    continue;
                };
                let inner = &rest[..close_idx];
                rest = &rest[close_idx + delim.close().len()..];
                match delim {
                    Delim::Bold => {
                        let inner_spans = parse_message_spans(inner, app);
                        for s in inner_spans {
                            spans.push(Span::styled(
                                s.content.to_string(),
                                s.style.add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                    Delim::Under => {
                        let inner_spans = parse_message_spans(inner, app);
                        for s in inner_spans {
                            spans.push(Span::styled(
                                s.content.to_string(),
                                s.style.add_modifier(Modifier::UNDERLINED),
                            ));
                        }
                    }
                    Delim::Strike => {
                        let inner_spans = parse_message_spans(inner, app);
                        for s in inner_spans {
                            spans.push(Span::styled(
                                s.content.to_string(),
                                s.style.add_modifier(Modifier::CROSSED_OUT),
                            ));
                        }
                    }
                    Delim::Spoiler => {
                        let inner_spans = parse_message_spans(inner, app);
                        for s in inner_spans {
                            spans.push(Span::styled(
                                s.content.to_string(),
                                s.style
                                    .fg(crate::ui::theme::TEXT_MUTED)
                                    .add_modifier(Modifier::DIM),
                            ));
                        }
                    }
                    Delim::Italic => {
                        let inner_spans = parse_message_spans(inner, app);
                        for s in inner_spans {
                            spans.push(Span::styled(
                                s.content.to_string(),
                                s.style.add_modifier(Modifier::ITALIC),
                            ));
                        }
                    }
                    Delim::Code => {
                        spans.push(Span::styled(
                            inner.to_string(),
                            Style::default()
                                .fg(crate::ui::theme::TEXT)
                                .bg(crate::ui::theme::BG_TERTIARY),
                        ));
                    }
                }
            }
        }
    }
}

fn flush_text_with_emoji_str(text: &str, spans: &mut Vec<Span<'static>>, base: Style) {
    if text.is_empty() {
        return;
    }
    let mut remaining = text;
    while !remaining.is_empty() {
        if let Some(emoji) = emojis::get(remaining) {
            let elen = emoji.as_str().len();
            spans.push(Span::styled(
                emoji.as_str().to_string(),
                base.fg(crate::ui::theme::EMOJI_UNKNOWN),
            ));
            remaining = &remaining[elen.min(remaining.len())..];
            continue;
        }

        let mut found_at = None;
        for (i, _) in remaining.char_indices().skip(1) {
            if emojis::get(&remaining[i..]).is_some() {
                found_at = Some(i);
                break;
            }
        }

        match found_at {
            Some(pos) => {
                spans.push(Span::styled(remaining[..pos].to_string(), base));
                remaining = &remaining[pos..];
            }
            None => {
                spans.push(Span::styled(remaining.to_string(), base));
                break;
            }
        }
    }
}

fn role_mention_span(app: &App, role_id: &str) -> Span<'static> {
    let tail = role_id
        .len()
        .checked_sub(4)
        .map(|i| &role_id[i..])
        .unwrap_or(role_id);
    let fallback = Style::default()
        .fg(crate::ui::theme::ACCENT)
        .add_modifier(Modifier::BOLD);

    let Some(gid) = app.guild_id_for_active_channel() else {
        return Span::styled(format!("@role-{tail}"), fallback);
    };
    let Some(roles) = app.guild_roles.get(&gid) else {
        return Span::styled(
            format!("@role-{tail}"),
            fallback.add_modifier(Modifier::UNDERLINED),
        );
    };
    let Some(role) = roles.iter().find(|r| r.id.trim() == role_id.trim()) else {
        return Span::styled(format!("@role-{tail}"), fallback);
    };
    let name = if role.name.trim().is_empty() {
        format!("role-{tail}")
    } else {
        role.name.clone()
    };
    Span::styled(
        format!("@{name}"),
        crate::ui::theme::role_mention_style(role.color).add_modifier(Modifier::UNDERLINED),
    )
}

fn resolve_channel_name(app: &App, id: &str) -> String {
    for channels in app.guild_channels.values() {
        if let Some(ch) = channels.iter().find(|c| c.id == id) {
            return ch.name.clone();
        }
    }
    for ch in &app.private_channels {
        if ch.id == id {
            return ch.name.clone();
        }
    }
    format!("unknown-{}", &id[id.len().saturating_sub(4)..])
}

fn resolve_user_name(app: &App, id: &str) -> String {
    let active_gid = app.guild_id_for_active_channel();
    if let Some(gid) = active_gid.as_deref() {
        if let Some(members) = app.guild_members.get(gid) {
            if let Some(m) = members.iter().find(|m| m.user.id == id) {
                let u = app.user_cache.get(id).unwrap_or(&m.user);
                return app.shown_name_for_user(Some(gid), u);
            }
        }
        if let Some(u) = app.user_cache.get(id) {
            return app.shown_name_for_user(Some(gid), u);
        }
    }
    if let Some(u) = app.user_cache.get(id) {
        return app.shown_name_for_user(None, u);
    }
    for (gid, members) in &app.guild_members {
        if let Some(m) = members.iter().find(|m| m.user.id == id) {
            let u = app.user_cache.get(id).unwrap_or(&m.user);
            return app.shown_name_for_user(Some(gid.as_str()), u);
        }
    }
    format!("user-{}", &id[id.len().saturating_sub(4)..])
}

fn format_discord_timestamp(inner: &str) -> String {
    let parts: Vec<&str> = inner.splitn(2, ':').collect();
    let unix_str = parts[0];
    if let Ok(ts) = unix_str.parse::<i64>()
        && let Some(dt) = chrono::DateTime::from_timestamp(ts, 0)
    {
        let local = dt.with_timezone(&chrono::Local);
        return local.format("%Y-%m-%d %H:%M").to_string();
    }
    format!("<t:{inner}>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::WellKnownFluxerResponse;
    use crate::app::ServerSelection;
    use crate::config::UiSettings;

    fn test_app() -> App {
        App::new(
            WellKnownFluxerResponse::default(),
            crate::api::types::UserPrivateResponse {
                id: "me".to_string(),
                ..crate::api::types::UserPrivateResponse::default()
            },
            None,
            Vec::new(),
            Vec::new(),
            ServerSelection::DirectMessages,
            None,
            UiSettings::default(),
        )
    }

    fn joined_text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn parses_single_asterisk_italics() {
        let app = test_app();
        let spans = parse_message_spans("alpha *bravo* charlie", &app);

        assert_eq!(joined_text(&spans), "alpha bravo charlie");
        assert!(
            spans.iter().any(|span| span.content == "bravo"
                && span.style.add_modifier.contains(Modifier::ITALIC))
        );
    }

    #[test]
    fn parses_markdown_links_and_autolinks_without_literal_markup() {
        let app = test_app();
        let spans = parse_message_spans(
            "[Fluxer](https://fluxer.app) and <https://fluxer.app>",
            &app,
        );

        assert_eq!(joined_text(&spans), "Fluxer and https://fluxer.app");
        assert!(
            spans
                .iter()
                .filter(|span| span.style.fg == Some(crate::ui::theme::LINK_COLOR))
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>()
                .contains(&"Fluxer")
        );
        assert!(
            spans
                .iter()
                .filter(|span| span.style.fg == Some(crate::ui::theme::LINK_COLOR))
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>()
                .contains(&"https://fluxer.app")
        );
    }
}
