//! Styles in one place. Color is never the only signal (D-79): every styled
//! thing also has a text twin (a label or marker), and focus uses reverse video.

use ratatui::style::{Color, Modifier, Style};

pub fn active_tab() -> Style {
    Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

pub fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub fn error() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub fn warning() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

pub fn ok() -> Style {
    Style::new().fg(Color::Green)
}
