use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Minimum usable terminal size (D-36).
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 20;

pub fn is_too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

pub fn render(frame: &mut Frame, area: Rect) {
    let text = format!(
        "Terminal too small: need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}. Resize to continue.",
        area.width, area.height
    );
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
}
