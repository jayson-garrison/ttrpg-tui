use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::theme;

const GLOBAL: &[(&str, &str)] = &[
    ("1-4", "switch tab"),
    ("Tab / Shift-Tab", "next / previous tab"),
    ("?", "this help"),
    ("q / Ctrl-C", "quit (Ctrl-C leaves a text field first)"),
];

/// A centered modal listing the global keys and the active tab's keys.
pub fn render(frame: &mut Frame, area: Rect, tab_title: &str, tab_keys: &[(&str, &str)]) {
    let mut lines = vec![Line::styled("Global", theme::bold())];
    lines.extend(
        GLOBAL
            .iter()
            .map(|(k, d)| Line::from(format!("  {k:<16}{d}"))),
    );
    lines.push(Line::from(""));
    lines.push(Line::styled(tab_title.to_string(), theme::bold()));
    if tab_keys.is_empty() {
        lines.push(Line::styled("  (no keys yet)", theme::dim()));
    }
    lines.extend(
        tab_keys
            .iter()
            .map(|(k, d)| Line::from(format!("  {k:<16}{d}"))),
    );
    lines.push(Line::from(""));
    lines.push(Line::styled("Press any key to close", theme::dim()));

    let width = area.width.min(64);
    let height = (lines.len() as u16).saturating_add(2).min(area.height);
    let modal = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: false }),
        modal,
    );
}
