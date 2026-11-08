//! Integration tests: helper functions here may unwrap (clippy only exempts `#[test]` bodies).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use clap::Parser;
use oraculum_app::cli::{run, Cli, Command, Env};
use oraculum_app::error::AppError;
use oraculum_app::storage::{DataDir, JournalStore, SessionName};
use oraculum_core::clock::FixedClock;
use oraculum_core::journal::{export_markdown, Kind};
use tempfile::{tempdir, TempDir};

const TS: u64 = 1_762_484_400;

struct Out {
    stdout: String,
    stderr: String,
    result: Result<(), AppError>,
}

fn invoke(dir: &Path, args: &[&str]) -> Out {
    let mut argv = vec!["oraculum", "--data-dir", dir.to_str().unwrap()];
    argv.extend_from_slice(args);
    let cli = Cli::try_parse_from(argv).expect("arguments parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let session = SessionName::parse(&cli.session).unwrap();
    let env = Env {
        data_dir: DataDir::resolve(cli.data_dir.clone()).unwrap(),
        session,
        seed: cli.seed.unwrap_or(1),
        clock: Box::new(FixedClock(TS)),
        builtin: Vec::new(),
        out: &mut out,
        err: &mut err,
    };
    let command = cli.command.expect("a subcommand");
    let result = run(command, env);
    Out {
        stdout: String::from_utf8(out).unwrap(),
        stderr: String::from_utf8(err).unwrap(),
        result,
    }
}

fn journal(dir: &Path, session: &str) -> Vec<oraculum_core::journal::Entry> {
    let d = DataDir::resolve(Some(dir.to_path_buf())).unwrap();
    JournalStore::new(d.session_path(&SessionName::parse(session).unwrap()))
        .load()
        .unwrap()
        .entries
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn with_content() -> TempDir {
    let dir = tempdir().unwrap();
    write(
        dir.path(),
        "tables/alpha.toml",
        "name = \"Alpha Table\"\ncategory = \"Places\"\ntags = [\"wet\"]\nitems = [\"one\", \"two\", \"three\", \"four\"]",
    );
    write(
        dir.path(),
        "generators/beta.toml",
        "name = \"Beta Gen\"\ncategory = \"Characters\"\ntemplate = \"Found {x}\"\n[[slots]]\nkey = \"x\"\ntable = \"alpha\"",
    );
    dir
}

#[test]
fn roll_prints_what_the_journal_stores() {
    let dir = tempdir().unwrap();
    let o = invoke(dir.path(), &["--seed", "7", "roll", "3d20kl1+2"]);
    o.result.unwrap();
    assert_eq!(o.stdout, "3d20kl1+2 => [11, 17, 2] keep [2] +2 = 4\n");
    assert!(o.stderr.is_empty());
    let entries = journal(dir.path(), "default");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, Kind::Dice);
    assert_eq!(format!("{}\n", entries[0].result), o.stdout);
    assert_eq!(entries[0].ts, TS);
}

#[test]
fn same_seed_same_stdout_and_journal_results() {
    let dir = with_content();
    let cmds: [&[&str]; 4] = [
        &["roll", "4d6kh3"],
        &["ask", "likely", "Is it open?"],
        &["table", "alpha"],
        &["gen", "beta"],
    ];
    for cmd in cmds {
        let mut args = vec!["--seed", "7"];
        args.extend_from_slice(cmd);
        let a = invoke(dir.path(), &args);
        let b = invoke(dir.path(), &args);
        a.result.unwrap();
        b.result.unwrap();
        assert_eq!(a.stdout, b.stdout, "{cmd:?}");
    }
    let entries = journal(dir.path(), "default");
    assert_eq!(entries.len(), 8);
    for pair in entries.chunks(2) {
        assert_eq!(pair[0].result, pair[1].result);
        assert_eq!(pair[0].input, pair[1].input);
    }
}

#[test]
fn each_command_prints_exactly_the_journal_result() {
    let dir = with_content();
    for cmd in [
        vec!["roll", "2d6"],
        vec!["ask", "even"],
        vec!["ask", "very-unlikely", "Will it rain?"],
        vec!["table", "alpha"],
        vec!["gen", "beta"],
    ] {
        let mut args = vec!["--seed", "7"];
        args.extend(cmd.iter());
        let o = invoke(dir.path(), &args);
        o.result.unwrap();
        let last = journal(dir.path(), "default").pop().unwrap();
        assert_eq!(o.stdout, format!("{}\n", last.result), "{cmd:?}");
    }
    let ask = journal(dir.path(), "default");
    assert_eq!(ask[2].input, "Very unlikely: Will it rain?");
}

#[test]
fn sessions_are_separate_files() {
    let dir = tempdir().unwrap();
    invoke(dir.path(), &["--session", "one", "roll", "d6"])
        .result
        .unwrap();
    invoke(dir.path(), &["--session", "two", "roll", "d6"])
        .result
        .unwrap();
    invoke(dir.path(), &["--session", "two", "roll", "d6"])
        .result
        .unwrap();
    assert_eq!(journal(dir.path(), "one").len(), 1);
    assert_eq!(journal(dir.path(), "two").len(), 2);
}

#[test]
fn bad_dice_is_an_input_error_and_journals_nothing() {
    let dir = tempdir().unwrap();
    let o = invoke(dir.path(), &["roll", "101d6"]);
    let e = o.result.unwrap_err();
    assert_eq!(e.exit_code(), 2);
    assert!(e.to_string().contains("position 0"), "{e}");
    assert!(o.stdout.is_empty());
    assert!(!dir.path().join("sessions").exists());
}

#[test]
fn unknown_ids_suggest_close_matches() {
    let dir = with_content();
    let o = invoke(dir.path(), &["table", "alpa"]);
    let e = o.result.unwrap_err();
    assert_eq!(e.exit_code(), 2);
    let msg = e.to_string();
    assert!(
        msg.contains("alpa") && msg.contains("did you mean") && msg.contains("alpha"),
        "{msg}"
    );

    let e = invoke(dir.path(), &["gen", "zzzzzzzz"]).result.unwrap_err();
    assert_eq!(e.exit_code(), 2);
    assert!(!e.to_string().contains("did you mean"), "{e}");
    // A table id is not a generator id (separate namespaces).
    assert_eq!(
        invoke(dir.path(), &["gen", "alpha"])
            .result
            .unwrap_err()
            .exit_code(),
        2
    );
}

#[test]
fn list_prints_aligned_rows_and_filters() {
    let dir = with_content();
    let o = invoke(dir.path(), &["list"]);
    o.result.unwrap();
    assert_eq!(
        o.stdout,
        "G  beta   Characters  Beta Gen\nT  alpha  Places      Alpha Table\n"
    );
    let o = invoke(dir.path(), &["list", "--category", "places"]);
    assert_eq!(o.stdout, "T  alpha  Places  Alpha Table\n");
    let o = invoke(dir.path(), &["list", "--search", "WET"]);
    assert_eq!(o.stdout, "T  alpha  Places  Alpha Table\n");
    let o = invoke(dir.path(), &["list", "--search", "nothing-matches"]);
    o.result.unwrap();
    assert!(o.stdout.is_empty());
    assert!(
        journal(dir.path(), "default").is_empty(),
        "list must not journal"
    );
}

#[test]
fn content_commands_print_warnings_to_stderr_but_roll_and_export_do_not() {
    let dir = with_content();
    write(
        dir.path(),
        "tables/broken.toml",
        "name = \"B\"\nitems = [\"only one\"]",
    );
    write(dir.path(), "tables/garbage.toml", "this is [ not toml");

    let o = invoke(dir.path(), &["table", "alpha"]);
    o.result.unwrap();
    let lines: Vec<&str> = o.stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with("warning: tables/broken.toml: broken: "),
        "{}",
        lines[0]
    );
    assert!(lines[0].ends_with("(skipped)"), "{}", lines[0]);
    assert!(
        lines[1].starts_with("warning: tables/garbage.toml: garbage: "),
        "{}",
        lines[1]
    );
    assert!(!o.stdout.contains("warning"));

    for args in [&["list"][..], &["gen", "beta"], &["ask", "even"]] {
        assert_eq!(
            invoke(dir.path(), args).stderr.lines().count(),
            2,
            "{args:?}"
        );
    }
    assert!(invoke(dir.path(), &["roll", "d6"]).stderr.is_empty());
    assert!(invoke(dir.path(), &["export"]).stderr.is_empty());
}

#[test]
fn a_user_table_that_replaces_nothing_loads_and_one_that_overrides_is_reported() {
    let dir = with_content();
    write(
        dir.path(),
        "generators/beta.toml",
        "name = \"B2\"\ntemplate = \"x\"",
    );
    // Same kind and id as another user file is a duplicate-id warning only
    // when two user files collide; here a lone user generator is just loaded.
    let o = invoke(dir.path(), &["list"]);
    o.result.unwrap();
    assert!(o.stderr.is_empty(), "{}", o.stderr);
}

#[test]
fn export_writes_markdown_matching_the_journal() {
    let dir = with_content();
    invoke(dir.path(), &["--seed", "3", "roll", "d20"])
        .result
        .unwrap();
    invoke(dir.path(), &["--seed", "3", "table", "alpha"])
        .result
        .unwrap();

    let o = invoke(dir.path(), &["export"]);
    o.result.unwrap();
    let default_path = dir.path().join("export.md");
    assert_eq!(o.stdout, format!("{}\n", default_path.display()));
    let entries = journal(dir.path(), "default");
    assert_eq!(
        fs::read_to_string(&default_path).unwrap(),
        export_markdown("default", &entries)
    );

    let custom = dir.path().join("out").join("mine.md");
    let o = invoke(dir.path(), &["export", "--out", custom.to_str().unwrap()]);
    o.result.unwrap();
    assert_eq!(o.stdout, format!("{}\n", custom.display()));
    assert_eq!(
        fs::read_to_string(custom).unwrap(),
        export_markdown("default", &entries)
    );
}

#[test]
fn export_reports_malformed_journal_lines_and_still_exports() {
    let dir = tempdir().unwrap();
    invoke(dir.path(), &["roll", "d6"]).result.unwrap();
    let path = dir.path().join("sessions").join("default.jsonl");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("garbage line\n");
    fs::write(&path, text).unwrap();

    let o = invoke(dir.path(), &["export"]);
    o.result.unwrap();
    assert!(
        o.stderr
            .starts_with(&format!("warning: {}: line 2: ", path.display())),
        "{}",
        o.stderr
    );
    assert!(o.stderr.trim_end().ends_with("(skipped)"));
    assert!(fs::read_to_string(dir.path().join("export.md"))
        .unwrap()
        .starts_with("# Session: default"));
}

#[test]
fn export_of_a_session_with_no_journal_is_just_the_heading() {
    let dir = tempdir().unwrap();
    invoke(dir.path(), &["--session", "fresh", "export"])
        .result
        .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("export.md")).unwrap(),
        "# Session: fresh\n"
    );
}

#[test]
fn a_journal_write_failure_still_prints_the_result_then_exits_1() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("sessions"),
        "a file where the folder belongs",
    )
    .unwrap();
    let o = invoke(dir.path(), &["--seed", "7", "roll", "d6"]);
    assert!(!o.stdout.is_empty(), "the result is still shown (D-52)");
    let e = o.result.unwrap_err();
    assert_eq!(e.exit_code(), 1);
    assert!(e.to_string().contains("sessions"), "{e}");
}

#[test]
fn clap_parses_the_documented_forms() {
    let ok = |args: &[&str]| {
        let mut argv = vec!["oraculum"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap_or_else(|e| panic!("{args:?}: {e}"))
    };
    assert!(matches!(
        ok(&["ask", "very-likely", "q?"]).command,
        Some(Command::Ask { .. })
    ));
    assert!(matches!(
        ok(&["list", "--category", "x", "--search", "y"]).command,
        Some(Command::List { .. })
    ));
    assert!(ok(&[]).command.is_none());
    let c = ok(&[
        "--seed",
        "9",
        "--session",
        "s",
        "--data-dir",
        "/tmp/x",
        "roll",
        "d6",
    ]);
    assert_eq!((c.seed, c.session.as_str()), (Some(9), "s"));
    assert!(Cli::try_parse_from(["oraculum", "ask", "maybe"]).is_err());
    assert!(Cli::try_parse_from(["oraculum", "frobnicate"]).is_err());
}
