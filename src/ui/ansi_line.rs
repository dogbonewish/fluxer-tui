use ansi_to_tui::IntoText as _;
use ratatui::text::Line;

pub fn line_from_ansi(row: &str) -> Line<'static> {
    match row.as_bytes().into_text() {
        Ok(t) => t
            .lines
            .into_iter()
            .next()
            .unwrap_or_else(|| Line::from(row.to_string())),
        Err(_) => Line::from(row.to_string()),
    }
}
