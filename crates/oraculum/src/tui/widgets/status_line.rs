use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::action::{Toast, ToastLevel};
use crate::tui::theme;

/// Last row: the toast if there is one, else the active tab's key hints.
pub fn render(frame: &mut Frame, area: Rect, toast: Option<&Toast>, hints: &[(&str, &str)]) {
    let line = match toast {
        Some(t) => match t.level {
            ToastLevel::Info => Line::from(Span::raw(format!(" {}", t.text))),
            ToastLevel::Error => Line::from(Span::styled(format!(" ! {}", t.text), theme::error())),
        },
        None => {
            let mut text = String::from(" ");
            for (keys, what) in hints {
                text.push_str(&format!("{keys} {what}  "));
            }
            text.push_str("? help");
            Line::from(Span::styled(text, theme::dim()))
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}
