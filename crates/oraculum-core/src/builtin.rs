//! Built-in content, embedded at compile time (A§3.11, D-27).
//!
//! `include_str!` keeps core free of file I/O. T6 fills these lists; the test
//! below fails if a file in `content/` is missing from them (or vice versa).

use crate::catalog::Source;

/// `(file stem, file text)` for each built-in table.
pub const TABLES: &[(&str, &str)] = &[
    (
        "atmosphere",
        include_str!("../../../content/tables/atmosphere.toml"),
    ),
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
        "dungeon_entrance",
        include_str!("../../../content/tables/dungeon_entrance.toml"),
    ),
    (
        "dungeon_inhabitant",
        include_str!("../../../content/tables/dungeon_inhabitant.toml"),
    ),
    (
        "dungeon_origin",
        include_str!("../../../content/tables/dungeon_origin.toml"),
    ),
    (
        "dungeon_state",
        include_str!("../../../content/tables/dungeon_state.toml"),
    ),
    (
        "dungeon_type",
        include_str!("../../../content/tables/dungeon_type.toml"),
    ),
    (
        "encounter_activity",
        include_str!("../../../content/tables/encounter_activity.toml"),
    ),
    (
        "encounter_disposition",
        include_str!("../../../content/tables/encounter_disposition.toml"),
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
        "item_base",
        include_str!("../../../content/tables/item_base.toml"),
    ),
    (
        "item_drawback",
        include_str!("../../../content/tables/item_drawback.toml"),
    ),
    (
        "item_history",
        include_str!("../../../content/tables/item_history.toml"),
    ),
    (
        "item_material",
        include_str!("../../../content/tables/item_material.toml"),
    ),
    (
        "item_power",
        include_str!("../../../content/tables/item_power.toml"),
    ),
    (
        "item_quality",
        include_str!("../../../content/tables/item_quality.toml"),
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
        "place_adjective",
        include_str!("../../../content/tables/place_adjective.toml"),
    ),
    (
        "place_noun",
        include_str!("../../../content/tables/place_noun.toml"),
    ),
    (
        "quest_complication",
        include_str!("../../../content/tables/quest_complication.toml"),
    ),
    (
        "quest_goal",
        include_str!("../../../content/tables/quest_goal.toml"),
    ),
    (
        "quest_patron",
        include_str!("../../../content/tables/quest_patron.toml"),
    ),
    (
        "quest_reward",
        include_str!("../../../content/tables/quest_reward.toml"),
    ),
    (
        "quest_stakes",
        include_str!("../../../content/tables/quest_stakes.toml"),
    ),
    (
        "quest_target",
        include_str!("../../../content/tables/quest_target.toml"),
    ),
    (
        "room_contents",
        include_str!("../../../content/tables/room_contents.toml"),
    ),
    (
        "room_exit",
        include_str!("../../../content/tables/room_exit.toml"),
    ),
    (
        "room_feature",
        include_str!("../../../content/tables/room_feature.toml"),
    ),
    (
        "room_shape",
        include_str!("../../../content/tables/room_shape.toml"),
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
        "settlement_feature",
        include_str!("../../../content/tables/settlement_feature.toml"),
    ),
    (
        "settlement_government",
        include_str!("../../../content/tables/settlement_government.toml"),
    ),
    (
        "settlement_problem",
        include_str!("../../../content/tables/settlement_problem.toml"),
    ),
    (
        "settlement_size",
        include_str!("../../../content/tables/settlement_size.toml"),
    ),
    (
        "settlement_trade",
        include_str!("../../../content/tables/settlement_trade.toml"),
    ),
    (
        "tavern_clientele",
        include_str!("../../../content/tables/tavern_clientele.toml"),
    ),
    (
        "tavern_specialty",
        include_str!("../../../content/tables/tavern_specialty.toml"),
    ),
    (
        "terrain",
        include_str!("../../../content/tables/terrain.toml"),
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
    (
        "treasure_valuable",
        include_str!("../../../content/tables/treasure_valuable.toml"),
    ),
    (
        "wild_danger",
        include_str!("../../../content/tables/wild_danger.toml"),
    ),
    (
        "wild_feature",
        include_str!("../../../content/tables/wild_feature.toml"),
    ),
    (
        "wild_landmark",
        include_str!("../../../content/tables/wild_landmark.toml"),
    ),
    (
        "wild_resident",
        include_str!("../../../content/tables/wild_resident.toml"),
    ),
];

/// `(file stem, file text)` for each built-in generator.
pub const GENERATORS: &[(&str, &str)] = &[
    (
        "creature",
        include_str!("../../../content/generators/creature.toml"),
    ),
    (
        "dungeon",
        include_str!("../../../content/generators/dungeon.toml"),
    ),
    (
        "dungeon_room",
        include_str!("../../../content/generators/dungeon_room.toml"),
    ),
    (
        "encounter",
        include_str!("../../../content/generators/encounter.toml"),
    ),
    (
        "event",
        include_str!("../../../content/generators/event.toml"),
    ),
    (
        "magic_item",
        include_str!("../../../content/generators/magic_item.toml"),
    ),
    ("npc", include_str!("../../../content/generators/npc.toml")),
    (
        "quest",
        include_str!("../../../content/generators/quest.toml"),
    ),
    (
        "rumor",
        include_str!("../../../content/generators/rumor.toml"),
    ),
    (
        "settlement",
        include_str!("../../../content/generators/settlement.toml"),
    ),
    (
        "tavern",
        include_str!("../../../content/generators/tavern.toml"),
    ),
    (
        "trap",
        include_str!("../../../content/generators/trap.toml"),
    ),
    (
        "treasure",
        include_str!("../../../content/generators/treasure.toml"),
    ),
    (
        "wilderness_location",
        include_str!("../../../content/generators/wilderness_location.toml"),
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
