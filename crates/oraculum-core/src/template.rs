//! Text templates shared by table entries and generator templates (A§3.5).
//!
//! `{{` and `}}` are literal braces. Substitution is single-pass: text that a
//! placeholder produces is never re-scanned (D-10).

use thiserror::Error;

use crate::dice::{DiceError, DiceExpr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    /// `{table:id}` (table entries only).
    TableRef(String),
    /// `{roll:expr}` (both contexts).
    Roll(DiceExpr),
    /// `{key}` (generator templates only).
    Slot(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateContext {
    TableEntry,
    GeneratorTemplate,
}

/// `pos` is a byte offset of the offending `{` or `}`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemplateError {
    #[error("unmatched '{{' at position {pos} (write '{{{{' for a literal brace)")]
    UnmatchedOpen { pos: usize },
    #[error("unmatched '}}' at position {pos} (write '}}}}' for a literal brace)")]
    UnmatchedClose { pos: usize },
    #[error("empty placeholder at position {pos}")]
    EmptyPlaceholder { pos: usize },
    #[error("unknown placeholder {{{text}}} at position {pos}")]
    UnknownPlaceholder { pos: usize, text: String },
    #[error("placeholder {{{text}}} at position {pos} is not allowed here")]
    WrongContext { pos: usize, text: String },
    #[error("invalid identifier in {{{text}}} at position {pos}")]
    BadId { pos: usize, text: String },
    #[error("bad dice in placeholder at position {pos}: {source}")]
    BadDice {
        pos: usize,
        #[source]
        source: DiceError,
    },
}

/// Item ids: `[a-z0-9][a-z0-9_-]{0,63}`.
pub fn is_valid_item_id(s: &str) -> bool {
    let mut chars = s.chars();
    let ok_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    ok_first
        && s.len() <= 64
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Slot keys: `[a-z_][a-z0-9_]{0,31}`.
pub fn is_valid_slot_key(s: &str) -> bool {
    let mut chars = s.chars();
    let ok_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    ok_first
        && s.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    segments: Vec<Segment>,
}

impl Template {
    pub fn parse(src: &str, ctx: TemplateContext) -> Result<Template, TemplateError> {
        let bytes = src.as_bytes();
        let mut segments: Vec<Segment> = Vec::new();
        let mut lit = String::new();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'{' if bytes.get(i + 1) == Some(&b'{') => {
                    lit.push('{');
                    i += 2;
                }
                b'}' if bytes.get(i + 1) == Some(&b'}') => {
                    lit.push('}');
                    i += 2;
                }
                b'}' => return Err(TemplateError::UnmatchedClose { pos: i }),
                b'{' => {
                    let rest = &src[i + 1..];
                    // The placeholder ends at the first '}'; a '{' before it is an error.
                    let end = rest
                        .find(['{', '}'])
                        .filter(|&e| rest.as_bytes()[e] == b'}')
                        .ok_or(TemplateError::UnmatchedOpen { pos: i })?;
                    let seg = parse_placeholder(&rest[..end], i, ctx)?;
                    if !lit.is_empty() {
                        segments.push(Segment::Literal(std::mem::take(&mut lit)));
                    }
                    segments.push(seg);
                    i += end + 2;
                }
                _ => {
                    // Copy one whole char (src is valid UTF-8; '{' and '}' are ASCII).
                    let ch_len = src[i..].chars().next().map_or(1, char::len_utf8);
                    lit.push_str(&src[i..i + ch_len]);
                    i += ch_len;
                }
            }
        }
        if !lit.is_empty() {
            segments.push(Segment::Literal(lit));
        }
        Ok(Template { segments })
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Ids named by `{table:..}` placeholders, in order.
    pub fn table_refs(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().filter_map(|s| match s {
            Segment::TableRef(id) => Some(id.as_str()),
            _ => None,
        })
    }

    /// Keys named by `{key}` placeholders, in order.
    pub fn slot_keys(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().filter_map(|s| match s {
            Segment::Slot(k) => Some(k.as_str()),
            _ => None,
        })
    }
}

fn parse_placeholder(
    inner: &str,
    pos: usize,
    ctx: TemplateContext,
) -> Result<Segment, TemplateError> {
    if inner.is_empty() {
        return Err(TemplateError::EmptyPlaceholder { pos });
    }
    let text = || inner.to_string();
    if let Some(id) = inner.strip_prefix("table:") {
        if ctx != TemplateContext::TableEntry {
            return Err(TemplateError::WrongContext { pos, text: text() });
        }
        return if is_valid_item_id(id) {
            Ok(Segment::TableRef(id.to_string()))
        } else {
            Err(TemplateError::BadId { pos, text: text() })
        };
    }
    if let Some(expr) = inner.strip_prefix("roll:") {
        return DiceExpr::parse(expr)
            .map(Segment::Roll)
            .map_err(|source| TemplateError::BadDice { pos, source });
    }
    match ctx {
        TemplateContext::GeneratorTemplate if is_valid_slot_key(inner) => Ok(Segment::Slot(text())),
        TemplateContext::GeneratorTemplate => Err(TemplateError::BadId { pos, text: text() }),
        TemplateContext::TableEntry => Err(TemplateError::UnknownPlaceholder { pos, text: text() }),
    }
}

#[cfg(test)]
mod tests;
