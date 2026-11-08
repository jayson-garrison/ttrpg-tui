# Originality review packet 3 (batch 3, final: T6.7-T6.8)

For the human reviewer (D-46, D-65). Covers PRs #23 and #24: `dungeon`, `dungeon_room`, `faction` and 14 tables. With this the built-in set is complete: **15 generators, 69 tables**. This packet is the input to the release sign-off in `content/PROVENANCE.md`.

## Checked automatically
Same lint and sweep as packets 1-2. In addition the full-set tests are enabled: exactly the 15 required generators and 69 required tables exist, no orphan table, zero load warnings, every generator renders for seeds 0..500 deterministically with no braces, double spaces or empty required values. QA also ran 4,500+ renders through the real binary (seeds 1000-1300, all 15 generators): clean.

## QA read (every entry of the 14 tables, samples of both generators)
No resemblance to published tables or settings found. Words like 'restless skeletons', 'giant rats', 'cave spiders', 'a patient guardian' are folklore-level generic terms.

## What to read (about 10 minutes)

| Tables | Entries | Read |
|---|---|---|
| `dungeon_type`, `_origin`, `_state`, `_entrance`, `_inhabitant` | 12 each | All |
| `room_shape`, `room_feature`, `room_contents`, `room_exit` | 12 each | All |
| `faction_form`, `_theme`, `_goal`, `_method`, `_resource` | 12 each | All |

## Release checklist for the sign-off
1. Batches 1 (accepted 2026-11-08), 2 and 3 answered in `content/PROVENANCE.md`.
2. Spelling is American throughout; no real-world nations, faiths or brands.
3. All 15 generators render cleanly: `oraculum gen <id>` for each id listed in the PROVENANCE packet listing.
4. Your final line in `content/PROVENANCE.md` (reviewer, date, commit) is the release gate (D-46).
