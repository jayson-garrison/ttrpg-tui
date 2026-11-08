use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::theme;

/// Row 0: `1 Oracle  2 Library ...` on the left, `seed N  warnings K` on the
/// right. The warnings count is bold-yellow when above zero, and always text.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    titles: &[&str],
    active: usize,
    seed: u64,
    warnings: usize,
) {
    let mut spans = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        let style = if i == active {
            theme::active_tab()
        } else {
            Default::default()
        };
        spans.push(Span::styled(format!(" {} {} ", i + 1, title), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    let warn_style = if warnings > 0 {
        theme::warning()
    } else {
        theme::dim()
    };
    let right = Line::from(vec![
        Span::styled(format!("seed {seed}  "), theme::dim()),
        Span::styled(format!("warnings {warnings} "), warn_style),
    ]);
    frame.render_widget(Paragraph::new(right).alignment(Alignment::Right), area);
}
