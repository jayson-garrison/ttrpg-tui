//! Content tests for the built-in tables and generators (T6_PLAN section 6).
//!
//! The only place that pins built-in output (D-72).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use oraculum_core::builtin;
use oraculum_core::catalog::Catalog;
use oraculum_core::generator::{Count, SlotSource};
use oraculum_core::rng::RngRoller;
use oraculum_core::table::TableForm;

const CATEGORIES: &[&str] = &[
    "Characters",
    "Creatures",
    "Places",
    "Sites",
    "Adventure",
    "Items",
    "Organizations",
    "Oracle",
];

const DENYLIST: &str = include_str!("data/denylist.txt");

/// Minimum list size per table (plan section 5); everything else has the
/// spec floor of 12.
const MIN_ITEMS: &[(&str, usize)] = &[
    ("given_names", 60),
    ("family_names", 60),
    ("npc_occupation", 40),
    ("npc_trait", 20),
    ("creature_form", 20),
    ("atmosphere", 20),
    ("place_adjective", 30),
    ("place_noun", 30),
    ("rumor_claim", 20),
    ("item_base", 20),
    ("item_power", 20),
    ("meaning_action", 20),
    ("meaning_subject", 20),
];

const REQUIRED_GENERATORS: &[&str] = &[
    "creature",
    "dungeon",
    "dungeon_room",
    "encounter",
    "event",
    "faction",
    "magic_item",
    "npc",
    "quest",
    "rumor",
    "settlement",
    "tavern",
    "trap",
    "treasure",
    "wilderness_location",
];

const REQUIRED_TABLES: &[&str] = &[
    "atmosphere",
    "creature_ability",
    "creature_behavior",
    "creature_form",
    "creature_lair",
    "creature_trait",
    "creature_weakness",
    "dungeon_entrance",
    "dungeon_inhabitant",
    "dungeon_origin",
    "dungeon_state",
    "dungeon_type",
    "encounter_activity",
    "encounter_disposition",
    "event_focus",
    "faction_form",
    "faction_goal",
    "faction_method",
    "faction_resource",
    "faction_theme",
    "family_names",
    "given_names",
    "item_base",
    "item_drawback",
    "item_history",
    "item_material",
    "item_power",
    "item_quality",
    "meaning_action",
    "meaning_subject",
    "npc_age",
    "npc_appearance",
    "npc_motivation",
    "npc_occupation",
    "npc_secret",
    "npc_trait",
    "place_adjective",
    "place_noun",
    "quest_complication",
    "quest_goal",
    "quest_patron",
    "quest_reward",
    "quest_stakes",
    "quest_target",
    "room_contents",
    "room_exit",
    "room_feature",
    "room_shape",
    "rumor_claim",
    "rumor_source",
    "rumor_veracity",
    "settlement_feature",
    "settlement_government",
    "settlement_problem",
    "settlement_size",
    "settlement_trade",
    "terrain",
    "tavern_clientele",
    "tavern_specialty",
    "trap_effect",
    "trap_mechanism",
    "trap_severity",
    "trap_sign",
    "trap_trigger",
    "treasure_valuable",
    "wild_danger",
    "wild_feature",
    "wild_landmark",
    "wild_resident",
];

// ---- lint (also unit-tested below so a broken lint cannot pass silently) ----

fn words(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    format!(
        " {} ",
        spaced.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn denylist_terms(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(words)
        .collect()
}

/// Problems with one piece of entry text (D-69, D-70).
fn lint_text(text: &str, denylist: &[String]) -> Vec<String> {
    let mut p = Vec::new();
    let n = text.chars().count();
    if !(3..=80).contains(&n) {
        p.push(format!("length {n} not in 3..=80"));
    }
    if !text.chars().all(|c| (' '..='~').contains(&c)) {
        p.push("non-ASCII or control character".into());
    }
    if text.starts_with(' ') || text.ends_with(' ') {
        p.push("leading or trailing space".into());
    }
    if text.contains("  ") {
        p.push("double space".into());
    }
    if text.contains('{') || text.contains('}') {
        p.push("brace".into());
    }
    if text.ends_with('.') {
        p.push("final period".into());
    }
    let w = words(text);
    for term in denylist {
        if w.contains(term.as_str()) {
            p.push(format!("denylisted term {:?}", term.trim()));
        }
    }
    p
}

/// Denylist check for the text around the entries: name, description, tags,
/// template (QA follow-up on T6.1).
fn lint_metadata(fields: &[(&str, String)], denylist: &[String]) -> Vec<String> {
    let mut p = Vec::new();
    for (field, value) in fields {
        let w = words(value);
        for term in denylist {
            if w.contains(term.as_str()) {
                p.push(format!("{field}: denylisted term {:?}", term.trim()));
            }
        }
    }
    p
}

// ---- helpers ----

fn catalog() -> (Catalog, Vec<oraculum_core::catalog::LoadWarning>) {
    Catalog::build(&builtin::sources(), &[])
}

fn raw(text: &str) -> toml::Table {
    text.parse::<toml::Table>().unwrap()
}

fn header_problems(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut p = Vec::new();
    if !lines
        .first()
        .is_some_and(|l| l.starts_with("# Written for Oraculum."))
    {
        p.push("first line must start with '# Written for Oraculum.'".to_string());
    }
    let comments: Vec<&str> = lines
        .iter()
        .take_while(|l| l.starts_with('#'))
        .copied()
        .collect();
    let joined = comments.join(" ");
    if comments.len() < 2 {
        p.push("header must be at least two comment lines".into());
    }
    if !joined.contains("Author:") {
        p.push("header lacks 'Author:'".into());
    }
    if !joined.contains("Form:") {
        p.push("header lacks 'Form:'".into());
    }
    p
}

fn tag_problems(tags: &[String]) -> Vec<String> {
    let mut p = Vec::new();
    if !(1..=3).contains(&tags.len()) {
        p.push(format!("{} tags, want 1 to 3", tags.len()));
    }
    for t in tags {
        if t.is_empty()
            || !t
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            p.push(format!("tag {t:?} is not a lowercase word"));
        }
    }
    p
}

fn min_items(id: &str) -> usize {
    MIN_ITEMS
        .iter()
        .find(|(i, _)| *i == id)
        .map_or(12, |(_, n)| *n)
}

fn assert_clean(problems: Vec<String>) {
    assert!(
        problems.is_empty(),
        "content problems:\n  {}",
        problems.join("\n  ")
    );
}

// ---- lint unit tests ----

#[test]
fn denylist_is_not_empty_and_catches_known_terms() {
    let list = denylist_terms(DENYLIST);
    assert!(list.len() >= 20, "denylist.txt looks empty or truncated");
    assert!(!lint_text("a wary beholder", &list).is_empty());
    assert!(!lint_text("Mind  Flayer", &list).is_empty());
    assert!(!lint_text("the Forgotten-Realms road", &list).is_empty());
    assert!(
        lint_text("a mythical bridge", &list).is_empty(),
        "whole words only"
    );
}

#[test]
fn metadata_lint_covers_name_description_tags_and_template() {
    let list = denylist_terms(DENYLIST);
    for field in ["name", "description", "tags", "template"] {
        let bad = [(field, "The Beholder and friends".to_string())];
        assert_eq!(lint_metadata(&bad, &list).len(), 1, "{field}");
    }
    let ok = [("name", "A plain name".to_string())];
    assert!(lint_metadata(&ok, &list).is_empty());
}

#[test]
fn lint_rejects_non_ascii_braces_and_shape_errors() {
    let none: Vec<String> = Vec::new();
    assert!(!lint_text("caf\u{e9} owner", &none).is_empty());
    assert!(!lint_text("a {table:x} thing", &none).is_empty());
    assert!(!lint_text("ends with a period.", &none).is_empty());
    assert!(!lint_text("ab", &none).is_empty());
    assert!(!lint_text(&"x".repeat(81), &none).is_empty());
    assert!(!lint_text(" padded", &none).is_empty());
    assert!(!lint_text("two  spaces", &none).is_empty());
    assert!(lint_text("a plain entry", &none).is_empty());
}

#[test]
fn header_and_tag_checks_reject_bad_input() {
    assert!(!header_problems("name = \"x\"\n").is_empty());
    assert!(!header_problems("# Written for Oraculum. Original.\nname = \"x\"\n").is_empty());
    assert!(header_problems(
        "# Written for Oraculum. Original.\n# Author: A. Form: fragment.\nname = \"x\"\n"
    )
    .is_empty());
    assert!(!tag_problems(&[]).is_empty());
    assert!(!tag_problems(&["Bad".into()]).is_empty());
    assert!(tag_problems(&["npc".into(), "name".into()]).is_empty());
}

// ---- the content ----

#[test]
fn catalog_loads_with_no_warnings() {
    let (cat, warnings) = catalog();
    assert!(warnings.is_empty(), "warnings: {warnings:#?}");
    assert_eq!(
        cat.items().len(),
        builtin::TABLES.len() + builtin::GENERATORS.len()
    );
}

#[test]
fn every_table_meets_the_entry_contract() {
    let deny = denylist_terms(DENYLIST);
    let (cat, _) = catalog();
    let mut problems = Vec::new();
    // Tables used with a count above one must not contain commas.
    let mut joined: BTreeSet<String> = BTreeSet::new();
    for (_, text) in builtin::GENERATORS {
        let g = oraculum_core::generator::Generator::from_toml(text, "x")
            .unwrap()
            .0;
        for s in &g.slots {
            if let (SlotSource::Table(t), Count::Dice(_)) = (&s.source, &s.count) {
                joined.insert(t.clone());
            }
            if let (SlotSource::Table(t), Count::Fixed(n)) = (&s.source, &s.count) {
                if *n > 1 {
                    joined.insert(t.clone());
                }
            }
        }
    }
    for (stem, text) in builtin::TABLES {
        let mut at = |m: String| problems.push(format!("table {stem}: {m}"));
        header_problems(text).into_iter().for_each(&mut at);
        let Some(t) = cat.table(stem) else {
            at("not in catalog".into());
            continue;
        };
        if t.id != *stem {
            at(format!("id {:?} differs from file stem", t.id));
        }
        if t.name.trim().is_empty() {
            at("empty name".into());
        }
        let fields = [
            ("name", t.name.clone()),
            ("description", t.description.clone().unwrap_or_default()),
            ("tags", t.tags.join(" ")),
        ];
        lint_metadata(&fields, &deny).into_iter().for_each(&mut at);
        if !CATEGORIES.contains(&t.category.as_str()) {
            at(format!("category {:?} not in the fixed set", t.category));
        }
        tag_problems(&t.tags).into_iter().for_each(&mut at);
        let mut seen = BTreeSet::new();
        for e in &t.entries {
            lint_text(&e.raw, &deny)
                .into_iter()
                .for_each(|m| at(format!("{:?}: {m}", e.raw)));
            if !seen.insert(e.raw.to_lowercase()) {
                at(format!("duplicate entry {:?}", e.raw));
            }
            if joined.contains(*stem) && e.raw.contains(',') {
                at(format!("comma in joined table: {:?}", e.raw));
            }
        }
        match t.form {
            TableForm::List => {
                if t.entries.len() < min_items(stem) {
                    at(format!(
                        "{} items, want at least {}",
                        t.entries.len(),
                        min_items(stem)
                    ));
                }
                if matches!(*stem, "meaning_action" | "meaning_subject") && t.entries.len() != 20 {
                    at("meaning tables must have exactly 20 items".into());
                }
            }
            TableForm::Ranged => {
                if *stem == "event_focus" {
                    if t.entries.len() < 10 {
                        at("event_focus needs at least 10 ranges".into());
                    }
                    let (lo, hi) = (
                        t.entries.first().map(|e| e.lo),
                        t.entries.last().map(|e| e.hi),
                    );
                    if (lo, hi) != (Some(1), Some(100)) {
                        at("event_focus ranges must cover 1-100".into());
                    }
                } else {
                    at("only event_focus is ranged".into());
                }
            }
        }
    }
    assert_clean(problems);
}

#[test]
fn event_focus_is_ranged_d100_and_meaning_tables_are_lists() {
    let (cat, _) = catalog();
    assert_eq!(cat.table("event_focus").unwrap().form, TableForm::Ranged);
    assert_eq!(cat.table("meaning_action").unwrap().form, TableForm::List);
    assert_eq!(cat.table("meaning_subject").unwrap().form, TableForm::List);
}

#[test]
fn every_generator_meets_the_contract() {
    let deny = denylist_terms(DENYLIST);
    let (cat, _) = catalog();
    let mut problems = Vec::new();
    for (stem, text) in builtin::GENERATORS {
        let mut at = |m: String| problems.push(format!("generator {stem}: {m}"));
        header_problems(text).into_iter().for_each(&mut at);
        let Some(g) = cat.generator(stem) else {
            at("not in catalog".into());
            continue;
        };
        if g.description.as_deref().is_none_or(|d| d.trim().is_empty()) {
            at("empty description".into());
        }
        if !CATEGORIES.contains(&g.category.as_str()) {
            at(format!("category {:?} not in the fixed set", g.category));
        }
        tag_problems(&g.tags).into_iter().for_each(&mut at);
        let template = raw(text)
            .get("template")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let fields = [
            ("name", g.name.clone()),
            ("description", g.description.clone().unwrap_or_default()),
            ("tags", g.tags.join(" ")),
            ("template", template.clone()),
        ];
        lint_metadata(&fields, &deny).into_iter().for_each(&mut at);
        for s in &g.slots {
            if !template.contains(&format!("{{{}}}", s.key)) {
                at(format!("template does not use slot {:?}", s.key));
            }
        }
    }
    assert_clean(problems);
}

#[test]
fn no_orphan_tables() {
    let (cat, _) = catalog();
    let mut used = BTreeSet::new();
    for (stem, _) in builtin::GENERATORS {
        for s in &cat.generator(stem).unwrap().slots {
            if let SlotSource::Table(t) = &s.source {
                used.insert(t.clone());
            }
        }
    }
    let orphans: Vec<_> = builtin::TABLES
        .iter()
        .map(|(s, _)| (*s).to_string())
        .filter(|s| !used.contains(s))
        .collect();
    assert!(orphans.is_empty(), "tables no generator uses: {orphans:?}");
}

/// Spec 6.2: every generator renders cleanly across seeds 0..500, and the
/// same seed gives the same result.
#[test]
fn all_generators_render_cleanly_across_seeds() {
    let (cat, _) = catalog();
    let mut problems = Vec::new();
    for (stem, _) in builtin::GENERATORS {
        let g = cat.generator(stem).unwrap();
        for seed in 0..500u64 {
            let out = cat.generate(stem, &mut RngRoller::from_seed(seed)).unwrap();
            let again = cat.generate(stem, &mut RngRoller::from_seed(seed)).unwrap();
            let at = |m: &str| format!("{stem} seed {seed}: {m} in {:?}", out.text);
            if out != again {
                problems.push(at("not deterministic"));
            }
            let t = &out.text;
            if t.trim().is_empty() {
                problems.push(at("empty text"));
            }
            if t.contains('{') || t.contains('}') {
                problems.push(at("brace"));
            }
            if t.contains("  ") {
                problems.push(at("double space"));
            }
            if t != t.trim() {
                problems.push(at("leading or trailing whitespace"));
            }
            for (slot, result) in g.slots.iter().zip(&out.slots) {
                if slot.chance == 100
                    && (!result.present || result.values.iter().all(|v| v.trim().is_empty()))
                {
                    problems.push(at(&format!("slot {:?} has no value", slot.key)));
                }
            }
            if problems.len() > 20 {
                break;
            }
        }
    }
    assert_clean(problems);
}

/// Seeds 1..3 per generator, pinned for review (D-72).
#[test]
fn generator_snapshots() {
    let (cat, _) = catalog();
    let mut by_id: BTreeMap<&str, String> = BTreeMap::new();
    for (stem, _) in builtin::GENERATORS {
        let body: String = (1..=3u64)
            .map(|seed| {
                let out = cat.generate(stem, &mut RngRoller::from_seed(seed)).unwrap();
                format!("seed {seed}: {}\n", out.text)
            })
            .collect();
        by_id.insert(stem, body);
    }
    for (id, body) in by_id {
        insta::assert_snapshot!(format!("gen_{id}"), body);
    }
}

// ---- whole-set requirements (enabled in T6.8) ----

#[test]
fn required_generators_exist() {
    let (cat, warnings) = catalog();
    assert!(warnings.is_empty());
    assert_eq!(REQUIRED_GENERATORS.len(), 15);
    for id in REQUIRED_GENERATORS {
        assert!(cat.generator(id).is_some(), "missing generator {id}");
    }
    assert_eq!(builtin::GENERATORS.len(), 15);
}

#[test]
fn required_tables_exist() {
    let (cat, warnings) = catalog();
    assert!(warnings.is_empty());
    assert_eq!(REQUIRED_TABLES.len(), 69);
    for id in REQUIRED_TABLES {
        assert!(cat.table(id).is_some(), "missing table {id}");
    }
    assert_eq!(builtin::TABLES.len(), 69);
}

/// Partial PRs stay honest: nothing is listed that the plan does not name.
#[test]
fn only_planned_ids_exist() {
    let gens: BTreeSet<&str> = REQUIRED_GENERATORS.iter().copied().collect();
    let tabs: BTreeSet<&str> = REQUIRED_TABLES.iter().copied().collect();
    for (s, _) in builtin::GENERATORS {
        assert!(gens.contains(s), "generator {s} is not in the plan");
    }
    for (s, _) in builtin::TABLES {
        assert!(tabs.contains(s), "table {s} is not in the plan");
    }
}
