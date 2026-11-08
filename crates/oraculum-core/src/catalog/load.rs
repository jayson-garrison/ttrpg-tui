//! Stages 1-2: parse each source, then merge by (kind, id).

use std::collections::BTreeMap;

use super::{ItemKind, LoadWarning, Source, WarningKind};
use crate::generator::{Generator, GeneratorWarning};
use crate::table::{Table, TableError};

pub(super) type Key = (ItemKind, String);

pub(super) enum Item {
    Table(Table),
    Generator(Generator),
}

pub(super) struct Candidate {
    pub item: Item,
    pub user: bool,
    pub origin: String,
}

impl Candidate {
    /// Ids this item depends on, as (kind, id) keys.
    pub fn edges(&self) -> Vec<Key> {
        match &self.item {
            Item::Table(t) => t
                .table_refs()
                .into_iter()
                .map(|id| (ItemKind::Table, id.to_string()))
                .collect(),
            Item::Generator(g) => g
                .table_refs()
                .map(|id| (ItemKind::Table, id.to_string()))
                .chain(
                    g.generator_refs()
                        .map(|id| (ItemKind::Generator, id.to_string())),
                )
                .collect(),
        }
    }
}

/// Per (kind, id): built-in first, then user items in file order. The last
/// entry is the effective one.
pub(super) type Layers = BTreeMap<Key, Vec<Candidate>>;

pub(super) fn load(builtin: &[Source], user: &[Source], warnings: &mut Vec<LoadWarning>) -> Layers {
    let mut layers = Layers::new();
    for (source, is_user) in builtin
        .iter()
        .map(|s| (s, false))
        .chain(user.iter().map(|s| (s, true)))
    {
        let Some((key, item)) = parse(source, warnings) else {
            continue;
        };
        let stack = layers.entry(key.clone()).or_default();
        if is_user {
            match stack.last() {
                Some(prev) if prev.user => warnings.push(warning(
                    source,
                    &key,
                    WarningKind::DuplicateUserId,
                    format!(
                        "replaces {} with the same id (the later file wins)",
                        prev.origin
                    ),
                )),
                Some(_) => warnings.push(warning(
                    source,
                    &key,
                    WarningKind::Replaced,
                    "replaces the built-in item with the same id".to_string(),
                )),
                None => {}
            }
        }
        stack.push(Candidate {
            item,
            user: is_user,
            origin: source.origin.clone(),
        });
    }
    layers
}

fn warning(source: &Source, key: &Key, kind: WarningKind, message: String) -> LoadWarning {
    LoadWarning {
        origin: source.origin.clone(),
        item: Some(key.clone()),
        kind,
        message,
    }
}

fn parse(source: &Source, warnings: &mut Vec<LoadWarning>) -> Option<(Key, Item)> {
    let text = source.text.strip_prefix('\u{feff}').unwrap_or(&source.text);
    let stem_key = (source.kind, source.file_stem.clone());
    match source.kind {
        ItemKind::Table => match Table::from_toml(text, &source.file_stem) {
            Ok(t) => Some(((ItemKind::Table, t.id.clone()), Item::Table(t))),
            Err(e) => {
                let kind = if matches!(e, TableError::Toml(_)) {
                    WarningKind::Parse
                } else {
                    WarningKind::Invalid
                };
                warnings.push(warning(source, &stem_key, kind, e.to_string()));
                None
            }
        },
        ItemKind::Generator => match Generator::from_toml(text, &source.file_stem) {
            Ok((g, notes)) => {
                let key = (ItemKind::Generator, g.id.clone());
                for note in notes {
                    let kind = match note {
                        GeneratorWarning::UnusedSlot(_) => WarningKind::UnusedSlot,
                        GeneratorWarning::IgnoredField { .. } => WarningKind::IgnoredField,
                    };
                    warnings.push(warning(source, &key, kind, note.to_string()));
                }
                Some((key.clone(), Item::Generator(g)))
            }
            Err(e) => {
                let kind = if matches!(e, crate::generator::GeneratorError::Toml(_)) {
                    WarningKind::Parse
                } else {
                    WarningKind::Invalid
                };
                warnings.push(warning(source, &stem_key, kind, e.to_string()));
                None
            }
        },
    }
}
