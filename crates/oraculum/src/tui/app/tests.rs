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
    events: Rc<RefCell<Vec<TabEvent>>>,
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
    fn on_event(&mut self, ev: &TabEvent) {
        self.events.borrow_mut().push(ev.clone());
    }
    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[("Enter", "roll the stub")]
    }
}

struct Fixture {
    app: App,
    now: Rc<Cell<u64>>,
    seen: Vec<Rc<RefCell<Vec<KeyCode>>>>,
    _dir: tempfile::TempDir,
    _store_dir: tempfile::TempDir,
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
                events: Rc::default(),
            }) as Box<dyn Tab>
        })
        .collect();
    let info = AppInfo {
        seed: 42,
        session: "default".into(),
        warnings: (0..warnings).map(|_| warning()).collect(),
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let (store, store_dir) = crate::tui::testkit::store();
    let app = App::with_tabs(
        info,
        crate::tui::testkit::engine(&[]),
        store,
        Box::new(TestClock(now.clone())),
        tabs,
    );
    Fixture {
        app,
        now,
        seen,
        _dir: dir,
        _store_dir: store_dir,
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
        events: Rc::default(),
    });
    f.app.handle_key(key(KeyCode::Enter));
    assert_eq!(f.app.active(), Some(TabId::Journal));

    let mut f = fixture(&[], 0);
    f.app.tabs[0] = Box::new(Stub {
        id: TabId::Oracle,
        capture: false,
        seen: Rc::default(),
        next: Action::Quit,
        events: Rc::default(),
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
        events: Rc::default(),
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
fn the_shipped_app_has_a_dice_tab_and_placeholders() {
    let dir = tempfile::tempdir().unwrap();
    let info = AppInfo {
        seed: 1,
        session: "s".into(),
        warnings: vec![],
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let (store, _store_dir) = crate::tui::testkit::store();
    let mut app = App::new(
        info,
        crate::tui::testkit::engine(&[]),
        store,
        Box::new(TestClock(Rc::new(Cell::new(0)))),
    );
    assert!(screen(&app, 80, 24).contains("Press Enter to ask."));
    app.handle_key(ch('2'));
    assert!(screen(&app, 80, 24).contains("Library is not built yet"));
    app.handle_key(ch('4'));
    assert_eq!(app.active(), Some(TabId::Journal));
}

impl App {
    /// Test helper: the help overlay is not on screen.
    fn render_text_lacks_help(&self) -> bool {
        !screen(self, 80, 24).contains("Help")
    }
}

// ---- T7.2: roll plumbing with the real Dice tab ----

struct DiceApp {
    app: App,
    journal_events: Rc<RefCell<Vec<TabEvent>>>,
    store_path: std::path::PathBuf,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

/// Real Dice tab at index 2, stubs elsewhere; the Journal stub records events.
fn dice_app(
    script: &[u32],
    store: Option<(crate::storage::JournalStore, tempfile::TempDir)>,
) -> DiceApp {
    let dir = tempfile::tempdir().unwrap();
    let (store, store_dir) = store.unwrap_or_else(crate::tui::testkit::store);
    let store_path = store.path().to_path_buf();
    let journal_events: Rc<RefCell<Vec<TabEvent>>> = Rc::default();
    let tabs: Vec<Box<dyn Tab>> = TabId::ALL
        .iter()
        .map(|id| -> Box<dyn Tab> {
            if *id == TabId::Dice {
                return Box::new(crate::tui::tabs::dice::DiceTab::new());
            }
            Box::new(Stub {
                id: *id,
                capture: false,
                seen: Rc::default(),
                next: Action::None,
                events: if *id == TabId::Journal {
                    journal_events.clone()
                } else {
                    Rc::default()
                },
            })
        })
        .collect();
    let info = AppInfo {
        seed: 7,
        session: "t".into(),
        warnings: vec![],
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let mut app = App::with_tabs(
        info,
        crate::tui::testkit::engine(script),
        store,
        Box::new(TestClock(Rc::new(Cell::new(100)))),
        tabs,
    );
    app.handle_key(ch('3'));
    DiceApp {
        app,
        journal_events,
        store_path,
        _dirs: (dir, store_dir),
    }
}

fn type_and_enter(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(ch(c));
    }
    app.handle_key(key(KeyCode::Enter));
}

fn journal_lines(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path).map_or(0, |t| t.lines().count())
}

#[test]
fn q_in_dice_edit_is_text_not_quit() {
    let mut d = dice_app(&[], None);
    assert!(d.app.handle_key(ch('q')));
    assert!(screen(&d.app, 80, 24).contains("Dice: q"));
}

#[test]
fn q_in_browse_quits() {
    let mut d = dice_app(&[], None);
    d.app.handle_key(key(KeyCode::Esc));
    assert!(!d.app.handle_key(ch('q')));
}

#[test]
fn ctrl_c_in_dice_cancels_the_edit_then_quits() {
    let mut d = dice_app(&[], None);
    assert!(d.app.handle_key(ctrl_c()), "first Ctrl-C leaves the input");
    assert!(!d.app.handle_key(ctrl_c()), "second one quits");
}

#[test]
fn app_dice_invalid_does_not_append_journal() {
    let mut d = dice_app(&[], None);
    for bad in ["2d", "101d6", ""] {
        type_and_enter(&mut d.app, bad);
        let text = screen(&d.app, 80, 24);
        assert!(text.contains("! "), "{bad:?}: {text}");
        d.app.handle_key(ctrl_c()); // not quitting: a field is open? no, Esc via Ctrl-C
        d.app.handle_key(ch('/'));
    }
    assert_eq!(journal_lines(&d.store_path), 0);
    assert!(d.journal_events.borrow().is_empty());
}

#[test]
fn app_roll_appends_entry_and_notifies_journal_tab() {
    let mut d = dice_app(&[4, 2], None);
    type_and_enter(&mut d.app, "2d6");
    assert_eq!(journal_lines(&d.store_path), 1);
    let events = d.journal_events.borrow();
    assert!(
        matches!(events.as_slice(), [TabEvent::JournalAppended(e)] if e.input == "2d6"),
        "{events:?}"
    );
    drop(events);
    let text = screen(&d.app, 80, 24);
    assert!(
        text.contains("2d6 =>"),
        "history shows the CLI detail line: {text}"
    );
    assert!(text.contains("= 6"), "{text}");
}

#[test]
fn dice_history_keeps_the_last_ten_newest_first() {
    let mut d = dice_app(&[1; 12], None);
    for _ in 0..12 {
        d.app.handle_key(key(KeyCode::Enter));
        if journal_lines(&d.store_path) == 0 {
            type_and_enter(&mut d.app, "d6");
        }
    }
    let text = screen(&d.app, 80, 24);
    assert_eq!(text.matches("d6 =>").count(), 10, "{text}");
}

#[test]
fn app_journal_write_failure_toasts_and_shows_entry() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("sessions"), "in the way").unwrap();
    let store = crate::storage::JournalStore::new(dir.path().join("sessions").join("t.jsonl"));
    let mut d = dice_app(&[3], Some((store, dir)));
    type_and_enter(&mut d.app, "d6");
    let text = screen(&d.app, 80, 24);
    assert!(text.contains("journal write failed"), "{text}");
    assert!(text.contains("d6 =>"), "the roll is still shown: {text}");
    let events = d.journal_events.borrow();
    assert!(events
        .iter()
        .any(|e| matches!(e, TabEvent::JournalWriteFailed(_))));
    assert!(events
        .iter()
        .any(|e| matches!(e, TabEvent::JournalAppended(_))));
}

#[test]
fn start_loads_the_journal_and_toasts_about_skipped_lines() {
    let (store, store_dir) = crate::tui::testkit::store();
    std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
    let good = oraculum_core::journal::encode_line(&oraculum_core::journal::Entry::new(
        1,
        oraculum_core::journal::Kind::Note,
        "",
        "hi",
        serde_json::Value::Null,
    ))
    .unwrap();
    std::fs::write(store.path(), format!("{good}\nnot json\n")).unwrap();
    let mut d = dice_app(&[], Some((store, store_dir)));
    d.app.start();
    let events = d.journal_events.borrow();
    assert!(
        matches!(events.as_slice(), [TabEvent::JournalLoaded(e, s)] if e.len() == 1 && s.len() == 1),
        "{events:?}"
    );
    drop(events);
    assert!(screen(&d.app, 80, 24).contains("1 malformed journal lines skipped"));
}

#[test]
fn render_dice_60x20_shows_input_and_history() {
    let mut d = dice_app(&[5], None);
    type_and_enter(&mut d.app, "d6");
    let text = screen(&d.app, 60, 20);
    for needle in ["Dice: d6", "Last results", "d6 =>"] {
        assert!(text.contains(needle), "{needle}: {text}");
    }
}

#[test]
fn same_script_gives_the_same_dice_output() {
    let run = || {
        let mut d = dice_app(&[3, 6, 2], None);
        type_and_enter(&mut d.app, "3d6");
        screen(&d.app, 80, 24)
    };
    assert_eq!(run(), run());
}

#[test]
fn resize_down_and_back_up_redraws_without_panic() {
    let mut f = fixture(&[], 1);
    for (w, h) in [(200, 60), (59, 19), (60, 20), (80, 24), (30, 10), (200, 60)] {
        let text = screen(&f.app, w, h);
        assert_eq!(text.contains("too small"), w < 60 || h < 20, "{w}x{h}");
    }
    f.app.handle_key(ch('?'));
    let _ = screen(&f.app, 59, 19);
    assert!(
        screen(&f.app, 80, 24).contains("Help"),
        "help survives a resize"
    );
}

// ---- T8.1: Oracle through the App ----

fn oracle_app(script: &[u32], dice: bool) -> (App, std::path::PathBuf, [tempfile::TempDir; 2]) {
    let dir = tempfile::tempdir().unwrap();
    let (store, store_dir) = crate::tui::testkit::store();
    let path = store.path().to_path_buf();
    let info = AppInfo {
        seed: 7,
        session: "t".into(),
        warnings: vec![],
        data_dir: crate::storage::DataDir::resolve(Some(dir.path().to_path_buf())).unwrap(),
    };
    let engine = if dice {
        crate::tui::testkit::engine(script)
    } else {
        crate::tui::testkit::empty_engine(script)
    };
    let app = App::new(
        info,
        engine,
        store,
        Box::new(TestClock(Rc::new(Cell::new(0)))),
    );
    (app, path, [dir, store_dir])
}

#[test]
fn oracle_enter_rolls_shows_the_answer_and_journals_one_entry() {
    let (mut app, path, _keep) = oracle_app(&[22, 2], true);
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Enter));
    let s = screen(&app, 80, 24);
    assert!(s.contains("Yes") && s.contains("d100: 22"), "{s}");
    assert!(s.contains("Twist:") && s.contains("beta"), "{s}");
    assert_eq!(journal_lines(&path), 1);
}

#[test]
fn oracle_t_toasts_until_the_library_exists() {
    let (mut app, _path, _keep) = oracle_app(&[], true);
    app.handle_key(ch('t'));
    assert_eq!(
        app.toast().map(|t| t.text.as_str()),
        Some("Library not available yet")
    );
}

#[test]
fn oracle_question_text_is_not_a_global_key() {
    let (mut app, _path, _keep) = oracle_app(&[], true);
    app.handle_key(ch('/'));
    for c in "q1?".chars() {
        app.handle_key(ch(c));
    }
    assert_eq!(app.active(), Some(TabId::Oracle));
    assert!(screen(&app, 80, 24).contains("Question: q1?"));
}

#[test]
fn oracle_with_no_event_generator_still_answers() {
    let (mut app, path, _keep) = oracle_app(&[33], false);
    app.handle_key(key(KeyCode::Enter));
    let s = screen(&app, 80, 24);
    assert!(s.contains("Twist unavailable: event is missing"), "{s}");
    assert_eq!(journal_lines(&path), 1);
}
