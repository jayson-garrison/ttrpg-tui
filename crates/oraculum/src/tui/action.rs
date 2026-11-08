//! The vocabulary between tabs and the `App` (A§5.4, D-37, D-77).
//!
//! Tabs are pure state machines: they return an `Action` describing what
//! they want, and the `App` does it (rolls, journal, toasts).

use crossterm::event::KeyEvent;
use oraculum_core::catalog::{ItemKind, LoadWarning};
use oraculum_core::engine::Rolled;
use oraculum_core::generator::GeneratorResult;
use oraculum_core::journal::{Entry, SkippedLine};
use oraculum_core::oracle::Likelihood;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Order is the key order: `1` to `4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Oracle,
    Library,
    Dice,
    Journal,
}

impl TabId {
    pub const ALL: [TabId; 4] = [TabId::Oracle, TabId::Library, TabId::Dice, TabId::Journal];
}

pub trait Tab {
    fn id(&self) -> TabId;
    fn title(&self) -> &'static str;
    fn update(&mut self, key: KeyEvent) -> Action;
    fn render(&self, frame: &mut Frame, area: Rect);
    /// True while a text field has focus: then `q`, digits and `?` are text.
    fn captures_input(&self) -> bool;
    /// App to tab notifications.
    fn on_event(&mut self, ev: &TabEvent);
    /// `(keys, description)` pairs shown in the help overlay and status line.
    fn help(&self) -> &'static [(&'static str, &'static str)];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    SwitchTab(TabId),
    Quit,
    /// The App performs it with the Engine, journals it, and answers the tab.
    Roll(RollRequest),
    AppendNote(String),
    /// The App writes the Markdown export and toasts the path (D-77).
    ExportJournal,
    Toast(Toast),
    OpenInLibrary(LibraryTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollRequest {
    Oracle {
        likelihood: Likelihood,
        question: Option<String>,
    },
    Dice {
        expr: String,
    },
    Table {
        id: String,
    },
    Generator {
        id: String,
    },
    RerollSlot {
        previous: Box<GeneratorResult>,
        slot: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryTarget {
    Item { kind: ItemKind, id: String },
    Category(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabEvent {
    /// To the tab that emitted the roll; carries the journal entry too (D-77).
    Rolled(Box<Rolled>),
    /// An `EvalError`, message ready to show.
    RollFailed(String),
    JournalLoaded(Vec<Entry>, Vec<SkippedLine>),
    JournalAppended(Entry),
    JournalWriteFailed(String),
    Focus(LibraryTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub level: ToastLevel,
    /// Epoch seconds; the App stamps it when the toast is shown (D-80).
    pub expires_at: u64,
}

impl Toast {
    pub fn info(text: impl Into<String>) -> Toast {
        Toast {
            text: text.into(),
            level: ToastLevel::Info,
            expires_at: 0,
        }
    }

    pub fn error(text: impl Into<String>) -> Toast {
        Toast {
            text: text.into(),
            level: ToastLevel::Error,
            expires_at: 0,
        }
    }
}

/// What the App knows about this run (D-80).
pub struct AppInfo {
    pub seed: u64,
    pub session: String,
    pub warnings: Vec<LoadWarning>,
    pub data_dir: crate::storage::DataDir,
}
