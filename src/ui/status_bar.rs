use crate::app::{App, Focus, ServerSelection};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let server = match &app.selected_server {
        ServerSelection::DirectMessages => "DMs".to_string(),
        ServerSelection::Guild(id) => app
            .guilds
            .iter()
            .find(|g| g.id == *id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| id.clone()),
    };

    let channel = app
        .active_channel()
        .map(|c| format!("#{}", c.name))
        .unwrap_or_default();

    let status_msg = if app.status_message.is_empty() {
        String::new()
    } else {
        format!(" | {}", app.status_message)
    };

    let hints = if app.focus == Focus::Messages {
        if app.selected_message_index.is_some() {
            " | r:reply f:fwd e:react Esc:deselect"
        } else {
            " | s:select i:input"
        }
    } else {
        ""
    };

    let api_hint = if app.discovery.api_code_version > 0 {
        format!(" api{}", app.discovery.api_code_version)
    } else {
        String::new()
    };

    let paragraph = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            app.gateway_status.label(),
            crate::ui::theme::gateway_status_style(app.gateway_status),
        ),
        Span::styled(api_hint, Style::default().fg(crate::ui::theme::TEXT_MUTED)),
        Span::styled(
            format!("  {server} {channel}{status_msg}"),
            Style::default().fg(crate::ui::theme::TEXT_DIM),
        ),
        Span::styled(hints, Style::default().fg(crate::ui::theme::TEXT_MUTED)),
    ]))
    .style(Style::default().bg(crate::ui::theme::BG_TERTIARY));
    frame.render_widget(paragraph, area);
}
