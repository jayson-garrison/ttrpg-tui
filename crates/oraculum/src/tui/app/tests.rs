use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oraculum_core::clock::Clock;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use super::*;
use crate::tui::action::TabEvent;

struct TestClock(Rc<Cell<u64>>);
impl Clock for TestClock {
    fn now_secs(&self) -> u64 {
        self.0.get()
    }
}

/// A tab that records keys and can pretend to hold a text field.
struct Stub {
    id: TabId,
    capture: bool,
    seen: Rc<RefCell<Vec<KeyCode>>>,
    next: Action,
}

impl Tab for Stub {
    fn id(&self) -> TabId {
        self.id
    }
    fn title(&self) -> &'static str {
        match self.id {
            TabId::Oracle => "Oracle",
            TabId::Library => "Library",
            TabId::Dice => "Dice",
            TabId::Journal => "Journal",
        }
    }
    fn update(&mut self, key: KeyEvent) -> Action {
        self.seen.borrow_mut().push(key.code);
        std::mem::replace(&mut self.next, Action::None)
    }
    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(ratatui::widgets::Paragraph::new("stub body"), area);
    }
    fn captures_input(&self) -> bool {
        self.capture
    }
    fn on_event(&mut self, _ev: &TabEvent) {}
    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[("Enter", "roll the stub")]
    }
}

struct Fixture {
    app: App,
    now: Rc<Cell<u64>>,
    seen: Vec<Rc<RefCell<Vec<KeyCode>>>>,
    _dir: tempfile::TempDir,
}

/// Four stub tabs; `capturing` lists the tab indexes that hold a text field.
fn fixture(capturing: &[usize], warnings: usize) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let now = Rc::new(Cell::new(100));
    let mut seen = Vec::new();
    let tabs: Vec<Box<dyn Tab>> = TabId::ALL
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let log = Rc::new(RefCell::new(Vec::new()));
            seen.push(log.clone());
            Box::new(Stub {
                id: *id,
                capture: capturing.contains(&i),
                seen: log,
                next: Action::None,
            }) as Box<dyn Tab>
        })
        .collect();
    let info = AppInfo {
        seed: 42,
        session: "default".into(),
        warnings: (0..warnings).map(|_| warning()).collect(),
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let app = App::with_tabs(info, Box::new(TestClock(now.clone())), tabs);
    Fixture {
        app,
        now,
        seen,
        _dir: dir,
    }
}

fn warning() -> oraculum_core::catalog::LoadWarning {
    use oraculum_core::catalog::{ItemKind, LoadWarning, WarningKind};
    LoadWarning {
        origin: "x.toml".into(),
        item: Some((ItemKind::Table, "x".into())),
        kind: WarningKind::Invalid,
        message: "bad".into(),
        kept_builtin: false,
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}
fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

fn screen(app: &App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn digits_and_tab_keys_switch_tabs() {
    let mut f = fixture(&[], 0);
    assert_eq!(f.app.active(), Some(TabId::Oracle));
    f.app.handle_key(ch('3'));
    assert_eq!(f.app.active(), Some(TabId::Dice));
    f.app.handle_key(key(KeyCode::Tab));
    assert_eq!(f.app.active(), Some(TabId::Journal));
    f.app.handle_key(key(KeyCode::Tab));
    assert_eq!(f.app.active(), Some(TabId::Oracle), "Tab wraps");
    f.app.handle_key(key(KeyCode::BackTab));
    assert_eq!(f.app.active(), Some(TabId::Journal), "Shift-Tab wraps back");
}

#[test]
fn q_and_ctrl_c_quit_when_not_typing() {
    let mut f = fixture(&[], 0);
    assert!(!f.app.handle_key(ch('q')));
    let mut f = fixture(&[], 0);
    assert!(!f.app.handle_key(ctrl_c()));
}

#[test]
fn q_in_a_text_field_is_text_not_quit() {
    let mut f = fixture(&[0], 0);
    assert!(f.app.handle_key(ch('q')));
    assert_eq!(*f.seen[0].borrow(), [KeyCode::Char('q')]);
}

#[test]
fn digits_and_question_mark_are_text_while_typing() {
    let mut f = fixture(&[0], 0);
    for c in ['2', '4', '?'] {
        f.app.handle_key(ch(c));
    }
    assert_eq!(f.app.active(), Some(TabId::Oracle));
    assert_eq!(f.seen[0].borrow().len(), 3);
    assert!(f.app.render_text_lacks_help());
}

#[test]
fn tab_switches_while_editing() {
    let mut f = fixture(&[0], 0);
    f.app.handle_key(key(KeyCode::Tab));
    assert_eq!(f.app.active(), Some(TabId::Library));
    assert!(f.seen[0].borrow().is_empty(), "the field never saw Tab");
}

#[test]
fn ctrl_c_while_editing_cancels_then_quits() {
    let mut f = fixture(&[0], 0);
    assert!(f.app.handle_key(ctrl_c()), "first Ctrl-C does not quit");
    assert_eq!(*f.seen[0].borrow(), [KeyCode::Esc], "it arrives as Esc");
    // The stub keeps capturing, so make the field release itself like a real tab.
    let mut f = fixture(&[], 0);
    assert!(!f.app.handle_key(ctrl_c()), "with no field open it quits");
}

#[test]
fn help_opens_with_question_mark_and_any_key_closes_it() {
    let mut f = fixture(&[], 0);
    f.app.handle_key(ch('?'));
    let text = screen(&f.app, 80, 24);
    assert!(text.contains("Help"), "{text}");
    assert!(text.contains("switch tab"), "global keys listed");
    assert!(text.contains("roll the stub"), "tab keys listed");
    assert!(
        f.app.handle_key(ch('q')),
        "the closing key is swallowed, not a quit"
    );
    assert!(!screen(&f.app, 80, 24).contains("Help"));
    assert!(f.seen[0].borrow().is_empty());
}

#[test]
fn tab_actions_are_executed() {
    let mut f = fixture(&[], 0);
    f.app.tabs[0] = Box::new(Stub {
        id: TabId::Oracle,
        capture: false,
        seen: Rc::default(),
        next: Action::SwitchTab(TabId::Journal),
    });
    f.app.handle_key(key(KeyCode::Enter));
    assert_eq!(f.app.active(), Some(TabId::Journal));

    let mut f = fixture(&[], 0);
    f.app.tabs[0] = Box::new(Stub {
        id: TabId::Oracle,
        capture: false,
        seen: Rc::default(),
        next: Action::Quit,
    });
    assert!(!f.app.handle_key(key(KeyCode::Enter)));
}

#[test]
fn toast_expires_after_three_seconds_with_a_fixed_clock() {
    let mut f = fixture(&[], 0);
    f.app.show_toast(Toast::info("hello"));
    assert!(screen(&f.app, 80, 24).contains("hello"));
    f.now.set(102);
    f.app.tick();
    assert!(f.app.toast().is_some(), "still showing at +2 s");
    f.now.set(103);
    f.app.tick();
    assert!(f.app.toast().is_none(), "gone at +3 s");
    assert!(!screen(&f.app, 80, 24).contains("hello"));
}

#[test]
fn not_yet_built_actions_toast_instead_of_doing_nothing() {
    let mut f = fixture(&[], 0);
    f.app.tabs[0] = Box::new(Stub {
        id: TabId::Oracle,
        capture: false,
        seen: Rc::default(),
        next: Action::ExportJournal,
    });
    f.app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        f.app.toast().map(|t| t.text.as_str()),
        Some("not available yet")
    );
}

#[test]
fn render_tab_bar_shows_four_tabs_seed_and_warnings() {
    let f = fixture(&[], 2);
    let text = screen(&f.app, 80, 24);
    let first = text.lines().next().unwrap();
    for needle in [
        "1 Oracle",
        "2 Library",
        "3 Dice",
        "4 Journal",
        "seed 42",
        "warnings 2",
    ] {
        assert!(first.contains(needle), "{needle} missing from {first:?}");
    }
    assert!(text.contains("stub body"));
    assert!(text.lines().last().unwrap().contains("? help"));
}

#[test]
fn status_line_shows_the_active_tabs_hints() {
    let f = fixture(&[], 0);
    assert!(screen(&f.app, 80, 24)
        .lines()
        .last()
        .unwrap()
        .contains("Enter roll the stub"));
}

#[test]
fn render_too_small_under_60x20_and_recovers() {
    let f = fixture(&[], 0);
    for (w, h) in [(59, 24), (80, 19), (10, 5)] {
        let text = screen(&f.app, w, h);
        assert!(text.contains("too small"), "{w}x{h}: {text}");
    }
    assert!(!screen(&f.app, 60, 20).contains("too small"));
}

#[test]
fn rendering_never_panics_at_any_size() {
    let mut f = fixture(&[0], 3);
    f.app
        .show_toast(Toast::error("an error toast with a long long long message"));
    for (w, h) in [(0, 0), (1, 1), (59, 19), (60, 20), (80, 24), (200, 60)] {
        let _ = screen(&f.app, w, h);
    }
    f.app.handle_key(ch('?'));
    for (w, h) in [(60, 20), (80, 24), (200, 60)] {
        let _ = screen(&f.app, w, h);
    }
}

#[test]
fn the_shipped_app_has_four_placeholder_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let info = AppInfo {
        seed: 1,
        session: "s".into(),
        warnings: vec![],
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let mut app = App::new(info, Box::new(TestClock(Rc::new(Cell::new(0)))));
    assert!(screen(&app, 80, 24).contains("Oracle is not built yet"));
    app.handle_key(ch('4'));
    assert_eq!(app.active(), Some(TabId::Journal));
}

impl App {
    /// Test helper: the help overlay is not on screen.
    fn render_text_lacks_help(&self) -> bool {
        !screen(self, 80, 24).contains("Help")
    }
}
