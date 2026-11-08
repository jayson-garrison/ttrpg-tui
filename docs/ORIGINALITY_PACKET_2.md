# Originality review packet 2 (batch 2: T6.4-T6.6)

For the human reviewer (D-46, D-65). Covers PRs #20, #21, #22: **9 more generators' worth of content, 30 tables** (settlement, tavern, wilderness_location, quest, encounter, magic_item, treasure). Running generators: `oraculum --seed 1 gen tavern` (try seeds 1..20). Pinned samples: `crates/oraculum-core/tests/snapshots/`.

## Checked automatically
Denylist lint (entries, names, descriptions, tags, templates), entry contract (3-80 ASCII, no braces, no final period, no duplicates), seed sweep 0..500 per generator, provenance header with author in every file. All clean.

## QA read (every entry of the 30 tables, 28 rendered samples)
Nothing resembling a published table, product, setting or named character was found. Findings are cosmetic only:
- Tavern names such as 'The Iron Hill' read as places rather than inns (shared place nouns).
- `magic_item` rolls base item and material independently ('a wooden bowl / copper wire').
- 'Coins: 14 coins' wording in `treasure`.

Limit: QA recognises products from memory and cannot search all published tables.

## What to read (about 15 minutes)

| Tables | Entries | Read |
|---|---|---|
| `place_adjective`, `place_noun` | 30 + 30 | Skim: do the pairs sound like real places or brands? |
| `terrain`, `atmosphere` | 14, 20 | Skim |
| `settlement_*` (size, government, feature, trade, problem) | 12 each | All |
| `tavern_specialty`, `tavern_clientele` | 12 each | All (food and drink names) |
| `wild_landmark`, `wild_feature`, `wild_danger`, `wild_resident` | 12 each | All |
| `quest_*` (patron, goal, target, complication, reward, stakes) | 12 each | All |
| `encounter_activity`, `encounter_disposition` | 12 each | All |
| `treasure_valuable`, `item_*` (base, material, quality, power, drawback, history) | 12-20 | All; `item_power` is the most inventive text |

## Decision for you
Anything that feels familiar? Name it and QA adds it to the denylist. Record: `content/PROVENANCE.md` > Human sign-off by batch, batch 2.
