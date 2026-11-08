//! The `App`: owns the tabs, applies A§5.4's key order, executes `Action`s,
//! and draws the frame. Pure: no terminal, so tests drive it directly (D-80).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oraculum_core::clock::Clock;
use oraculum_core::engine::{Engine, Rolled};
use oraculum_core::journal::{export_markdown, Entry};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

use super::action::{Action, AppInfo, RollRequest, Tab, TabEvent, TabId, Toast};
use super::tabs::dice::DiceTab;
use super::tabs::journal::JournalTab;
use super::tabs::library::LibraryTab;
use super::tabs::oracle::OracleTab;
use super::widgets::{help, status_line, tab_bar, too_small};
use crate::storage::{write_export, JournalStore};

/// How long a toast stays, in seconds (D-80).
const TOAST_SECONDS: u64 = 3;

pub struct App {
    info: AppInfo,
    engine: Engine,
    store: JournalStore,
    clock: Box<dyn Clock>,
    tabs: Vec<Box<dyn Tab>>,
    active: usize,
    toast: Option<Toast>,
    help_open: bool,
}

impl App {
    /// The shipped app: Oracle, Library, Dice, Journal.
    pub fn new(info: AppInfo, engine: Engine, store: JournalStore, clock: Box<dyn Clock>) -> App {
        let tabs: Vec<Box<dyn Tab>> = vec![
            Box::new(OracleTab::new()),
            Box::new(LibraryTab::new(
                engine.catalog().clone(),
                info.warnings.clone(),
            )),
            Box::new(DiceTab::new()),
            Box::new(JournalTab::new()),
        ];
        App::with_tabs(info, engine, store, clock, tabs)
    }

    pub fn with_tabs(
        info: AppInfo,
        engine: Engine,
        store: JournalStore,
        clock: Box<dyn Clock>,
        tabs: Vec<Box<dyn Tab>>,
    ) -> App {
        App {
            info,
            engine,
            store,
            clock,
            tabs,
            active: 0,
            toast: None,
            help_open: false,
        }
    }

    /// Load the session journal into the Journal tab and report damage (D-81).
    pub fn start(&mut self) {
        match self.store.load() {
            Ok(load) => {
                let skipped = load.skipped.len();
                self.send(
                    TabId::Journal,
                    &TabEvent::JournalLoaded(load.entries, load.skipped),
                );
                if skipped > 0 {
                    self.show_toast(Toast::error(format!(
                        "{skipped} malformed journal lines skipped"
                    )));
                }
            }
            Err(e) => self.show_toast(Toast::error(format!("cannot read journal: {e}"))),
        }
    }

    fn send(&mut self, to: TabId, ev: &TabEvent) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id() == to) {
            tab.on_event(ev);
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
            Action::Roll(request) => self.roll(request),
            Action::AppendNote(text) => {
                let entry = self.engine.note(&text);
                self.persist(&entry);
                self.send(TabId::Journal, &TabEvent::JournalAppended(entry));
            }
            Action::ExportJournal => self.export(),
            Action::OpenInLibrary(target) => {
                self.execute(Action::SwitchTab(TabId::Library));
                self.send(TabId::Library, &TabEvent::Focus(target));
            }
        }
        true
    }

    /// Run a roll request, journal it, and tell the tab that asked and the
    /// Journal tab. A write failure keeps the entry on screen (A§5.4).
    fn roll(&mut self, request: RollRequest) {
        let origin = self.tabs.get(self.active).map(|t| t.id());
        let outcome: Result<Rolled, String> = match request {
            RollRequest::Dice { expr } => self
                .engine
                .roll_dice(&expr)
                .map_err(|e| format!("bad dice expression: {e}")),
            RollRequest::Oracle {
                likelihood,
                question,
            } => Ok(self.engine.ask(likelihood, question.as_deref())),
            RollRequest::Table { id } => self.engine.roll_table(&id).map_err(|e| e.to_string()),
            RollRequest::Generator { id } => self.engine.generate(&id).map_err(|e| e.to_string()),
            RollRequest::RerollSlot { previous, slot } => self
                .engine
                .reroll_slot(&previous, slot)
                .map_err(|e| e.to_string()),
        };
        let Some(origin) = origin else { return };
        match outcome {
            Err(msg) => self.send(origin, &TabEvent::RollFailed(msg)),
            Ok(rolled) => {
                let entry = rolled.entry.clone();
                self.persist(&entry);
                self.send(origin, &TabEvent::Rolled(Box::new(rolled)));
                self.send(TabId::Journal, &TabEvent::JournalAppended(entry));
            }
        }
    }

    /// Append to the session file. A failure toasts and tells the Journal tab;
    /// the caller still shows the entry (A§5.4).
    fn persist(&mut self, entry: &Entry) {
        if let Err(e) = self.store.append(entry) {
            let msg = format!("journal write failed: {e}");
            self.show_toast(Toast::error(msg.clone()));
            self.send(TabId::Journal, &TabEvent::JournalWriteFailed(msg));
        }
    }

    /// Same file and text as `oraculum export` (D-77).
    fn export(&mut self) {
        let load = match self.store.load() {
            Ok(load) => load,
            Err(e) => return self.show_toast(Toast::error(format!("cannot read journal: {e}"))),
        };
        let path = self.info.data_dir.export_path();
        let markdown = export_markdown(&self.info.session, &load.entries);
        match write_export(&path, &markdown) {
            Ok(()) => self.show_toast(Toast::info(format!("exported to {}", path.display()))),
            Err(e) => self.show_toast(Toast::error(format!("export failed: {e}"))),
        }
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
