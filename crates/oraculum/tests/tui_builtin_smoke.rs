//! Smoke tests over the shipped content (D-72): these only prove the built-in
//! catalog works through the TUI. They never assert on content text.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oraculum_app::cli::build_catalog;
use oraculum_app::clock::SystemClock;
use oraculum_app::storage::{DataDir, JournalStore};
use oraculum_app::tui::action::{AppInfo, ToastLevel};
use oraculum_app::tui::App;
use oraculum_core::engine::Engine;
use oraculum_core::rng::RngRoller;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn draw(app: &App, w: u16, h: u16) {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| app.render(f)).unwrap();
}

#[test]
fn builtin_smoke_every_item_rolls_from_the_library_and_every_tab_renders() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::resolve(Some(dir.path().to_path_buf())).unwrap();
    let (catalog, warnings) = build_catalog(&data_dir, &oraculum_core::builtin::sources());
    assert!(warnings.is_empty(), "{warnings:?}");
    let items = catalog.items().len();
    assert!(items > 0);
    let store_path =
        data_dir.session_path(&oraculum_app::storage::SessionName::parse("smoke").unwrap());
    let engine = Engine::new(
        Rc::new(catalog),
        Box::new(RngRoller::from_seed(1)),
        Box::new(SystemClock),
    );
    let info = AppInfo {
        seed: 1,
        session: "smoke".into(),
        warnings,
        data_dir,
    };
    let mut app = App::new(
        info,
        engine,
        JournalStore::new(store_path.clone()),
        Box::new(SystemClock),
    );
    app.start();

    // Oracle: ask until a twist could have happened is not guaranteed; just ask.
    app.handle_key(key(KeyCode::Enter));

    // Library: roll every item, top to bottom.
    app.handle_key(key(KeyCode::Char('2')));
    app.handle_key(key(KeyCode::Home));
    for _ in 0..items {
        app.handle_key(key(KeyCode::Enter));
        draw(&app, 80, 24);
        app.handle_key(key(KeyCode::Down));
    }
    assert!(
        app.toast().is_none_or(|t| t.level != ToastLevel::Error),
        "{:?}",
        app.toast()
    );
    let lines = std::fs::read_to_string(&store_path)
        .unwrap()
        .lines()
        .count();
    assert_eq!(lines, items + 1, "one oracle ask plus one roll per item");

    for tab in ['1', '2', '3', '4'] {
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char(tab)));
        for (w, h) in [(60, 20), (80, 24), (200, 60)] {
            draw(&app, w, h);
        }
    }
}
