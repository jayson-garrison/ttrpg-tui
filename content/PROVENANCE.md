# Provenance and review log

Written for Oraculum, from scratch (D-68). Authored by Stack (Claude, AI developer agent).
QA (Rick) records "reviewed" per PR below. The human sign-off line at the end is the release gate (D-46).

| PR | Content | Reviewed by | Notes |
|---|---|---|---|
| T6.1 | `event_focus`, `meaning_action`, `meaning_subject`, `event` | Rick (QA), PR #14 | Oracle tables read first. Ranges are uneven and cover 1-100 exactly; wording is generic English. Table ids and names (`event_focus`, `meaning_*`) are fixed by the spec and echo terms used by published oracle products; the content itself shares nothing beyond common words. For the human: confirm that is acceptable. Denylist extended; lint covers entries only, not name/description/tags (follow-up). |
| T6.2 | `given_names`, `family_names`, `npc_*` (6), `npc` | Rick (QA), PR #15 | Read all 8 tables and 20 samples; `Eskel` (published-fiction name) replaced, `Underhill` swapped for `Underbank` (flagged for the human); both added to the denylist |
| T6.3 | `rumor` (+3), `trap` (+5), `creature` (+6) | Rick (QA), PR #16 | Read all 14 tables and samples; no issues. Early-packet checkpoint (D-65) |
| T6.4 | places: `terrain`, `atmosphere`, `place_adjective`, `place_noun`, settlement (5), tavern (2), wild (4); `settlement`, `tavern`, `wilderness_location` | pending | first use of D-76 optional lines |
| T6.5 | `quest_*` (6), `encounter_activity`, `encounter_disposition`; `quest`, `encounter` | pending | encounter twist uses `event` at 25% (D-76 line) |
| T6.6 | `treasure_valuable`, `item_*` (6); `magic_item`, `treasure` | pending | nested magic item and drawback on D-76 lines |
| T6.7 | `dungeon_*` (5), `room_*` (4); `dungeon_room`, `dungeon` | pending | deepest nesting dungeon > treasure > magic_item |
| T6.8 | `faction_*` (5), `faction`; required-15 and required-69 tests enabled | pending | complete set: 15 generators, 69 tables |

## Packet listing (complete set)

- 15 generators: creature, dungeon, dungeon_room, encounter, event, faction, magic_item, npc, quest, rumor, settlement, tavern, trap, treasure, wilderness_location.
- 69 tables (ids in `crates/oraculum-core/tests/builtin.rs`, `REQUIRED_TABLES`; the list matches `docs/T6_PLAN.md` section 5).
- Snapshots of seeds 1..3 per generator: `crates/oraculum-core/tests/snapshots/`.
- Automated checks: entry contract, denylist (entries and name/description/tags/template), seed sweep 0..500, determinism, no orphan tables.

## Human sign-off

Reviewer: _pending_ | Date: _pending_ | Commit: _pending_
