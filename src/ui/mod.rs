pub mod ansi_line;
pub mod channel_picker;
pub mod command_popup;
pub mod emoji_popup;
pub mod help_overlay;
pub mod image_preview;
pub mod input_bar;
pub(crate) mod input_word_wrap;
pub mod mention_popup;
pub mod message_markdown;
pub mod message_pane;
pub mod settings_overlay;
pub mod sidebar;
pub mod status_bar;
pub mod theme;

use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Clear, Paragraph};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    app.chafa_preview_cells = image_preview::overlay_chafa_cells(area);
    const MIN_W: u16 = 28;
    const MIN_H: u16 = 8;
    if area.width < MIN_W || area.height < MIN_H {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(format!(
                "Terminal too small (need at least {MIN_W}×{MIN_H}). Enlarge the window or reduce font size."
            ))
            .style(Style::default().fg(crate::ui::theme::TEXT)),
            area,
        );
        return;
    }

    let inner_w = area.width.saturating_sub(2).max(1);
    let input_lines = input_bar::input_display_row_count(app, inner_w);
    let input_block_h = input_lines.saturating_add(2).clamp(3, 40);

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(input_block_h),
        ])
        .split(area);

    status_bar::render(frame, root[0], app);

    let sidebar_width = sidebar_width(area.width);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
        .split(root[1]);

    let server_height = server_list_height(body[0].height);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(server_height), Constraint::Min(1)])
        .split(body[0]);

    sidebar::render_servers(frame, sidebar[0], app);
    sidebar::render_channels(frame, sidebar[1], app);
    app.chafa_viewport = (
        body[1].width.saturating_sub(2).max(12),
        body[1].height.saturating_sub(2).max(6),
    );
    message_pane::render(frame, body[1], app);

    if app.emoji_autocomplete.is_some() {
        emoji_popup::render(frame, root[1], app);
    }
    if app.mention_autocomplete.is_some() {
        mention_popup::render(frame, root[1], app);
    }
    if app.command_autocomplete.is_some() {
        command_popup::render(frame, root[1], app);
    }

    if let Some(cursor) = input_bar::render(frame, root[2], app) {
        frame.set_cursor_position(cursor);
    }

    if app.show_help {
        help_overlay::render(frame, area, app);
    } else if app.show_settings {
        settings_overlay::render(frame, area, app);
    } else if app.image_preview.is_some() {
        image_preview::render(frame, area, app);
    } else if app.channel_picker.is_some() {
        channel_picker::render(frame, area, app);
    }
}

fn sidebar_width(terminal_width: u16) -> u16 {
    let w = terminal_width / 4;
    w.clamp(22, 50)
}

fn server_list_height(sidebar_height: u16) -> u16 {
    const CHANNEL_BLOCK_MIN: u16 = 3;
    if sidebar_height <= CHANNEL_BLOCK_MIN {
        return 1;
    }
    let max_for_server = sidebar_height.saturating_sub(CHANNEL_BLOCK_MIN);
    let desired = (sidebar_height * 3 / 10).clamp(3, 12);
    desired.min(max_for_server).max(1)
}
