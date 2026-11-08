//! Integration tests: helper functions here may unwrap (clippy only exempts `#[test]` bodies).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use oraculum_app::storage::{
    load_user_sources, write_export, DataDir, JournalStore, SessionName, StorageError,
};
use oraculum_core::catalog::{ItemKind, WarningKind};
use oraculum_core::journal::{Entry, Kind};
use tempfile::tempdir;

fn note(ts: u64, text: &str) -> Entry {
    Entry::new(ts, Kind::Note, "", text, serde_json::Value::Null)
}

#[test]
fn session_names_are_validated() {
    for ok in ["default", "my-game_2", "A", &"a".repeat(64)] {
        assert!(SessionName::parse(ok).is_ok(), "{ok}");
    }
    for bad in ["", "../x", "a/b", "a b", "a.b", "é", &"a".repeat(65), ".."] {
        assert_eq!(
            SessionName::parse(bad).unwrap_err(),
            StorageError::InvalidSessionName(bad.to_string()),
            "{bad}"
        );
    }
    assert_eq!(SessionName::parse("run1").unwrap().as_str(), "run1");
}

#[test]
fn data_dir_layout() {
    let dir = tempdir().unwrap();
    let d = DataDir::resolve(Some(dir.path().to_path_buf())).unwrap();
    let s = SessionName::parse("s1").unwrap();
    assert_eq!(d.tables_dir(), dir.path().join("tables"));
    assert_eq!(d.generators_dir(), dir.path().join("generators"));
    assert_eq!(
        d.session_path(&s),
        dir.path().join("sessions").join("s1.jsonl")
    );
    assert_eq!(d.export_path(), dir.path().join("export.md"));
}

#[test]
fn nothing_is_created_by_reading() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("fresh");
    let d = DataDir::resolve(Some(root.clone())).unwrap();
    let store = JournalStore::new(d.session_path(&SessionName::parse("default").unwrap()));
    let load = store.load().unwrap();
    assert!(load.entries.is_empty() && load.skipped.is_empty());
    let (sources, warnings) = load_user_sources(&d);
    assert!(sources.is_empty() && warnings.is_empty());
    assert!(!root.exists(), "reads must not create the data directory");
}

#[test]
fn append_creates_directories_and_round_trips() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deep").join("sessions").join("s.jsonl");
    let store = JournalStore::new(path.clone());
    store.append(&note(1, "one")).unwrap();
    store.append(&note(2, "two")).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(text.ends_with('\n'));
    let load = store.load().unwrap();
    assert_eq!(load.entries, [note(1, "one"), note(2, "two")]);
}

#[test]
fn a_crash_truncated_last_line_does_not_swallow_the_next_entry() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let store = JournalStore::new(path.clone());
    store.append(&note(1, "kept")).unwrap();
    // Simulate a crash mid-write: a partial line with no newline.
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"v\":1,\"ts\":2,\"ki");
    fs::write(&path, text).unwrap();

    store.append(&note(3, "after")).unwrap();
    let load = store.load().unwrap();
    assert_eq!(load.entries, [note(1, "kept"), note(3, "after")]);
    assert_eq!(load.skipped.len(), 1);
    assert_eq!(load.skipped[0].line_no, 2);
}

#[test]
fn a_non_utf8_journal_loads_what_it_can() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let store = JournalStore::new(path.clone());
    store.append(&note(1, "ok")).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"\xff\xfe broken\n");
    fs::write(&path, bytes).unwrap();
    let load = store.load().unwrap();
    assert_eq!(load.entries.len(), 1);
    assert_eq!(load.skipped.len(), 1);
}

#[test]
fn append_reports_io_errors() {
    let dir = tempdir().unwrap();
    // A file where a directory is needed.
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let store = JournalStore::new(blocker.join("sessions").join("s.jsonl"));
    assert!(matches!(
        store.append(&note(1, "x")),
        Err(StorageError::Io { .. })
    ));
}

fn write(dir: &std::path::Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

#[test]
fn user_sources_are_sorted_filtered_and_tagged() {
    let dir = tempdir().unwrap();
    let r = dir.path();
    write(r, "tables/b.toml", b"name = \"B\"");
    write(r, "tables/a.toml", b"name = \"A\"");
    write(r, "tables/UPPER.TOML", b"name = \"U\"");
    write(r, "tables/.hidden.toml", b"x");
    write(r, "tables/backup.toml~", b"x");
    write(r, "tables/notes.txt", b"x");
    write(r, "tables/sub/inner.toml", b"x");
    write(r, "generators/g.toml", b"name = \"G\"");
    let d = DataDir::resolve(Some(r.to_path_buf())).unwrap();

    let (sources, warnings) = load_user_sources(&d);
    assert!(warnings.is_empty(), "{warnings:?}");
    let got: Vec<(ItemKind, &str, &str)> = sources
        .iter()
        .map(|s| (s.kind, s.origin.as_str(), s.file_stem.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (ItemKind::Table, "tables/UPPER.TOML", "UPPER"),
            (ItemKind::Table, "tables/a.toml", "a"),
            (ItemKind::Table, "tables/b.toml", "b"),
            (ItemKind::Generator, "generators/g.toml", "g"),
        ]
    );
    assert_eq!(sources[1].text, "name = \"A\"");
}

#[test]
fn unreadable_and_oversized_files_become_warnings() {
    let dir = tempdir().unwrap();
    let r = dir.path();
    write(r, "tables/ok.toml", b"name = \"ok\"");
    write(r, "tables/binary.toml", b"\xff\xfe\x00");
    write(r, "tables/huge.toml", &vec![b'x'; 1024 * 1024 + 1]);
    let d = DataDir::resolve(Some(r.to_path_buf())).unwrap();

    let (sources, warnings) = load_user_sources(&d);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].file_stem, "ok");
    let kinds: Vec<(&str, WarningKind)> = warnings
        .iter()
        .map(|w| (w.origin.as_str(), w.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("tables/binary.toml", WarningKind::FileUnreadable),
            ("tables/huge.toml", WarningKind::FileTooLarge),
        ]
    );
}

#[test]
fn exactly_one_mebibyte_is_accepted() {
    let dir = tempdir().unwrap();
    write(dir.path(), "tables/edge.toml", &vec![b' '; 1024 * 1024]);
    let d = DataDir::resolve(Some(dir.path().to_path_buf())).unwrap();
    let (sources, warnings) = load_user_sources(&d);
    assert_eq!((sources.len(), warnings.len()), (1, 0));
}

#[test]
fn export_is_written_atomically_and_overwrites() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("out").join("export.md");
    write_export(&path, "first\n").unwrap();
    write_export(&path, "second\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");
    let names: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["export.md"], "no temp file may be left behind");
}
