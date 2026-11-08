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

## Human sign-off

Reviewer: _pending_ | Date: _pending_ | Commit: _pending_
