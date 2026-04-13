use crate::app::{App, ImagePreviewState};
use crate::ui::ansi_line;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

fn overlay_body_split(area: Rect) -> (Rect, Rect) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(area);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(20),
            Constraint::Length(2),
        ])
        .split(outer[1]);

    let popup = mid[1];
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(popup);

    (body[0], body[1])
}

pub fn overlay_chafa_cells(area: Rect) -> (u16, u16) {
    let (content, _) = overlay_body_split(area);
    (
        content.width.saturating_sub(2).max(12),
        content.height.saturating_sub(2).max(6),
    )
}

/// Place `child` (w×h) inside `outer`, centered; clamps to `outer` if needed.
fn center_subrect(outer: Rect, child_w: u16, child_h: u16) -> Rect {
    let w = child_w.min(outer.width);
    let h = child_h.min(outer.height);
    let x = outer.x + outer.width.saturating_sub(w) / 2;
    let y = outer.y + outer.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

fn render_bitmap_like(
    frame: &mut Frame<'_>,
    content: Rect,
    footer_row: Rect,
    title: &str,
    protocol: &mut StatefulProtocol,
    accent: Style,
    text_style: Style,
    is_gif: bool,
) {
    let title_part = if is_gif {
        format!(" {title} · GIF ")
    } else {
        format!(" {title} ")
    };
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Image ", accent),
            Span::styled(title_part, text_style),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));
    let inner = block.inner(content);
    let resize = Resize::Scale(None);
    let fit = protocol.size_for(resize.clone(), inner);
    let img_area = center_subrect(inner, fit.width, fit.height);
    let fill = Block::default().style(Style::default().bg(crate::ui::theme::BG));
    frame.render_widget(fill, inner);
    let img = StatefulImage::default().resize(resize);
    frame.render_stateful_widget(img, img_area, protocol);
    frame.render_widget(block, content);
    let footer = if is_gif {
        " GIF · Esc / q - close "
    } else {
        " Esc / q - close "
    };
    let hint = Paragraph::new(Line::from(Span::styled(
        footer,
        Style::default().fg(crate::ui::theme::TEXT_MUTED),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(hint, footer_row);
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(state) = app.image_preview.as_mut() else {
        return;
    };

    frame.render_widget(Clear, area);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(area);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(20),
            Constraint::Length(2),
        ])
        .split(outer[1]);

    let popup = mid[1];
    frame.render_widget(Clear, popup);

    let (content, footer_row) = overlay_body_split(area);
    let text_style = Style::default().fg(crate::ui::theme::TEXT);
    let accent = Style::default()
        .fg(crate::ui::theme::ACCENT)
        .add_modifier(Modifier::BOLD);

    match state {
        ImagePreviewState::Loading { title } => {
            let title = title.as_str();
            let block = Block::default()
                .title(Line::from(Span::styled(" Image preview ", accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));
            let p = Paragraph::new(Text::from(vec![Line::from(Span::styled(
                format!("Loading… {title}"),
                text_style,
            ))]))
            .block(block)
            .alignment(Alignment::Left);
            frame.render_widget(p, content);
            let hint = Paragraph::new(Line::from(Span::styled(
                " Esc - cancel ",
                Style::default().fg(crate::ui::theme::TEXT_MUTED),
            )))
            .alignment(Alignment::Center);
            frame.render_widget(hint, footer_row);
        }
        ImagePreviewState::Failed { message } => {
            let block = Block::default()
                .title(Line::from(Span::styled(" Image preview ", accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));
            let p = Paragraph::new(Text::from(vec![Line::from(Span::styled(
                message.clone(),
                text_style,
            ))]))
            .block(block)
            .alignment(Alignment::Left);
            frame.render_widget(p, content);
            let hint = Paragraph::new(Line::from(Span::styled(
                " Esc / q - close ",
                Style::default().fg(crate::ui::theme::TEXT_MUTED),
            )))
            .alignment(Alignment::Center);
            frame.render_widget(hint, footer_row);
        }
        ImagePreviewState::ReadyBitmap { title, protocol } => {
            render_bitmap_like(
                frame,
                content,
                footer_row,
                title.as_str(),
                protocol,
                accent,
                text_style,
                false,
            );
        }
        ImagePreviewState::ReadyAnimatedGif {
            title,
            current_protocol,
            ..
        } => {
            render_bitmap_like(
                frame,
                content,
                footer_row,
                title.as_str(),
                current_protocol,
                accent,
                text_style,
                true,
            );
        }
        ImagePreviewState::ReadyChafa {
            title,
            lines,
            scroll,
        } => {
            let inner_h = content.height.saturating_sub(2).max(1) as usize;
            let max_scroll = lines.len().saturating_sub(inner_h);
            let scroll_pos = (*scroll).min(max_scroll);
            let end = (scroll_pos + inner_h).min(lines.len());
            let slice = &lines[scroll_pos..end];
            let mut out_lines: Vec<Line> = slice
                .iter()
                .map(|row| ansi_line::line_from_ansi(row))
                .collect();
            if max_scroll == 0 && out_lines.len() < inner_h {
                let pad = (inner_h - out_lines.len()) / 2;
                out_lines.splice(0..0, std::iter::repeat(Line::default()).take(pad));
            }
            let block = Block::default()
                .title(Line::from(vec![
                    Span::styled(" Image ", accent),
                    Span::styled(format!(" {title} "), text_style),
                ]))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(crate::ui::theme::ACCENT_DIM));
            let p = Paragraph::new(Text::from(out_lines))
                .block(block)
                .alignment(Alignment::Left);
            frame.render_widget(p, content);
            let footer = if max_scroll > 0 {
                " ↑/↓ PgUp/PgDn · j/k - scroll · Esc / q - close "
            } else {
                " Esc / q - close "
            };
            let hint = Paragraph::new(Line::from(Span::styled(
                footer,
                Style::default().fg(crate::ui::theme::TEXT_MUTED),
            )))
            .alignment(Alignment::Center);
            frame.render_widget(hint, footer_row);
        }
    }
}
