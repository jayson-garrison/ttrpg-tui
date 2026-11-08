//! Catalog: loads, validates and serves tables and generators (spec 5.7; A§3.8).
//!
//! `Catalog::build` never fails: every problem becomes a [`LoadWarning`].
//! Pipeline: parse each source, merge by (kind, id), then validate the
//! reference graph to a fixpoint (dangling references, cycles, nesting depth),
//! preferring to drop user items over built-ins (D-21).

mod load;
mod search;
mod validate;

use std::collections::BTreeMap;

use crate::error::EvalError;
use crate::generator::{Content, Generator, GeneratorResult};
use crate::oracle::TwistSource;
use crate::rng::Roller;
use crate::table::{Table, TableLookup, TableRoll};

pub use search::{Filter, ItemSummary};

/// Table and generator ids live in separate namespaces (D-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Table,
    Generator,
}

/// One content file's text, already read by the caller (core does no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: ItemKind,
    /// Where it came from, for warnings (a file name or `builtin:<name>`).
    pub origin: String,
    /// Default id when the file has none.
    pub file_stem: String,
    pub text: String,
}

impl Source {
    pub fn table(origin: &str, file_stem: &str, text: &str) -> Source {
        Source::new(ItemKind::Table, origin, file_stem, text)
    }

    pub fn generator(origin: &str, file_stem: &str, text: &str) -> Source {
        Source::new(ItemKind::Generator, origin, file_stem, text)
    }

    fn new(kind: ItemKind, origin: &str, file_stem: &str, text: &str) -> Source {
        Source {
            kind,
            origin: origin.into(),
            file_stem: file_stem.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// Not valid TOML or wrong shape (including unknown keys).
    Parse,
    /// Valid TOML, invalid content.
    Invalid,
    /// A user item replaced a built-in one (informational).
    Replaced,
    DuplicateUserId,
    Cycle,
    MissingReference,
    DepthExceeded,
    UnusedSlot,
    IgnoredField,
    /// A content file could not be read (added by the storage layer).
    FileUnreadable,
    /// A content file is over the size limit (added by the storage layer).
    FileTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadWarning {
    pub origin: String,
    pub item: Option<(ItemKind, String)>,
    pub kind: WarningKind,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Catalog {
    tables: BTreeMap<String, Table>,
    generators: BTreeMap<String, Generator>,
    items: Vec<ItemSummary>,
    /// Generators excluded for a missing reference, with the missing id.
    excluded: BTreeMap<String, String>,
}

impl Catalog {
    /// `builtin` and `user` must already be in a stable order (the reader sorts
    /// by file name). User items replace built-ins of the same kind and id.
    pub fn build(builtin: &[Source], user: &[Source]) -> (Catalog, Vec<LoadWarning>) {
        let mut warnings = Vec::new();
        let mut layers = load::load(builtin, user, &mut warnings);
        let excluded = validate::validate(&mut layers, &mut warnings);

        let mut cat = Catalog {
            excluded,
            ..Catalog::default()
        };
        for stack in layers.into_values() {
            match stack.into_iter().last().map(|c| c.item) {
                Some(load::Item::Table(t)) => {
                    cat.tables.insert(t.id.clone(), t);
                }
                Some(load::Item::Generator(g)) => {
                    cat.generators.insert(g.id.clone(), g);
                }
                None => {}
            }
        }
        cat.items = search::summaries(&cat.tables, &cat.generators);
        (cat, warnings)
    }

    pub fn table(&self, id: &str) -> Option<&Table> {
        self.tables.get(id)
    }

    pub fn generator(&self, id: &str) -> Option<&Generator> {
        self.generators.get(id)
    }

    /// Sorted by category, then name (case-insensitive), then id.
    pub fn items(&self) -> &[ItemSummary] {
        &self.items
    }

    pub fn categories(&self) -> Vec<String> {
        search::categories(&self.items)
    }

    pub fn search(&self, filter: &Filter) -> Vec<&ItemSummary> {
        self.items.iter().filter(|i| filter.matches(i)).collect()
    }

    pub fn roll_table(&self, id: &str, roller: &mut dyn Roller) -> Result<TableRoll, EvalError> {
        let table = self
            .tables
            .get(id)
            .ok_or_else(|| EvalError::UnknownTable(id.into()))?;
        table.roll(self, roller)
    }

    pub fn generate(
        &self,
        id: &str,
        roller: &mut dyn Roller,
    ) -> Result<GeneratorResult, EvalError> {
        let generator = self
            .generators
            .get(id)
            .ok_or_else(|| EvalError::UnknownGenerator(id.into()))?;
        generator.generate(self, roller)
    }

    /// Re-roll one slot of a previous result; everything else is kept.
    pub fn reroll_slot(
        &self,
        prev: &GeneratorResult,
        slot: usize,
        roller: &mut dyn Roller,
    ) -> Result<GeneratorResult, EvalError> {
        let generator = self
            .generators
            .get(&prev.id)
            .ok_or_else(|| EvalError::UnknownGenerator(prev.id.clone()))?;
        generator.reroll_slot(prev, slot, self, roller)
    }
}

impl TableLookup for Catalog {
    fn table(&self, id: &str) -> Option<&Table> {
        self.tables.get(id)
    }
}

impl Content for Catalog {
    fn generator(&self, id: &str) -> Option<&Generator> {
        self.generators.get(id)
    }
}

impl TwistSource for Catalog {
    type Event = GeneratorResult;

    fn roll_twist(
        &self,
        generator: &str,
        roller: &mut dyn Roller,
    ) -> Result<GeneratorResult, String> {
        self.generate(generator, roller).map_err(|e| match e {
            // Excluded at load because of a missing reference: name that.
            EvalError::UnknownGenerator(id) => self.excluded.get(&id).cloned().unwrap_or(id),
            EvalError::UnknownTable(id) => id,
            _ => generator.to_string(),
        })
    }
}

#[cfg(test)]
mod tests;
