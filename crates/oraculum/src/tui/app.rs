//! The `App`: owns the tabs, applies A§5.4's key order, executes `Action`s,
//! and draws the frame. Pure: no terminal, so tests drive it directly (D-80).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oraculum_core::clock::Clock;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use super::action::{Action, AppInfo, Tab, TabId, Toast};
use super::tabs::placeholder::PlaceholderTab;
use super::widgets::{help, status_line, tab_bar, too_small};

/// How long a toast stays, in seconds (D-80).
const TOAST_SECONDS: u64 = 3;

pub struct App {
    info: AppInfo,
    clock: Box<dyn Clock>,
    tabs: Vec<Box<dyn Tab>>,
    active: usize,
    toast: Option<Toast>,
    help_open: bool,
}

impl App {
    /// The shipped app. Tabs are placeholders until T7.2 to T9.
    pub fn new(info: AppInfo, clock: Box<dyn Clock>) -> App {
        let tabs: Vec<Box<dyn Tab>> = vec![
            Box::new(PlaceholderTab::new(TabId::Oracle, "Oracle")),
            Box::new(PlaceholderTab::new(TabId::Library, "Library")),
            Box::new(PlaceholderTab::new(TabId::Dice, "Dice")),
            Box::new(PlaceholderTab::new(TabId::Journal, "Journal")),
        ];
        App::with_tabs(info, clock, tabs)
    }

    pub fn with_tabs(info: AppInfo, clock: Box<dyn Clock>, tabs: Vec<Box<dyn Tab>>) -> App {
        App {
            info,
            clock,
            tabs,
            active: 0,
            toast: None,
            help_open: false,
        }
    }

    pub fn active(&self) -> Option<TabId> {
        self.tabs.get(self.active).map(|t| t.id())
    }

    pub fn toast(&self) -> Option<&Toast> {
        self.toast.as_ref()
    }

    pub fn show_toast(&mut self, mut toast: Toast) {
        toast.expires_at = self.clock.now_secs() + TOAST_SECONDS;
        self.toast = Some(toast);
    }

    /// Expire the toast. Called by the event loop every 250 ms.
    pub fn tick(&mut self) {
        let now = self.clock.now_secs();
        if self.toast.as_ref().is_some_and(|t| now >= t.expires_at) {
            self.toast = None;
        }
    }

    /// Returns false when the app should quit.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // (1) Help open: any key closes it.
        if self.help_open {
            self.help_open = false;
            return true;
        }
        // (2) Tab always switches, even while editing (D-38).
        match key.code {
            KeyCode::Tab => return self.step_tab(1),
            KeyCode::BackTab => return self.step_tab(-1),
            _ => {}
        }
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        let capturing = self
            .tabs
            .get(self.active)
            .is_some_and(|t| t.captures_input());
        let key = if capturing {
            // (4) Ctrl-C cancels the edit; a second one quits (D-39).
            if ctrl_c {
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            } else {
                key
            }
        } else {
            // (3) Global keys when no text field has focus.
            match key.code {
                _ if ctrl_c => return false,
                KeyCode::Char('q') if plain => return false,
                KeyCode::Char('?') if plain => {
                    self.help_open = true;
                    return true;
                }
                KeyCode::Char(c @ '1'..='4') if plain => {
                    return self.switch_to_index(usize::from(c as u8 - b'1'));
                }
                _ => key,
            }
        };
        // (5) Everything else is the tab's.
        let action = match self.tabs.get_mut(self.active) {
            Some(tab) => tab.update(key),
            None => Action::None,
        };
        self.execute(action)
    }

    fn switch_to_index(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active = index;
        }
        true
    }

    fn step_tab(&mut self, delta: isize) -> bool {
        let n = self.tabs.len().max(1) as isize;
        self.active = (self.active as isize + delta).rem_euclid(n) as usize;
        true
    }

    /// Returns false when the app should quit.
    fn execute(&mut self, action: Action) -> bool {
        match action {
            Action::None => {}
            Action::Quit => return false,
            Action::SwitchTab(id) => {
                if let Some(i) = self.tabs.iter().position(|t| t.id() == id) {
                    self.active = i;
                }
            }
            Action::Toast(t) => self.show_toast(t),
            // Rolling, notes, export and Library jumps arrive with T7.2 to T9.
            Action::Roll(_)
            | Action::AppendNote(_)
            | Action::ExportJournal
            | Action::OpenInLibrary(_) => self.show_toast(Toast::info("not available yet")),
        }
        true
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        if too_small::is_too_small(area) {
            too_small::render(frame, area);
            return;
        }
        let [bar, body, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let titles: Vec<&str> = self.tabs.iter().map(|t| t.title()).collect();
        tab_bar::render(
            frame,
            bar,
            &titles,
            self.active,
            self.info.seed,
            self.info.warnings.len(),
        );
        let hints = self.tabs.get(self.active).map_or(&[][..], |t| t.help());
        if let Some(tab) = self.tabs.get(self.active) {
            tab.render(frame, body);
        }
        status_line::render(frame, status, self.toast.as_ref(), hints);
        if self.help_open {
            let title = self.tabs.get(self.active).map_or("", |t| t.title());
            help::render(frame, body, title, hints);
        }
    }
}

#[cfg(test)]
mod tests;
