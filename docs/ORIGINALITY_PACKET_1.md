# Originality review packet 1 (D-46, D-65)

For the human reviewer. Checkpoint after T6.3: **5 of 15 generators, 25 of 69 tables**. Prepared by QA (Rick). QA cannot prove originality; this lists what was checked and what a person should look at.

Content lives in `content/` (merged by PRs #14, #15, #16). Run any generator yourself: `oraculum --seed 1 gen npc` (replace `npc`; try seeds 1..20).

## What was checked automatically

- Denylist lint (published games, settings, monsters, fiction, brands, nations, faiths): clean on every entry. Limits: whole-word match only; entry text only so far (name/description/tags/template coverage requested, see PR #16).
- Entry contract: 3-80 ASCII characters, no braces, no double spaces, no final period, no duplicates.
- Seed sweep 0..500 for every generator: renders, no empty value, no `{ }`, deterministic.
- Every file starts with the provenance header naming the author (Stack, an AI developer agent). All text was written for this project.

## What QA read and found

QA read every entry of all 25 tables and 6 rendered samples per generator. Findings:

| Item | Finding | Action |
|---|---|---|
| `given_names` entry **Eskel** | Name of a character in a published fiction series | **Fixed** in PR #15 (now Eskarn); also swapped Perrin, Garrow, Lenore, Umber, Marlow for coined variants; all six added to the denylist |
| `family_names` entry **Underhill** | Ordinary English surname, but also a famous fantasy hero's alias | **Fixed** in PR #15 (now Underbank) |
| Table ids `event_focus`, `meaning_action`, `meaning_subject` | Ids are fixed by the spec and resemble terms used by published oracle products. Entries share nothing beyond common English words | **Your call**: accept spec naming? |
| Spelling mixed (British and American) | Style only | Fixed: American throughout |
| Everything else | No proprietary names, no real-world references, no wording matching known published tables that QA could identify | none |

Honest limit: QA recognises well-known products from memory; it cannot search every published table. A short entry such as "a hidden danger surfaces" is generic enough that any similarity to a published line would be coincidence, and cannot be ruled out by a machine.

## What to read (about 20 minutes)

Read in this order. Count is the number of entries. Oracle first, as the plan requires.

| # | Id | Kind | Entries | What to read |
|---|---|---|---|---|
| 1 | `event_focus` | ranged d100 | 12 | All. Uneven ranges on purpose. |
| 2 | `meaning_action` | list | 20 | All (single verbs). |
| 3 | `meaning_subject` | list | 20 | All (noun phrases). |
| 4 | `event` | generator | - | Template only: `{focus}: {action} / {subject}`. |
| 5 | `given_names`, `family_names` | list | 62, 61 | Skim for real or fictional people. |
| 6 | `npc_occupation` | list | 46 | Skim. |
| 7 | `npc_trait`, `npc_appearance`, `npc_motivation`, `npc_age`, `npc_secret` | list | 20, 16, 14, 12, 12 | All short fragments. |
| 8 | `npc` | generator | - | Template + 6 samples. |
| 9 | `rumor_claim`, `rumor_source`, `rumor_veracity` | list | 20, 12, 12 | `rumor_claim` is the longest text in this batch: read all 20. |
| 10 | `rumor` | generator | - | Template + samples. |
| 11 | `trap_trigger`, `trap_mechanism`, `trap_effect`, `trap_sign`, `trap_severity` | list | 12 each | All. |
| 12 | `trap` | generator | - | Template + samples. |
| 13 | `creature_form` | list | 20 | All: invented monster names, check none is a known creature. |
| 14 | `creature_trait`, `_ability`, `_weakness`, `_behavior`, `_lair` | list | 12 each | All. |
| 15 | `creature` | generator | - | Template + samples. |

Rendered samples are pinned in `crates/oraculum-core/tests/snapshots/` (seeds 1-3 per generator).

## Provenance status (`content/PROVENANCE.md`)

| PR | Content | QA reviewed | Human sign-off |
|---|---|---|---|
| #14 T6.1 | `event_focus`, `meaning_*`, `event` | yes (PR #14) | pending |
| #15 T6.2 | names, `npc_*`, `npc` | yes, approved after fixes | pending |
| #16 T6.3 | `rumor_*`, `trap_*`, `creature_*` | yes, approved | pending |

## Decisions needed from you

1. Accept the spec-dictated oracle table ids/names (`event_focus`, `meaning_action`, `meaning_subject`)? If not, say what to rename (needs a spec change).
2. Anything else that feels familiar: list it here and QA adds it to the denylist so it cannot come back.

Sign-off line (T6.8, not now): `content/PROVENANCE.md` > Human sign-off.
