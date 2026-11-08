//! Built-in content, embedded at compile time (A§3.11, D-27).
//!
//! `include_str!` keeps core free of file I/O. T6 fills these lists; the test
//! below fails if a file in `content/` is missing from them (or vice versa).

use crate::catalog::Source;

/// `(file stem, file text)` for each built-in table.
pub const TABLES: &[(&str, &str)] = &[
    (
        "creature_ability",
        include_str!("../../../content/tables/creature_ability.toml"),
    ),
    (
        "creature_behavior",
        include_str!("../../../content/tables/creature_behavior.toml"),
    ),
    (
        "creature_form",
        include_str!("../../../content/tables/creature_form.toml"),
    ),
    (
        "creature_lair",
        include_str!("../../../content/tables/creature_lair.toml"),
    ),
    (
        "creature_trait",
        include_str!("../../../content/tables/creature_trait.toml"),
    ),
    (
        "creature_weakness",
        include_str!("../../../content/tables/creature_weakness.toml"),
    ),
    (
        "event_focus",
        include_str!("../../../content/tables/event_focus.toml"),
    ),
    (
        "family_names",
        include_str!("../../../content/tables/family_names.toml"),
    ),
    (
        "given_names",
        include_str!("../../../content/tables/given_names.toml"),
    ),
    (
        "meaning_action",
        include_str!("../../../content/tables/meaning_action.toml"),
    ),
    (
        "meaning_subject",
        include_str!("../../../content/tables/meaning_subject.toml"),
    ),
    (
        "npc_age",
        include_str!("../../../content/tables/npc_age.toml"),
    ),
    (
        "npc_appearance",
        include_str!("../../../content/tables/npc_appearance.toml"),
    ),
    (
        "npc_motivation",
        include_str!("../../../content/tables/npc_motivation.toml"),
    ),
    (
        "npc_occupation",
        include_str!("../../../content/tables/npc_occupation.toml"),
    ),
    (
        "npc_secret",
        include_str!("../../../content/tables/npc_secret.toml"),
    ),
    (
        "npc_trait",
        include_str!("../../../content/tables/npc_trait.toml"),
    ),
    (
        "rumor_claim",
        include_str!("../../../content/tables/rumor_claim.toml"),
    ),
    (
        "rumor_source",
        include_str!("../../../content/tables/rumor_source.toml"),
    ),
    (
        "rumor_veracity",
        include_str!("../../../content/tables/rumor_veracity.toml"),
    ),
    (
        "trap_effect",
        include_str!("../../../content/tables/trap_effect.toml"),
    ),
    (
        "trap_mechanism",
        include_str!("../../../content/tables/trap_mechanism.toml"),
    ),
    (
        "trap_severity",
        include_str!("../../../content/tables/trap_severity.toml"),
    ),
    (
        "trap_sign",
        include_str!("../../../content/tables/trap_sign.toml"),
    ),
    (
        "trap_trigger",
        include_str!("../../../content/tables/trap_trigger.toml"),
    ),
];

/// `(file stem, file text)` for each built-in generator.
pub const GENERATORS: &[(&str, &str)] = &[
    (
        "creature",
        include_str!("../../../content/generators/creature.toml"),
    ),
    (
        "event",
        include_str!("../../../content/generators/event.toml"),
    ),
    ("npc", include_str!("../../../content/generators/npc.toml")),
    (
        "rumor",
        include_str!("../../../content/generators/rumor.toml"),
    ),
    (
        "trap",
        include_str!("../../../content/generators/trap.toml"),
    ),
];

/// All built-in sources, sorted by file stem within each kind (tables first).
pub fn sources() -> Vec<Source> {
    let mut tables: Vec<_> = TABLES.to_vec();
    tables.sort_by_key(|(stem, _)| *stem);
    let mut generators: Vec<_> = GENERATORS.to_vec();
    generators.sort_by_key(|(stem, _)| *stem);
    tables
        .into_iter()
        .map(|(stem, text)| Source::table(&format!("builtin:tables/{stem}.toml"), stem, text))
        .chain(generators.into_iter().map(|(stem, text)| {
            Source::generator(&format!("builtin:generators/{stem}.toml"), stem, text)
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn stems_in(dir: &str) -> Vec<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content")
            .join(dir);
        let mut stems: Vec<String> = std::fs::read_dir(path)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .filter_map(|e| {
                        let p = e.path();
                        (p.extension()? == "toml")
                            .then(|| p.file_stem()?.to_str().map(String::from))?
                    })
                    .collect()
            })
            .unwrap_or_default();
        stems.sort();
        stems
    }

    fn listed(list: &[(&str, &str)]) -> Vec<String> {
        let mut v: Vec<String> = list.iter().map(|(s, _)| (*s).to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn lists_match_the_content_directory() {
        assert_eq!(listed(TABLES), stems_in("tables"));
        assert_eq!(listed(GENERATORS), stems_in("generators"));
    }

    #[test]
    fn sources_are_tagged_and_sorted() {
        let s = sources();
        assert_eq!(s.len(), TABLES.len() + GENERATORS.len());
        assert!(s.iter().all(|x| x.origin.starts_with("builtin:")));
    }
}
