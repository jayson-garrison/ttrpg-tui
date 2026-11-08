//! Markdown export (D-25): a pure function of the entries.

use super::{format_utc, Entry, Kind};

/// Heading, then entries in file order. Consecutive non-note entries form one
/// bullet list; every note is a paragraph with a blank line on both sides.
pub fn export_markdown(session: &str, entries: &[Entry]) -> String {
    let mut out = format!("# Session: {session}\n");
    let mut prev: Option<Kind> = None;
    for e in entries {
        let is_note = e.kind == Kind::Note;
        if is_note || prev.is_none() || prev == Some(Kind::Note) {
            out.push('\n');
        }
        let when = format_utc(e.ts);
        if is_note {
            out.push_str(&format!("{when} UTC — note: {}\n", e.result));
        } else {
            let mut lines = e.result.lines();
            let first = lines.next().unwrap_or("");
            if e.input.is_empty() {
                out.push_str(&format!("- {when} UTC · {} · {first}\n", e.kind.label()));
            } else {
                out.push_str(&format!(
                    "- {when} UTC · {} · {} → {first}\n",
                    e.kind.label(),
                    e.input
                ));
            }
            for line in lines {
                out.push_str(&format!("  {line}\n"));
            }
        }
        prev = Some(e.kind);
    }
    out
}
