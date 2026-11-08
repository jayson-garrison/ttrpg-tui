use serde_json::json;

use super::*;

fn entry(ts: u64, kind: Kind, input: &str, result: &str) -> Entry {
    Entry::new(ts, kind, input, result, serde_json::Value::Null)
}

#[test]
fn format_utc_known_epochs() {
    // Expected values come from independent sources (`date -u -r <ts>`).
    let cases = [
        (0, "1970-01-01 00:00"),
        (86_399, "1970-01-01 23:59"),
        (951_782_400, "2000-02-29 00:00"),
        (1_700_000_000, "2023-11-14 22:13"),
        (1_709_208_000, "2024-02-29 12:00"),
        (2_147_483_647, "2038-01-19 03:14"),
        (4_102_444_800, "2100-01-01 00:00"),
        (4_107_542_400, "2100-03-01 00:00"),
        (1_762_484_400, "2025-11-07 03:00"),
    ];
    for (ts, want) in cases {
        assert_eq!(format_utc(ts), want, "{ts}");
    }
}

#[test]
fn encode_is_one_exact_line() {
    let e = Entry::new(
        1_762_484_400,
        Kind::Dice,
        "2d6+1",
        "2d6+1 => [3, 5] +1 = 9",
        json!({"total": 9}),
    );
    assert_eq!(
        encode_line(&e).unwrap(),
        r#"{"v":1,"ts":1762484400,"kind":"dice","input":"2d6+1","result":"2d6+1 => [3, 5] +1 = 9","detail":{"total":9}}"#
    );
}

#[test]
fn multi_line_text_stays_on_one_line_and_round_trips() {
    let e = entry(
        5,
        Kind::Oracle,
        "Even: Is it?",
        "Even: Yes (d100: 11)\nTwist: x",
    );
    let line = encode_line(&e).unwrap();
    assert!(!line.contains('\n'));
    let load = decode(&format!("{line}\n"));
    assert!(load.skipped.is_empty());
    assert_eq!(load.entries, [e]);
}

#[test]
fn every_kind_round_trips_lowercase() {
    for (kind, name) in [
        (Kind::Oracle, "oracle"),
        (Kind::Table, "table"),
        (Kind::Generator, "generator"),
        (Kind::Dice, "dice"),
        (Kind::Note, "note"),
    ] {
        let line = encode_line(&entry(1, kind, "i", "r")).unwrap();
        assert!(line.contains(&format!("\"kind\":\"{name}\"")), "{line}");
        assert_eq!(decode(&line).entries[0].kind, kind);
    }
}

#[test]
fn decode_skips_blank_lines_and_reports_malformed_ones_with_numbers() {
    let good = encode_line(&entry(1, Kind::Note, "", "hi")).unwrap();
    let text = format!(
        "{good}\n\n   \nnot json\n{{\"v\":1,\"ts\":2,\"kind\":\"weird\",\"input\":\"\",\"result\":\"\"}}\n{good}\n{{\"v\":1,\"ts\":3,\"ki"
    );
    let load = decode(&text);
    assert_eq!(load.entries.len(), 2);
    let lines: Vec<usize> = load.skipped.iter().map(|s| s.line_no).collect();
    assert_eq!(lines, [4, 5, 7]);
    assert!(load.skipped.iter().all(|s| !s.reason.is_empty()));
}

#[test]
fn decode_tolerates_missing_optional_fields_extra_fields_and_crlf() {
    let text = "{\"ts\":9,\"kind\":\"note\",\"input\":\"\",\"result\":\"x\",\"future\":true}\r\n";
    let load = decode(text);
    assert!(load.skipped.is_empty(), "{:?}", load.skipped);
    let e = &load.entries[0];
    assert_eq!(
        (e.v, e.ts, e.detail.clone()),
        (1, 9, serde_json::Value::Null)
    );
}

#[test]
fn export_markdown_exact_layout() {
    let entries = [
        entry(1_762_484_400, Kind::Dice, "2d6+1", "2d6+1 => [3, 5] +1 = 9"),
        entry(
            1_762_484_460,
            Kind::Oracle,
            "Even: Is it locked?",
            "Even: Yes (d100: 11)\nTwist: a crow",
        ),
        entry(1_762_484_520, Kind::Note, "", "Remember the key."),
        entry(1_762_484_580, Kind::Table, "", "A rusty sword"),
        entry(1_762_484_640, Kind::Generator, "tavern", "The Red Inn"),
        entry(1_762_484_700, Kind::Note, "", "Second note."),
    ];
    let want = "\
# Session: demo

- 2025-11-07 03:00 UTC · dice · 2d6+1 → 2d6+1 => [3, 5] +1 = 9
- 2025-11-07 03:01 UTC · oracle · Even: Is it locked? → Even: Yes (d100: 11)
  Twist: a crow

2025-11-07 03:02 UTC — note: Remember the key.

- 2025-11-07 03:03 UTC · table · A rusty sword
- 2025-11-07 03:04 UTC · generator · tavern → The Red Inn

2025-11-07 03:05 UTC — note: Second note.
";
    assert_eq!(export_markdown("demo", &entries), want);
}

#[test]
fn export_of_an_empty_journal_is_just_the_heading() {
    assert_eq!(export_markdown("empty", &[]), "# Session: empty\n");
}
