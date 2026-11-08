# T6 Built-in Content Plan

Status: plan for Stack (developer) and Rick (QA). Sources: `DESIGN.md` §3, §6, §5.4-5.5 ("Spec"); `ARCHITECTURE.md` §3.11, §7, §13 ("A"); `DECISIONS.md` D-21, D-27, D-46, D-63, D-65. Plan decisions are D-68..D-73 in `DECISIONS.md`.

Skills used: grill-with-docs-auto was not loadable ("Unknown skill"); the grilling and domain-modeling skills were used instead and the grilling answers are the decisions below.

## 1. Scope

15 generators and 69 tables as `.toml` files under `content/`, embedded through `builtin.rs` (A§3.11), plus the test harness that proves them. No new engine features: if a content need cannot be met with the frozen formats, stop and raise it (do not extend the format inside a content PR).

Out of scope: user-facing authoring guide (T10), TUI work, the human's sign-off itself (D-46).

## 2. File layout

```
content/
  README.md                 authoring rules (copy of section 4 and 5 below, short)
  PROVENANCE.md             per-PR review log; final human sign-off line (D-46)
  tables/<id>.toml          one file per table, file stem == id
  generators/<id>.toml      one file per generator, file stem == id
crates/oraculum-core/src/builtin.rs      TABLES / GENERATORS lists (A§3.11), alphabetical, one line per file
crates/oraculum-core/tests/builtin.rs    the content tests (section 6)
crates/oraculum-core/tests/data/denylist.txt   originality/tone lint terms (section 4)
crates/oraculum-core/tests/snapshots/    insta snapshots: seeds 1..3 per generator (D-72)
```

Every file starts with this header (the test checks the first line and that the next lines are comments):

```toml
# Written for Oraculum. Original: not copied or adapted from any published table,
# game, oracle or setting. Author: <name>. Form: <noun phrase|clause|name|fragment>, lowercase, no final period.
```

`builtin.rs` is edited by every PR; keep it alphabetical with one entry per line so merge conflicts are trivial to resolve.

## 3. Required generators (Spec §6.1)

All 15 exist and are built from the tables in section 5. Category and slot recipe are exactly A§7 and Spec §6.1; this plan adds only the contracts below.

| id | category | notes beyond the spec |
|---|---|---|
| `event` | Oracle | template `{focus}: {action} / {subject}`; used by oracle twists and `encounter` |
| `npc` | Characters | `npc_trait` slot has `count = 2` (distinct); `npc_secret` chance 30 |
| `rumor` | Adventure | leaf generator (no nested generators) |
| `trap` | Sites | leaf generator |
| `creature` | Creatures | leaf generator |
| `treasure` | Items | `coins` is a dice slot `4d6`, template "{coins} coins" (D-73); `treasure_valuable` count `1d2` |
| `magic_item` | Items | `item_drawback` chance 40 |
| `dungeon_room` | Sites | `room_exit` count `1d3` |
| `dungeon` | Sites | rooms slot: generator `dungeon_room`, count `1d4+3`, `list = true` |
| `tavern` | Places | `rumor` slot chance 60 |
| `settlement` | Places | leader via `npc` |
| `wilderness_location` | Places | `wild_resident` chance 50 |
| `encounter` | Adventure | `twist` via `event`, chance 25 |
| `quest` | Adventure | location name from `place_adjective` + `place_noun` in two slots |
| `faction` | Organizations | leader via `npc` |

Deepest nesting is `dungeon` => `treasure` => `magic_item` (depth 2), far below the limit of 5.

## 4. Originality and tone rules (Spec §3, D-46)

These are rules for the author, enforced where a machine can enforce them and by the human sign-off where it cannot.

1. **Write from scratch.** Do not open, recall-and-reword, or paraphrase any published random table, oracle, bestiary, adventure generator or setting book. No SRD, Creative Commons or other licensed text either, even where the licence allows it: it would add attribution duties the project does not carry (D-68).
2. **No proprietary names.** No monster, spell, deity, place, class or product names from published games or fiction (examples for the denylist: beholder, mind flayer, owlbear, Forgotten Realms, Greyhawk, Mythic, Ironsworn, Pathfinder, Tolkien terms). Common folklore creatures (dragon, goblin, troll) are allowed as generic words but must not carry published stat or lore wording.
3. **Oracle tables get extra care.** `event_focus`, `meaning_action` and `meaning_subject` are the most likely to resemble existing oracle products. Use your own categories and word choices; `event_focus` ranges must be written for this project (uneven widths on purpose), not mapped from any known d100 table. Rick reviews these three first.
4. **Setting-neutral and safe.** No real-world nations, religions, peoples, living or famous persons, or brands. No slurs. Violence is allowed in the tone of a general adventure game; no sexual content. Names (`given_names`, `family_names`) are invented or blended, pronounceable, and not tied to one real culture; no famous fictional characters.
5. **Provenance header** in every file (section 2). The author name is part of the audit trail.
6. **Lint, not proof.** `denylist.txt` catches known risks (rules 2 and 4); passing it does not establish originality. The human reviews the packet (section 7).

## 5. Table inventory (69)

Shared tables are used by more than one generator. Count column is the minimum number of entries; the target is 20 (a clean d20) unless noted. Ranged tables are marked R. Format of every other table: list table, `items = [...]`.

| # | id | category | min | used by | note |
|---|---|---|---|---|---|
| 1 | `given_names` | Characters | 60 | npc | shared; name, capitalised |
| 2 | `family_names` | Characters | 60 | npc | shared; name, capitalised |
| 3 | `npc_age` | Characters | 12 | npc | |
| 4 | `npc_occupation` | Characters | 40 | npc | |
| 5 | `npc_appearance` | Characters | 12 | npc | |
| 6 | `npc_trait` | Characters | 20 | npc | no commas (used twice, joined) |
| 7 | `npc_motivation` | Characters | 12 | npc | |
| 8 | `npc_secret` | Characters | 12 | npc | |
| 9 | `creature_form` | Creatures | 20 | creature, encounter | shared; bare noun phrase with article |
| 10 | `creature_trait` | Creatures | 12 | creature | |
| 11 | `creature_ability` | Creatures | 12 | creature | |
| 12 | `creature_weakness` | Creatures | 12 | creature | |
| 13 | `creature_behavior` | Creatures | 12 | creature | |
| 14 | `creature_lair` | Creatures | 12 | creature | |
| 15 | `terrain` | Places | 12 | encounter, wilderness_location | shared |
| 16 | `atmosphere` | Places | 20 | tavern, dungeon_room | shared; adjective phrase |
| 17 | `place_adjective` | Places | 30 | settlement, tavern, quest | shared; first word of a name |
| 18 | `place_noun` | Places | 30 | settlement, tavern, quest | shared; second word of a name |
| 19 | `encounter_activity` | Adventure | 12 | encounter | |
| 20 | `encounter_disposition` | Adventure | 12 | encounter | |
| 21 | `settlement_size` | Places | 12 | settlement | |
| 22 | `settlement_government` | Places | 12 | settlement | |
| 23 | `settlement_feature` | Places | 12 | settlement | |
| 24 | `settlement_trade` | Places | 12 | settlement | |
| 25 | `settlement_problem` | Places | 12 | settlement | |
| 26 | `tavern_specialty` | Places | 12 | tavern | |
| 27 | `tavern_clientele` | Places | 12 | tavern | |
| 28 | `wild_landmark` | Places | 12 | wilderness_location | |
| 29 | `wild_feature` | Places | 12 | wilderness_location | |
| 30 | `wild_danger` | Places | 12 | wilderness_location | |
| 31 | `wild_resident` | Places | 12 | wilderness_location | |
| 32 | `dungeon_type` | Sites | 12 | dungeon | |
| 33 | `dungeon_origin` | Sites | 12 | dungeon | |
| 34 | `dungeon_state` | Sites | 12 | dungeon | |
| 35 | `dungeon_entrance` | Sites | 12 | dungeon | |
| 36 | `dungeon_inhabitant` | Sites | 12 | dungeon | count 2, no commas |
| 37 | `room_shape` | Sites | 12 | dungeon_room | |
| 38 | `room_feature` | Sites | 12 | dungeon_room | |
| 39 | `room_contents` | Sites | 12 | dungeon_room | |
| 40 | `room_exit` | Sites | 12 | dungeon_room | count 1d3, no commas |
| 41 | `trap_trigger` | Sites | 12 | trap | |
| 42 | `trap_mechanism` | Sites | 12 | trap | |
| 43 | `trap_effect` | Sites | 12 | trap | |
| 44 | `trap_sign` | Sites | 12 | trap | |
| 45 | `trap_severity` | Sites | 12 | trap | |
| 46 | `quest_patron` | Adventure | 12 | quest | |
| 47 | `quest_goal` | Adventure | 12 | quest | |
| 48 | `quest_target` | Adventure | 12 | quest | |
| 49 | `quest_complication` | Adventure | 12 | quest | |
| 50 | `quest_reward` | Adventure | 12 | quest | |
| 51 | `quest_stakes` | Adventure | 12 | quest | |
| 52 | `rumor_source` | Adventure | 12 | rumor | |
| 53 | `rumor_claim` | Adventure | 20 | rumor | |
| 54 | `rumor_veracity` | Adventure | 12 | rumor | |
| 55 | `treasure_valuable` | Items | 12 | treasure | count 1d2, no commas |
| 56 | `item_base` | Items | 20 | magic_item | |
| 57 | `item_material` | Items | 12 | magic_item | |
| 58 | `item_quality` | Items | 12 | magic_item | |
| 59 | `item_power` | Items | 20 | magic_item | |
| 60 | `item_drawback` | Items | 12 | magic_item | |
| 61 | `item_history` | Items | 12 | magic_item | |
| 62 | `faction_form` | Organizations | 12 | faction | |
| 63 | `faction_theme` | Organizations | 12 | faction | |
| 64 | `faction_goal` | Organizations | 12 | faction | |
| 65 | `faction_method` | Organizations | 12 | faction | |
| 66 | `faction_resource` | Organizations | 12 | faction | |
| 67 | `event_focus` | Oracle | 10 ranges | event | R, `d100`, ranges cover 1-100 exactly |
| 68 | `meaning_action` | Oracle | exactly 20 | event | Spec says d20 |
| 69 | `meaning_subject` | Oracle | exactly 20 | event | Spec says d20 |

Total about 1,500 entries. This list is the source of truth: the test `required_tables_exist` holds the same 69 ids; adding or dropping one means editing this plan, the test, and `builtin.rs` in the same PR.

Entry contract (checked by tests where possible, D-69):
- Text is 3 to 80 ASCII characters, no leading or trailing space, no double space, no `{` or `}` (D-70), no duplicates within a table (case-insensitive), no final period.
- Templates never add articles. An entry that needs one carries it ("a rusted lantern"); plural entries are fine.
- Grammar form per table is declared in its header (`Form:`) so a generator author can write a template that reads correctly. Templates are punctuated by the generator; entries are fragments.
- Tables used with `count > 1` contain no commas (the default join is `", "`).
- Each table's `category` is from the fixed set: Characters, Creatures, Places, Sites, Adventure, Items, Organizations, Oracle. `tags`: 1 to 3 lowercase words, including the owning generator's theme (e.g. `npc`, `trap`).
- Every table is referenced by at least one generator (no orphans).

## 6. Acceptance (tests in `tests/builtin.rs`)

Per table: parses with no warning; header present; entry contract above; list tables have at least the minimum from section 5 (spec floor 12); `meaning_*` have exactly 20 items; `event_focus` is ranged on `d100` with at least 10 entries; category and tags valid; denylist clean.

Per generator: declared in `GENERATORS`; description non-empty; template uses every slot; every generator passes `Catalog::build` with zero warnings.

Whole set:
1. `Catalog::build(builtin::sources(), [])` returns zero warnings and exactly 15 generators and 69 tables.
2. `builtin.rs` lists equal the files in `content/` (D-27, no forgotten file).
3. **All generators render across seeds 0..500** (Spec §6.2): for each generator and seed, text is non-empty; every slot with `chance = 100` has at least one non-empty value; the rendered text contains no `{` or `}`, no double space, no leading or trailing whitespace.
4. Same seed gives the same result (run twice).
5. No orphan table, and no table referenced by only a later PR's generator.
6. Insta snapshot per generator for seeds 1..3 (D-72); this is also Rick's review material.
7. `required_generators_exist` (15) and `required_tables_exist` (69) are `#[ignore = "enabled in T6.8"]` until the final PR, so partial PRs stay green.
8. Denylist negative test: a unit test feeds the lint a table containing a denylisted term (and one with a non-ASCII character and one with `{`) and asserts the lint rejects each, so a broken or empty `denylist.txt` cannot pass silently; it also asserts the denylist file is non-empty.

Rick's checks per PR (manual, in the PR): read 20 rendered samples per new generator (`oraculum gen <id> --seed N` once T5 exists, else the snapshots); check grammar, tone, and that nothing resembles a published product; record "reviewed" with PR number in `content/PROVENANCE.md`.

## 7. Split into pull requests

Each PR is self-contained: every generator lands with every table and nested generator it needs, so the Catalog loads with zero warnings in every PR (a missing reference would otherwise drop the generator). Target 0.5 to 1 day each, about 5 d in total. PRs T6.2 to T6.7 may be done in parallel after T6.1 once the dependency in the last column is merged.

| PR | Contents | Generators (running total) | Needs |
|---|---|---|---|
| T6.1 | Harness (`tests/builtin.rs`, denylist, snapshot setup, `builtin.rs` skeleton, `content/README.md`, `PROVENANCE.md` skeleton); oracle: `event_focus`, `meaning_action`, `meaning_subject`, `event` | `event` (1) | T4 |
| T6.2 | Names and NPC: `given_names`, `family_names`, `npc_*` (6), `npc` | `npc` (2) | T6.1 |
| T6.3 | Small leaf generators: `rumor` (+3 tables), `trap` (+5), `creature` (+6 incl. `creature_form`) | `rumor`, `trap`, `creature` (5) | T6.1 |
| | **Checkpoint (D-65):** Rick assembles the early originality packet for the human: all files so far plus snapshots. Authors keep working on T6.4 onward meanwhile. | | |
| T6.4 | Places: `terrain`, `atmosphere`, `place_adjective`, `place_noun`, settlement (5), tavern (2), wild (4); `settlement`, `tavern`, `wilderness_location` | +3 (8) | T6.2, T6.3 |
| T6.5 | Adventure: `quest_*` (6), `encounter_activity`, `encounter_disposition`; `quest`, `encounter` | +2 (10) | T6.1, T6.3, T6.4 |
| T6.6 | Items: `treasure_valuable`, `item_*` (6); `magic_item`, `treasure` | +2 (12) | T6.1 |
| T6.7 | Sites: `dungeon_*` (5), `room_*` (4); `dungeon_room`, `dungeon` | +2 (14) | T6.3, T6.6 |
| T6.8 | Organizations: `faction_*` (5), `faction`; remove the `#[ignore]` on the required-15 and required-69 tests; final `PROVENANCE.md` packet listing; QA runs the full seed sweep | +1 (15) | all |

After T6.8, the human's sign-off line in `content/PROVENANCE.md` (reviewer, date, commit) is the release gate (D-46); it is not a PR dependency.

Tables per PR: T6.1 3, T6.2 8, T6.3 14, T6.4 15, T6.5 8, T6.6 7, T6.7 9, T6.8 5; total 69.

## 8. Risks and coordination

- **Churn in T5/T7-T9 tests:** any test that pins built-in output (CLI snapshots, TUI render tests) changes whenever content changes. Those tests must use fixture catalogs (`Catalog::build` over test sources), never built-in ids; only `tests/builtin.rs` snapshots pin built-in output (D-72). Stack to confirm for T5.
- **Dice slot grammar:** the dice grammar has no multiplication (checked in `dice/parse.rs`), so `treasure.coins` is a plain `4d6` slot and the template says "{coins} coins" (D-73). No format change is needed.
- **Authoring volume:** about 1,500 entries is the real cost of T6. The 5 d estimate assumes an author who writes fast and QA that samples rather than reads everything; the human reads the packet.
- **Merge conflicts in `builtin.rs`:** accepted, alphabetical single-line entries (D-27).
- **Originality remains a human judgement.** The denylist is a safety net only.
