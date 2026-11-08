//! Session journal model and codecs (spec 5.6; A§3.9; D-22..D-25).
//!
//! Pure: no file I/O. The storage layer in the binary reads and appends lines.

mod jsonl;
mod markdown;
mod time;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use jsonl::{decode, encode_line};
pub use markdown::export_markdown;
pub use time::format_utc;

/// What produced an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Oracle,
    Table,
    Generator,
    Dice,
    Note,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Oracle => "oracle",
            Self::Table => "table",
            Self::Generator => "generator",
            Self::Dice => "dice",
            Self::Note => "note",
        }
    }
}

fn schema_version() -> u8 {
    1
}

/// One journal line. Unknown extra fields are ignored on load; an unknown
/// `kind` makes the line malformed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default = "schema_version")]
    pub v: u8,
    /// Unix epoch seconds, UTC.
    pub ts: u64,
    pub kind: Kind,
    pub input: String,
    /// Exactly what the headless command prints.
    pub result: String,
    /// Informational only; nothing reads it back (D-23).
    #[serde(default)]
    pub detail: serde_json::Value,
}

impl Entry {
    pub fn new(
        ts: u64,
        kind: Kind,
        input: impl Into<String>,
        result: impl Into<String>,
        detail: serde_json::Value,
    ) -> Entry {
        Entry {
            v: schema_version(),
            ts,
            kind,
            input: input.into(),
            result: result.into(),
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedLine {
    /// One-based.
    pub line_no: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JournalLoad {
    pub entries: Vec<Entry>,
    pub skipped: Vec<SkippedLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JournalError {
    #[error("could not encode journal entry: {0}")]
    Encode(String),
}

#[cfg(test)]
mod tests;
