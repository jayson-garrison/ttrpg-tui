# Oraculum: Initial Design Specification

A terminal (ratatui) utility for solo and GMless tabletop RPG play. It answers yes/no questions with an oracle, rolls dice, and rolls on any table or generator from one searchable library. Everything it produces is logged to a per-session journal.

This document is the source of truth for what to build. It says nothing about schedule.

## 1. Goals

- Ask a yes/no question at a chosen likelihood and get an outcome, with a random "twist" event when the dice call for one.
- Browse and roll **every** table and generator from one place (the Library), including ones the user adds.
- Provide at least ten essential generators (NPC, settlement, dungeon, quest, and so on) built by combining multiple tables.
- Roll dice from expressions, including keep-highest/lowest.
- Keep a journal of every roll, with notes and markdown export.
- Be fully deterministic under a seed, so every behavior is testable.

## 2. Rust crates

| crate | use |
|---|---|
| `ratatui`, `crossterm` | TUI rendering and terminal/key input |
| `clap` (derive) | CLI |
| `serde`, `serde_json` | journal entries (JSON Lines), structured results |
| `toml` | table and generator definition files |
| `rand`, `rand_chacha` | seedable RNG (`ChaCha8Rng`) |
| `thiserror` | typed errors in library code |
| `anyhow` | error handling in the binary crate only |
| `directories` | platform data directory |

Dev-only: `proptest` (properties), `assert_cmd` (CLI tests), `tempfile` (temp dirs), `insta` (snapshots).

No other dependencies without a recorded reason in `DECISIONS.md`.

## 3. Hard constraints

- Pure Rust, CPU only. No C dependencies (no `rusqlite`, `git2`, `openssl`). No GPU.
- Builds and runs on macOS (Apple Silicon) and Linux.
- No async runtime. No network access at runtime.
- `oraculum-core` performs no terminal I/O and no file I/O. It takes `&str` and returns values.
- No `unwrap()`/`expect()` in non-test code. Library code returns typed errors.
- All randomness goes through the `Roller` trait; all time through a `Clock` trait.
- The terminal is restored on exit and on panic.
- Built-in content (starter tables and generators) is original. It must not copy text from published games, tables, or oracle products. Users supply their own tables as files.
- Broken user files never crash the app; they produce warnings.
- Quality gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Where this spec is ambiguous, choose the simplest reading and record it in `DECISIONS.md`.

## 4. Architecture

```
oraculum/
  Cargo.toml                 # workspace
  crates/
    oraculum-core/           # dice, oracle, tables, generators, journal model (pure logic)
    oraculum/                # binary: CLI, file storage, TUI
  content/                   # built-in tables and generators (TOML), embedded with include_str!
    tables/*.toml
    generators/*.toml
```

Each TUI tab is its own module with a state struct, `update(&mut self, KeyEvent) -> Action`, and `render(&self, &mut Frame, Rect)`. `Action` is an enum handled by the top-level app (switch tab, quit, append journal entry, show toast, open item in Library). Tabs never read each other's state.

## 5. Core mechanics

### 5.1 Randomness

```rust
pub trait Roller {
    /// Uniform integer in 1..=sides, sides >= 2.
    fn roll(&mut self, sides: u32) -> u32;
}
```

`RngRoller` wraps a seedable `ChaCha8Rng`. `ScriptedRoller` returns a fixed sequence and is used by tests that assert exact values. With the same seed and the same loaded content, any command produces identical output.

### 5.2 Dice expressions

Grammar (case-insensitive, whitespace ignored):

```
expr = term (("+" | "-") term)*
term = dice | integer
dice = [N] "d" M [("kh" | "kl") [K]]  |  "d66"
```

- `NdM` rolls N dice of M sides (N defaults to 1). `kh K` keeps the K highest, `kl K` keeps the K lowest (K defaults to 1).
- `d66` rolls two d6 and reads them as tens and units, giving 11-66.
- Limits: 1 ≤ N ≤ 100, 2 ≤ M ≤ 1000, 1 ≤ K ≤ N. Violations are typed errors.
- Results carry the raw dice, which were kept or dropped, per-term subtotals, and the total. Example display: `3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5`.
- Boon/bane-style rolls are simply `2d20kl1` and `2d20kh1`.

### 5.3 Oracle

A d100 against a yes-chance set by likelihood:

| likelihood | yes-chance | exceptional yes | exceptional no |
|---|---|---|---|
| Very unlikely | 10 | 1-2 | 83-100 |
| Unlikely | 30 | 1-6 | 87-100 |
| Even | 50 | 1-10 | 91-100 |
| Likely | 70 | 1-14 | 95-100 |
| Very likely | 90 | 1-18 | 99-100 |

Algorithm for `roll` in 1..=100 and `chance`:

1. Yes if `roll <= chance`, otherwise No.
2. The result is exceptional if it falls in the outer fifth of its side: for Yes, `roll <= chance / 5`; for No, `roll > 100 - (100 - chance) / 5` (integer division).
3. Outcomes: `ExceptionalYes`, `Yes`, `No`, `ExceptionalNo`.
4. **Twist:** if `roll` is a double (11, 22, ..., 99) the answer carries a twist; 100 is not a double. That is a 9% chance per question, independent of the answer. When a twist occurs, the oracle rolls the `event` generator (id configurable) and shows the result next to the answer. If the generator or one of its tables is missing, the twist says what is missing rather than failing.

Worked examples (all must be unit tests): Even/10 → ExceptionalYes; Even/11 → Yes with twist; Even/50 → Yes; Even/51 → No; Even/90 → No; Even/91 → ExceptionalNo; Very likely/18 → ExceptionalYes; Very likely/99 → ExceptionalNo with twist; Very unlikely/2 → ExceptionalYes; Very unlikely/83 → ExceptionalNo.

### 5.4 Tables

A table maps a dice roll to a text entry. Two file forms:

```toml
# Ranged table
id = "event_focus"          # optional; defaults to the file stem
name = "Event Focus"
category = "Oracle"
tags = ["oracle", "event"]
dice = "d100"               # any dice expression, or d66
[[entries]]
range = [1, 20]             # inclusive; single value is [5, 5]
text = "A figure from the past appears"
```

```toml
# List table: uniform choice, dice is derived as 1dN (2 <= N <= 1000)
id = "npc_trait"
name = "NPC Traits"
category = "Characters"
items = ["Fussy about details", "Quick to laugh", "..."]
```

- Ranged tables must cover exactly the possible totals of their dice with no gaps and no overlaps. `d66` is valid only for tens/units digits 1-6. Violations produce typed errors naming the bad entry or value.
- **Inline references** in entry text, resolved when the entry is rolled:
  - `{table:other_id}` rolls the other table and substitutes the text.
  - `{roll:2d6}` evaluates a dice expression and substitutes the total.
  - `{{` and `}}` produce literal braces.
- Recursion depth is limited to 5. At load time, table reference graphs are checked for cycles; a table in a cycle is excluded with a warning.
- Rolling a table returns the dice detail, the matched entry, and the final resolved text.

### 5.5 Generators

A generator builds a result from several tables (or other generators) and assembles it with a template.

```toml
id = "tavern"
name = "Tavern"
category = "Places"
description = "A place to eat, drink, and hear things."
tags = ["place", "social"]
template = """
{name_a} {name_b}: {atmosphere}. Run by {proprietor}.
Known for {specialty}. Patrons: {clientele}.
Rumor: {rumor}"""

[[slots]]
key = "name_a"
table = "place_adjective"

[[slots]]
key = "name_b"
table = "place_noun"

[[slots]]
key = "proprietor"
generator = "npc"           # nested generator, rendered inline

[[slots]]
key = "rumor"
generator = "rumor"
chance = 60                 # included with 60% probability, else empty
```

Slot fields:

| field | meaning |
|---|---|
| `key` | placeholder name used in the template (`{key}`) |
| exactly one of `table`, `generator`, `dice` | the source: roll a table, run a generator, or evaluate a dice expression |
| `count` | integer or dice expression; how many results to produce (default 1) |
| `distinct` | for table slots with count > 1, re-roll duplicates (up to 10 attempts, then accept fewer). Default true |
| `join` | separator for multiple results (default `", "`) |
| `list` | if true, multiple results render as separate bulleted lines instead of joined |
| `chance` | percent (1-100) chance the slot is present; otherwise it renders empty. Default 100 |

Evaluation:

- Slots resolve in declaration order and are independent of one another.
- Templates substitute `{key}`; a template may also use `{roll:expr}` and `{{`/`}}`. A template line whose placeholders all render empty (for example an absent optional slot) is dropped with its newline (D-76).
- Nested generators have a depth limit of 5 and are checked for cycles at load time.
- Load-time validation: every referenced table/generator exists; every template placeholder has a slot; unused slots produce a warning; invalid dice expressions are errors. A generator that fails validation is excluded with a warning.
- The result is structured: `{ id, text, slots: [{ key, source, values, detail }] }`. This lets the UI re-roll a single slot and re-render the template without re-rolling the rest.

### 5.6 Journal

Entry: `{ ts: u64 (epoch seconds), kind: "oracle" | "table" | "generator" | "dice" | "note", input, result, detail }`.

Stored as JSON Lines in `<data-dir>/sessions/<session>.jsonl`, one write per appended entry. Loading skips malformed lines and reports them. Every roll in the TUI or CLI is appended automatically. Markdown export is a pure function of the entries: a heading with the session name, then one entry per bullet or paragraph with UTC time (`YYYY-MM-DD HH:MM`), kind, input and result; notes are plain paragraphs.

### 5.7 Content loading

1. Embedded built-in tables and generators are loaded first.
2. User files from `<data-dir>/tables/*.toml` and `<data-dir>/generators/*.toml` are loaded next. A user item with the same `id` replaces the built-in one.
3. Files that fail to parse or validate are skipped and recorded in a warnings list, shown in the Library and printed by the CLI to stderr.

## 6. Built-in content

### 6.1 Required generators

At least these fifteen, all original, all built from multiple tables:

| id | category | built from |
|---|---|---|
| `npc` | Characters | given name, family name, age, occupation, appearance, traits ×2, motivation, secret (30%) |
| `creature` | Creatures | form, trait, ability, weakness, behavior, lair |
| `encounter` | Adventure | terrain, number (dice), creature form, activity, disposition, twist via `event` (25%) |
| `settlement` | Places | name (adjective + noun), size, government, feature, trade, problem, leader via `npc` |
| `tavern` | Places | name (adjective + noun), proprietor via `npc`, atmosphere, specialty, clientele, rumor via `rumor` (60%) |
| `wilderness_location` | Places | terrain, landmark, feature, danger, resident (50%) |
| `dungeon` | Sites | type, origin, current state, entrance, inhabitants ×2, hazard via `trap` (60%), treasure via `treasure`, rooms via `dungeon_room` (1d4+3, listed) |
| `dungeon_room` | Sites | shape, feature, contents, exits (1d3), atmosphere |
| `trap` | Sites | trigger, mechanism, effect, telltale sign, severity |
| `quest` | Adventure | patron, goal, target, complication, reward, stakes, location name |
| `rumor` | Adventure | source, claim, veracity |
| `treasure` | Items | coins (dice), valuables ×1d2, magic item via `magic_item` (25%) |
| `magic_item` | Items | base item, material, quality, power, drawback (40%), history |
| `faction` | Organizations | form, theme, goal, method, resource, leader via `npc` |
| `event` | Oracle | event focus, meaning action, meaning subject (used for oracle twists) |

### 6.2 Tables

Each generator slot above is backed by a table (for example `npc_occupation`, `settlement_problem`, `room_feature`, `trap_trigger`, `quest_complication`, `item_power`, `faction_goal`), with shared tables reused across generators (`atmosphere`, `terrain`, `creature_form`, `place_adjective`, `place_noun`, `given_names`, `family_names`). Oracle tables are `event_focus` (d100, ranged), `meaning_action` and `meaning_subject` (d20).

Content rules: list tables have at least 12 items (more for names and occupations), tables are assigned a category and tags, text is generic and setting-neutral, and wording is original. A test must prove that every built-in generator renders successfully across many seeds with no empty or unresolved placeholders.

## 7. TUI

Tabs: **Oracle**, **Library**, **Dice**, **Journal**. Switch with `1`-`4` or Tab/Shift-Tab. `?` toggles a help overlay. `q` or Ctrl-C quits when no text field is focused. Transient messages appear as a toast.

### Oracle
- Likelihood selector showing all five labels; left/right (or `h`/`l`) changes it.
- `/` edits an optional question (Esc finishes). Enter rolls.
- Result panel: outcome in color (green yes, red no, bold for exceptional), the d100 roll, and the twist event if present (rendered like a generator result).
- The last ten oracle results listed below.
- `t` jumps to the Library for freeform "ask the tables" rolling.

### Library (all tables and generators)
- One list containing every loaded table and generator, grouped by category, each marked `T` (table) or `G` (generator). User-added items appear automatically.
- `/` filters by case-insensitive substring over id, name, category and tags; `c` cycles the category filter; Esc clears.
- Right pane for a table: name, dice, description, and the full entry list with ranges (`v` toggles entries view, scrollable).
- Right pane for a generator: description, its slots, and the latest result with each slot's value highlighted.
- Enter rolls the selected item and appends to the journal. `r` re-rolls the whole thing.
- `s` focuses the slot list; up/down selects a slot; Enter re-rolls only that slot and re-renders the template. Esc leaves slot focus.
- Loader warnings are listed in a collapsible panel at the bottom.

### Dice
- Expression input line; Enter rolls; invalid input shows an inline error and does not crash.
- The last ten results are listed with full detail.

### Journal
- Scrollable list (up/down, PgUp/PgDn).
- `n` opens a note input; Enter saves.
- `x` exports markdown to `<data-dir>/export.md` and shows the path in a toast.

## 8. CLI and storage

```
oraculum [--data-dir DIR] [--session NAME] [--seed N]             # TUI
oraculum [..] roll "<expr>"
oraculum [..] ask <very-unlikely|unlikely|even|likely|very-likely> ["question"]
oraculum [..] table <id>
oraculum [..] gen <id>
oraculum [..] list [--category CAT] [--search TEXT]               # tables and generators
oraculum [..] export [--out FILE]
```

- `--data-dir` defaults to the platform data directory (project name `oraculum`). `--session` defaults to `default`. `--seed` makes all rolls reproducible.
- Headless commands print the same text that is written to the journal and append to the journal. Warnings go to stderr. Invalid input yields a non-zero exit code and a clear message.
- Data layout: `<data-dir>/tables/`, `<data-dir>/generators/`, `<data-dir>/sessions/<session>.jsonl`, `<data-dir>/export.md`.

## 9. Testing strategy

- **Dice:** table-driven parse tests (valid and invalid), scripted-roller cases for keep-highest/lowest, `d66`, negative terms and constants; proptests that every die is within 1..=M and totals equal kept dice plus constants.
- **Oracle:** every worked example above; exhaustive checks over all 100 rolls at each likelihood (yes count equals the chance, exceptional counts match the formula, yes count increases with likelihood, exactly 9 twist rolls).
- **Tables:** parse and validation tests for every error class (gap, overlap, out of range, bad `d66` digit, empty); inline reference and recursion-limit tests; cycle detection.
- **Generators:** slot behavior tests (count, distinct, join, list, chance) using scripted rolls; nested generators; load-time validation; every built-in generator renders cleanly across many seeds; single-slot re-roll leaves other slots unchanged.
- **Journal:** JSONL append/load with temp dirs, malformed-line reporting, markdown export snapshot.
- **CLI:** `assert_cmd` tests with `--seed` and a temp `--data-dir` covering stdout, stderr, exit codes and journal contents.
- **TUI:** `update` unit tests with synthetic key events (likelihood change, invalid dice input shows an error, `q` while typing does not quit, Library filtering and slot re-roll); `TestBackend` render tests asserting key strings are present.

## 10. Non-goals

Character sheets, initiative tracking, maps, networking or sync, AI features, importing proprietary table formats, mouse support, themes, sound, a "chaos factor" mechanic, cross-slot dependencies inside generators.
