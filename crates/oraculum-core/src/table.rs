//! Tables (spec 5.4; A§3.6; D-11, D-18, D-28).
//!
//! `Table::from_toml` validates everything up front; rolling validated tables
//! only fails through the defensive depth guard or a missing referenced table.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use thiserror::Error;

use crate::dice::{DiceError, DiceExpr, DiceRoll, Totals};
use crate::error::{EvalError, MAX_DEPTH};
use crate::rng::Roller;
use crate::template::{is_valid_item_id, Segment, Template, TemplateContext, TemplateError};

const MIN_ITEMS: usize = 2;
const MAX_ITEMS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableForm {
    Ranged,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    pub lo: i64,
    pub hi: i64,
    pub text: Template,
    /// Entry text before resolution.
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
    /// List tables: `1dN` derived from the item count.
    pub dice: DiceExpr,
    pub form: TableForm,
    /// Sorted by `lo`; validated free of gaps and overlaps.
    pub entries: Vec<TableEntry>,
}

/// Validation errors. `entry` is the zero-based position in the file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TableError {
    #[error("table has no entries")]
    EmptyTable,
    #[error("entry {entry} has empty text")]
    EmptyText { entry: usize },
    #[error("entry {entry} has an invalid range: {lo} is greater than {hi}")]
    InvalidRange { entry: usize, lo: i64, hi: i64 },
    #[error("no entry covers {from} to {to}")]
    Gap { from: i64, to: i64 },
    #[error("entry {entry} overlaps an earlier entry")]
    Overlap { entry: usize },
    #[error("entry {entry} range {lo}-{hi} is outside the dice totals {min}-{max}")]
    OutOfRange {
        entry: usize,
        lo: i64,
        hi: i64,
        min: i64,
        max: i64,
    },
    #[error("entry {entry} contains {value}, which is not a d66 value (both digits must be 1-6)")]
    BadD66Value { entry: usize, value: i64 },
    #[error("a list table needs 2 to 1000 items, got {got}")]
    ListSize { got: usize },
    #[error("`items` cannot be combined with `dice` or `entries`")]
    ConflictingFields,
    #[error("a table with `entries` needs `dice`")]
    MissingDice,
    #[error("a table needs a `name`")]
    MissingName,
    #[error("invalid table id {0:?} (use a-z, 0-9, '_' and '-', starting with a letter or digit)")]
    BadId(String),
    #[error("d66 is only valid as the whole dice expression of a table")]
    UnsupportedDice,
    #[error("invalid dice: {0}")]
    BadDice(#[source] DiceError),
    #[error("entry {entry}: {source}")]
    BadTemplate {
        entry: usize,
        #[source]
        source: TemplateError,
    },
    #[error("invalid table file: {0}")]
    Toml(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    id: Option<String>,
    name: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
    dice: Option<String>,
    items: Option<Vec<String>>,
    entries: Option<Vec<RawEntry>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    range: [i64; 2],
    text: String,
}

/// Finds tables by id when resolving `{table:..}` references.
pub trait TableLookup {
    fn table(&self, id: &str) -> Option<&Table>;
}

impl TableLookup for BTreeMap<String, Table> {
    fn table(&self, id: &str) -> Option<&Table> {
        self.get(id)
    }
}

/// A resolved `{table:..}` or `{roll:..}` inside an entry, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedRef {
    Table(Box<TableRoll>),
    Dice(DiceRoll),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRoll {
    pub table_id: String,
    pub dice: DiceRoll,
    /// Index into `Table::entries`.
    pub entry_index: usize,
    /// Entry text before resolution.
    pub raw: String,
    /// Fully resolved text.
    pub text: String,
    pub refs: Vec<ResolvedRef>,
}

impl Table {
    /// Parse and validate a table file. `default_id` is used when the file has
    /// no `id` (the loader passes the file stem).
    pub fn from_toml(src: &str, default_id: &str) -> Result<Table, TableError> {
        let raw: RawTable = toml::from_str(src).map_err(|e| TableError::Toml(e.to_string()))?;

        let id = raw.id.unwrap_or_else(|| default_id.to_string());
        if !is_valid_item_id(&id) {
            return Err(TableError::BadId(id));
        }
        let name = match raw.name {
            Some(n) if !n.trim().is_empty() => n,
            _ => return Err(TableError::MissingName),
        };
        let category = raw
            .category
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| "Uncategorized".to_string());
        let tags = raw.tags.unwrap_or_default();

        let (dice, form, entries) = match (raw.items, raw.entries) {
            (Some(_), Some(_)) => return Err(TableError::ConflictingFields),
            (Some(_), None) if raw.dice.is_some() => return Err(TableError::ConflictingFields),
            (Some(items), None) => list_entries(items)?,
            (None, Some(entries)) => ranged_entries(raw.dice, entries)?,
            (None, None) => return Err(TableError::EmptyTable),
        };

        Ok(Table {
            id,
            name,
            category,
            tags,
            description: raw.description,
            dice,
            form,
            entries,
        })
    }

    /// Ids named by `{table:..}` in any entry (sorted, deduplicated).
    pub fn table_refs(&self) -> Vec<&str> {
        let set: BTreeSet<&str> = self
            .entries
            .iter()
            .flat_map(|e| e.text.table_refs())
            .collect();
        set.into_iter().collect()
    }

    /// Roll the table, resolving references through `lookup`.
    pub fn roll(
        &self,
        lookup: &dyn TableLookup,
        roller: &mut dyn Roller,
    ) -> Result<TableRoll, EvalError> {
        self.roll_at(lookup, roller, 0)
    }

    fn roll_at(
        &self,
        lookup: &dyn TableLookup,
        roller: &mut dyn Roller,
        depth: usize,
    ) -> Result<TableRoll, EvalError> {
        let dice = self.dice.roll(roller);
        let total = dice.total;
        let idx = self.entries.partition_point(|e| e.lo <= total);
        let entry_index = idx.checked_sub(1).ok_or_else(|| self.no_entry(total))?;
        let entry = &self.entries[entry_index];
        if total > entry.hi {
            return Err(self.no_entry(total));
        }

        let mut text = String::new();
        let mut refs = Vec::new();
        for seg in entry.text.segments() {
            match seg {
                Segment::Literal(s) => text.push_str(s),
                Segment::Roll(expr) => {
                    let r = expr.roll(roller);
                    text.push_str(&r.total.to_string());
                    refs.push(ResolvedRef::Dice(r));
                }
                Segment::TableRef(id) => {
                    if depth + 1 > MAX_DEPTH {
                        return Err(EvalError::DepthExceeded { max: MAX_DEPTH });
                    }
                    let other = lookup
                        .table(id)
                        .ok_or_else(|| EvalError::UnknownTable(id.clone()))?;
                    let r = other.roll_at(lookup, roller, depth + 1)?;
                    text.push_str(&r.text);
                    refs.push(ResolvedRef::Table(Box::new(r)));
                }
                // Parsing rejects slots in table entries; if one ever got here,
                // show it as written rather than dropping it silently.
                Segment::Slot(key) => {
                    text.push('{');
                    text.push_str(key);
                    text.push('}');
                }
            }
        }
        Ok(TableRoll {
            table_id: self.id.clone(),
            dice,
            entry_index,
            raw: entry.raw.clone(),
            text,
            refs,
        })
    }

    fn no_entry(&self, total: i64) -> EvalError {
        EvalError::NoEntry {
            table: self.id.clone(),
            total,
        }
    }
}

fn parse_entry_text(entry: usize, text: &str) -> Result<Template, TableError> {
    if text.trim().is_empty() {
        return Err(TableError::EmptyText { entry });
    }
    Template::parse(text, TemplateContext::TableEntry)
        .map_err(|source| TableError::BadTemplate { entry, source })
}

type Built = (DiceExpr, TableForm, Vec<TableEntry>);

fn list_entries(items: Vec<String>) -> Result<Built, TableError> {
    if !(MIN_ITEMS..=MAX_ITEMS).contains(&items.len()) {
        return Err(TableError::ListSize { got: items.len() });
    }
    let dice = DiceExpr::parse(&format!("1d{}", items.len())).map_err(TableError::BadDice)?;
    let mut entries = Vec::with_capacity(items.len());
    for (i, raw) in items.into_iter().enumerate() {
        let text = parse_entry_text(i, &raw)?;
        let n = i as i64 + 1;
        entries.push(TableEntry {
            lo: n,
            hi: n,
            text,
            raw,
        });
    }
    Ok((dice, TableForm::List, entries))
}

fn ranged_entries(dice: Option<String>, raw_entries: Vec<RawEntry>) -> Result<Built, TableError> {
    if raw_entries.is_empty() {
        return Err(TableError::EmptyTable);
    }
    let dice_src = dice.ok_or(TableError::MissingDice)?;
    let dice = DiceExpr::parse(&dice_src).map_err(TableError::BadDice)?;
    if dice.contains_d66() && !dice.is_bare_d66() {
        return Err(TableError::UnsupportedDice);
    }

    // (original index, entry); validated per entry first, then coverage.
    let mut entries: Vec<(usize, TableEntry)> = Vec::with_capacity(raw_entries.len());
    for (i, e) in raw_entries.into_iter().enumerate() {
        let [lo, hi] = e.range;
        if lo > hi {
            return Err(TableError::InvalidRange { entry: i, lo, hi });
        }
        let text = parse_entry_text(i, &e.text)?;
        entries.push((
            i,
            TableEntry {
                lo,
                hi,
                text,
                raw: e.text,
            },
        ));
    }

    // Sorted by `lo`, ties by file position, so overlap errors name the later entry.
    entries.sort_by_key(|(i, e)| (e.lo, *i));
    match dice.totals() {
        Totals::Contiguous { min, max } => check_contiguous(&entries, min, max)?,
        Totals::D66 => check_d66(&entries)?,
    }

    Ok((
        dice,
        TableForm::Ranged,
        entries.into_iter().map(|(_, e)| e).collect(),
    ))
}

fn check_contiguous(entries: &[(usize, TableEntry)], min: i64, max: i64) -> Result<(), TableError> {
    for (i, e) in entries.iter() {
        if e.lo < min || e.hi > max {
            return Err(TableError::OutOfRange {
                entry: *i,
                lo: e.lo,
                hi: e.hi,
                min,
                max,
            });
        }
    }
    let mut next = min;
    for (i, e) in entries.iter() {
        if e.lo > next {
            return Err(TableError::Gap {
                from: next,
                to: e.lo - 1,
            });
        }
        if e.lo < next {
            return Err(TableError::Overlap { entry: *i });
        }
        next = e.hi + 1;
    }
    if next <= max {
        return Err(TableError::Gap {
            from: next,
            to: max,
        });
    }
    Ok(())
}

fn is_d66_value(v: i64) -> bool {
    (11..=66).contains(&v) && (1..=6).contains(&(v % 10)) && (1..=6).contains(&(v / 10))
}

fn check_d66(entries: &[(usize, TableEntry)]) -> Result<(), TableError> {
    let mut seen = BTreeSet::new();
    for (i, e) in entries {
        // Any 101 consecutive integers contain a non-d66 value, so a long
        // range is rejected after inspecting only its first 101 values.
        for v in (e.lo..=e.hi).take(101) {
            if !is_d66_value(v) {
                return Err(TableError::BadD66Value {
                    entry: *i,
                    value: v,
                });
            }
            if !seen.insert(v) {
                return Err(TableError::Overlap { entry: *i });
            }
        }
    }
    let missing = (1..=6)
        .flat_map(|t| (1..=6).map(move |u| t * 10 + u))
        .find(|v| !seen.contains(v));
    match missing {
        Some(v) => Err(TableError::Gap { from: v, to: v }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
