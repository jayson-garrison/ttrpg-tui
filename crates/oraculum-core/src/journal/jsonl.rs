//! JSON Lines codec.

use super::{Entry, JournalError, JournalLoad, SkippedLine};

/// One line, no trailing newline.
pub fn encode_line(entry: &Entry) -> Result<String, JournalError> {
    serde_json::to_string(entry).map_err(|e| JournalError::Encode(e.to_string()))
}

/// Parse a whole journal. Blank lines are ignored; malformed lines are skipped
/// and reported with their line number.
pub fn decode(text: &str) -> JournalLoad {
    let mut load = JournalLoad::default();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Entry>(line) {
            Ok(entry) => load.entries.push(entry),
            Err(e) => load.skipped.push(SkippedLine {
                line_no: i + 1,
                reason: e.to_string(),
            }),
        }
    }
    load
}
