//! The real `oraculum` binary: exit codes, stdout and stderr (A§5.1).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

/// Run to completion and return (exit code, stdout, stderr).
fn finish(mut c: Command) -> (i32, String, String) {
    let out = c.output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn with_args(dir: &std::path::Path, args: &[&str]) -> Command {
    let mut c = oraculum(dir);
    c.args(args);
    c
}

fn oraculum(dir: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("oraculum").unwrap();
    c.arg("--data-dir").arg(dir);
    c
}

#[test]
fn seeded_roll_is_identical_across_processes() {
    let dir = tempdir().unwrap();
    let run = || {
        let out = oraculum(dir.path())
            .args(["--seed", "7", "roll", "3d20kl1+2"])
            .assert()
            .code(0)
            .get_output()
            .stdout
            .clone();
        String::from_utf8(out).unwrap()
    };
    let first = run();
    assert_eq!(first, "3d20kl1+2 => [11, 17, 2] keep [2] +2 = 4\n");
    assert_eq!(first, run());
    let journal = fs::read_to_string(dir.path().join("sessions/default.jsonl")).unwrap();
    assert_eq!(journal.lines().count(), 2);
}

#[test]
fn unseeded_runs_still_work() {
    let dir = tempdir().unwrap();
    oraculum(dir.path()).args(["roll", "d6"]).assert().code(0);
}

#[test]
fn invalid_input_exits_2_with_a_plain_message() {
    let dir = tempdir().unwrap();
    let (code, out, err) = finish(with_args(dir.path(), &["roll", "2d6x"]));
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.starts_with("oraculum: bad dice expression"), "{err}");

    let (code, _, err) = finish(with_args(dir.path(), &["table", "nope"]));
    assert_eq!(code, 2);
    assert!(
        err.starts_with("oraculum: no table with the id \"nope\""),
        "{err}"
    );
}

#[test]
fn path_traversal_in_the_session_name_exits_2() {
    let dir = tempdir().unwrap();
    let (code, _, err) = finish(with_args(dir.path(), &["--session", "../x", "roll", "d6"]));
    assert_eq!(code, 2);
    assert!(err.contains("invalid session name"), "{err}");
    assert!(!dir.path().join("sessions").exists());
}

#[test]
fn usage_errors_exit_2() {
    let dir = tempdir().unwrap();
    oraculum(dir.path()).arg("frobnicate").assert().code(2);
    oraculum(dir.path()).args(["ask", "maybe"]).assert().code(2);
    oraculum(dir.path()).args(["roll"]).assert().code(2);
}

#[test]
fn help_and_version_succeed() {
    let dir = tempdir().unwrap();
    oraculum(dir.path()).arg("--help").assert().code(0);
    oraculum(dir.path()).arg("--version").assert().code(0);
}

#[test]
fn running_without_a_subcommand_explains_and_exits_2() {
    let dir = tempdir().unwrap();
    let (code, _, err) = finish(with_args(dir.path(), &[]));
    assert_eq!(code, 2);
    assert!(err.contains("interactive interface"), "{err}");
}

#[test]
fn an_unwritable_journal_exits_1_after_printing_the_result() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("sessions"), "in the way").unwrap();
    let (code, out, err) = finish(with_args(dir.path(), &["--seed", "7", "roll", "d6"]));
    assert_eq!(code, 1);
    assert!(out.contains("d6 =>"), "{out}");
    assert!(err.starts_with("oraculum: cannot access"), "{err}");
}

#[test]
fn warnings_go_to_stderr_only() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("tables")).unwrap();
    fs::write(dir.path().join("tables/bad.toml"), "not [ toml").unwrap();
    let out = oraculum(dir.path())
        .arg("list")
        .assert()
        .code(0)
        .get_output()
        .clone();
    // Built-in content may print here (D-72); the warning must not.
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("warning") && !stdout.contains("bad.toml"),
        "{stdout}"
    );
    let err = String::from_utf8(out.stderr).unwrap();
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.starts_with("warning: tables/bad.toml: bad: "), "{err}");
}
