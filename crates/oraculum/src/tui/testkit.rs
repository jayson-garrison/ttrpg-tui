//! Shared test fixtures (D-72, D-82): TUI tests use this catalog and never the
//! built-in content.

use oraculum_core::catalog::{Catalog, LoadWarning, Source};
use oraculum_core::clock::FixedClock;
use oraculum_core::engine::Engine;
use oraculum_core::rng::ScriptedRoller;
use std::rc::Rc;

use crate::storage::JournalStore;

fn table(id: &str, category: &str, body: &str) -> Source {
    Source::table(
        "test",
        id,
        &format!("name = \"{id}\"\ncategory = \"{category}\"\n{body}"),
    )
}

fn generator(id: &str, category: &str, body: &str) -> Source {
    Source::generator(
        "test",
        id,
        &format!("name = \"{id}\"\ncategory = \"{category}\"\n{body}"),
    )
}

/// `colors` (list of 3), `loot` (ranged d6), `inner`, `pair` (table, nested
/// generator, dice), `opt` (optional line, D-76), `event`, plus a user-layer
/// `bad_ref` that produces one loader warning.
pub fn fixture_catalog() -> (Catalog, Vec<LoadWarning>) {
    let builtin = vec![
        table(
            "colors",
            "Alpha",
            "tags = [\"hue\"]\nitems = [\"red\", \"green\", \"blue\"]",
        ),
        table(
            "loot",
            "Alpha",
            "dice = \"d6\"\n[[entries]]\nrange = [1, 2]\ntext = \"copper\"\n[[entries]]\nrange = [3, 5]\ntext = \"silver\"\n[[entries]]\nrange = [6, 6]\ntext = \"gold\"",
        ),
        table("words", "Beta", "items = [\"alpha\", \"beta\", \"gamma\"]"),
        generator(
            "inner",
            "Beta",
            "template = \"{w}\"\n[[slots]]\nkey = \"w\"\ntable = \"words\"",
        ),
        generator(
            "pair",
            "Beta",
            "template = \"{a} and {b} ({c})\"\n[[slots]]\nkey = \"a\"\ntable = \"colors\"\n[[slots]]\nkey = \"b\"\ngenerator = \"inner\"\n[[slots]]\nkey = \"c\"\ndice = \"1d6\"",
        ),
        generator(
            "opt",
            "Beta",
            "template = \"Head\\nExtra: {tail}\"\n[[slots]]\nkey = \"tail\"\ntable = \"colors\"\nchance = 50",
        ),
        generator(
            "event",
            "Oracle",
            "template = \"{e}\"\n[[slots]]\nkey = \"e\"\ntable = \"words\"",
        ),
    ];
    let user = vec![generator(
        "bad_ref",
        "Beta",
        "template = \"{x}\"\n[[slots]]\nkey = \"x\"\ntable = \"nowhere\"",
    )];
    Catalog::build(&builtin, &user)
}

/// An engine over the fixture catalog, rolling the scripted values, with a
/// fixed clock.
pub fn engine(script: &[u32]) -> Engine {
    let (catalog, _) = fixture_catalog();
    Engine::new(
        Rc::new(catalog),
        Box::new(ScriptedRoller::new(script.iter().copied())),
        Box::new(FixedClock(1_700_000_000)),
    )
}

/// A journal store in a fresh temp dir; keep the guard alive.
pub fn store() -> (JournalStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap_or_else(|e| unreachable!("temp dir: {e}"));
    (
        JournalStore::new(dir.path().join("sessions").join("t.jsonl")),
        dir,
    )
}

/// An engine over an empty catalog (no `event` generator, so twists are
/// unavailable).
pub fn empty_engine(script: &[u32]) -> Engine {
    let (catalog, _) = Catalog::build(&[], &[]);
    Engine::new(
        Rc::new(catalog),
        Box::new(ScriptedRoller::new(script.iter().copied())),
        Box::new(FixedClock(1_700_000_000)),
    )
}
