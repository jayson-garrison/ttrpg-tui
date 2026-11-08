use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::action::{Action, Tab, TabEvent, TabId};

/// Stands in for a tab that is not built yet.
pub struct PlaceholderTab {
    id: TabId,
    title: &'static str,
}

impl PlaceholderTab {
    pub fn new(id: TabId, title: &'static str) -> PlaceholderTab {
        PlaceholderTab { id, title }
    }
}

impl Tab for PlaceholderTab {
    fn id(&self) -> TabId {
        self.id
    }
    fn title(&self) -> &'static str {
        self.title
    }
    fn update(&mut self, _key: KeyEvent) -> Action {
        Action::None
    }
    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(format!("{} is not built yet.", self.title))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title(self.title)),
            area,
        );
    }
    fn captures_input(&self) -> bool {
        false
    }
    fn on_event(&mut self, _ev: &TabEvent) {}
    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
}
