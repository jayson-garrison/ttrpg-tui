//! Library listing: sorted summaries, categories and filtering.

use std::collections::BTreeMap;

use super::ItemKind;
use crate::generator::Generator;
use crate::table::Table;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    pub kind: ItemKind,
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Case-insensitive substring over id, name, category and tags.
    pub text: Option<String>,
    /// Case-insensitive exact category.
    pub category: Option<String>,
    pub kind: Option<ItemKind>,
}

impl Filter {
    pub fn matches(&self, item: &ItemSummary) -> bool {
        if self.kind.is_some_and(|k| k != item.kind) {
            return false;
        }
        if let Some(c) = &self.category {
            if !item.category.eq_ignore_ascii_case(c) {
                return false;
            }
        }
        match &self.text {
            None => true,
            Some(text) => {
                let needle = text.to_lowercase();
                [&item.id, &item.name, &item.category]
                    .into_iter()
                    .chain(&item.tags)
                    .any(|field| field.to_lowercase().contains(&needle))
            }
        }
    }
}

pub(super) fn summaries(
    tables: &BTreeMap<String, Table>,
    generators: &BTreeMap<String, Generator>,
) -> Vec<ItemSummary> {
    let mut items: Vec<ItemSummary> = tables
        .values()
        .map(|t| ItemSummary {
            kind: ItemKind::Table,
            id: t.id.clone(),
            name: t.name.clone(),
            category: t.category.clone(),
            tags: t.tags.clone(),
            description: t.description.clone(),
        })
        .chain(generators.values().map(|g| ItemSummary {
            kind: ItemKind::Generator,
            id: g.id.clone(),
            name: g.name.clone(),
            category: g.category.clone(),
            tags: g.tags.clone(),
            description: g.description.clone(),
        }))
        .collect();
    items.sort_by_cached_key(|i| {
        (
            i.category.to_lowercase(),
            i.name.to_lowercase(),
            i.id.clone(),
            i.kind,
        )
    });
    items
}

pub(super) fn categories(items: &[ItemSummary]) -> Vec<String> {
    let mut cats: Vec<String> = items.iter().map(|i| i.category.clone()).collect();
    cats.sort_by_cached_key(|c| (c.to_lowercase(), c.clone()));
    cats.dedup();
    cats
}
