# T7-T9 TUI Plan

For Stack (developer) and Rick (QA). Sources: `DESIGN.md` §4, §7, §9 ("Spec"); `ARCHITECTURE.md` §3.10, §5.4, §9 ("A"); ADR 0001; D-36..D-45, D-57, D-66, D-72, D-76. New decisions are D-77..D-83 in `DECISIONS.md`. A§5.4 already fixes the `Tab` trait, `App`, `Action`, `TabEvent` and key order; this plan does not repeat those definitions, it adds what is missing so no design question remains: per-tab state, render shapes at 60x20, the pieces A§5.4 left open, test targets and the PR split.

Skills used: grill-with-docs-auto not available ("Unknown skill"); grilling and domain-modeling used instead, answers recorded as D-77..D-83.

## 1. Module layout (`crates/oraculum/src/tui/`)

```
mod.rs            pub fn run(engine, store, startup) -> Result<()>   // enter guard, loop, restore
terminal.rs       TerminalOps trait, CrosstermOps, TerminalGuard, install_panic_hook, restore
event_loop.rs     poll(250 ms) -> Event; calls App::handle_key / tick / render
app.rs            App, handle_key order (A§5.4), Action execution, toast, help flag
action.rs         Tab trait, Action, RollRequest, LibraryTarget, TabEvent, Toast
input.rs          LineInput (D-34)
theme.rs          styles: one place for colors; every color has a text twin (label, marker)
widgets/          tab_bar.rs, status_line.rs, help.rs, toast.rs, result_block.rs, too_small.rs
tabs/             dice.rs, oracle.rs, journal.rs, library.rs
testkit.rs        #[cfg(test)] fixture catalog, ScriptedRoller app builder, buffer helpers
```

`main.rs`: with no subcommand, build the Engine and `JournalStore` as the CLI does (same data-dir/seed/session flags, same warnings handling) and call `tui::run`. Everything under `tui/` except `terminal.rs` and `event_loop.rs` is pure and runs in tests with no terminal.

## 2. Decisions that complete A§5.4

1. **One new Action: `ExportJournal` (D-77).** A§5.4 says the App performs export on `x` but the `Action` enum has no variant for it. Add `Action::ExportJournal`. The App writes `data_dir.export_path()` through the existing storage function and emits `Toast("exported to <path>")`; failure is an error toast.
2. **Rolled results carry the journal entry (D-77).** `TabEvent::Rolled(Box<Rolled>)` (the Engine's `Rolled { result, entry }`), not just `RollResult`, so a tab can show the same text the journal stores. `RollFailed(String)` is for `EvalError` only.
3. **Result block widget (D-78).** `widgets::result_block::lines(result: &GeneratorResult, focus: Option<usize>) -> Vec<Line>` renders `segments`: a segment with `slot: Some(i)` is highlighted (underline + bold, never color alone), the focused slot additionally reversed. It is used by the Oracle twist and by the Library, so highlighting is written once. Nested generator values appear as their already-rendered text; no expansion of `ValueDetail::Generator`.
4. **Slot picker uses `result.slots`, not segments (D-78, Rick's note).** Under D-76 a dropped optional line leaves no segment for its slot. The picker therefore lists every entry of `result.slots` in declaration order: `key`, current value(s) joined, and `(absent)` dimmed when `present == false`. Segments are used only for highlighting. Re-rolling an absent slot is allowed (the chance is rolled again by `Engine::reroll_slot`). Selection index = index in `slots`, which is exactly the `slot: usize` in `RollRequest::RerollSlot`.
5. **Minimum size and layout (D-79).** 60x20 minimum (D-36/D-57). Frame = row 0 tab bar, rows 1..H-2 body, last row status line (toast if any, else key hints of the active tab). Body at 60x20 is 58x18 inside a 1-cell border. Anything that does not fit is clipped with `…`, never panics; wrap uses `Paragraph` with `trim: false`. All render code uses `saturating_sub` and `Rect` helpers; a fuzz-lite render test runs every tab at 60x20, 80x24 and 200x60.
6. **Colors (D-79).** Green Yes, red No, bold exceptional (Spec), but the outcome label is always text; focus and selection use reverse video; the TUI must be legible with colors off (render tests assert on text only).
7. **App is testable without a terminal (D-80).** `App::new(engine: Engine, store: JournalStore, info: AppInfo) -> App` where `AppInfo { seed: u64, session: String, warnings: Vec<LoadWarning>, data_dir: DataDir }`. Tests build it with `ScriptedRoller`, `FixedClock`, a `tempfile` dir, and a fixture catalog. `render(&self, frame)` and `handle_key(&mut self, KeyEvent) -> bool` are the only entry points tests use; the event loop is a thin wrapper and is covered by the `TerminalOps` mock and manual QA.
8. **Toast time (D-80).** `expires_at = clock.now() + 3` epoch seconds; `App::tick()` compares with the Clock, so tests advance a `FixedClock`.
9. **Startup (D-81).** `run` loads the journal (`JournalStore::load`), sends `JournalLoaded(entries, skipped)` to the Journal tab, shows a toast "N malformed journal lines skipped" when `skipped > 0`, and the tab bar shows `seed <n>` and `warnings <n>` (the latter highlighted when > 0).
10. **Fixture vs built-in content (D-72, D-82).** TUI tests use `testkit::fixture_catalog()` only. One smoke test per tab (`builtin_smoke`) builds the real built-in catalog, renders at 60x20 and 80x24, and asserts no panic and only structural strings (`Library`, a category header `Oracle`, the `T`/`G` markers); it never asserts generated text.

### Fixture catalog (`testkit.rs`)

| id | kind | shape | purpose |
|---|---|---|---|
| `colors` | table | list, 3 items | roll, entries view |
| `loot` | table | ranged `d6`, 3 ranges | entries view with ranges |
| `inner` | generator | one table slot | nested value |
| `pair` | generator | slots: `a` (table, chance 100), `b` (generator `inner`, chance 100), `c` (dice `1d6`) | highlighting, slot picker |
| `opt` | generator | template with line `Extra: {tail}`, slot `tail` chance 50 on its own line | D-76: absent slot, picker lists it as `(absent)` |
| `event` | generator | one table slot | oracle twist |
| `bad_ref` (user layer) | generator | references an unknown table | produces a loader warning for the `w` panel |

Categories: `Alpha` (colors, loot), `Beta` (pair, inner, opt), `Oracle` (event). Rolls are fed through `ScriptedRoller`, so every test states the exact numbers it expects.

## 3. Per-tab specification

Key bindings follow Spec §7 and A§5.4 exactly; this section gives state, the `Action`s emitted, and the rendering at 60x20. "G" = global keys handled by `App` before the tab (A§5.4 order): `Tab`/`Shift-Tab`, `1`-`4`, `?`, `q`, `Ctrl-C` (D-38/D-39).

### 3.1 Dice (T7)

```rust
pub struct DiceTab {
    input: LineInput, mode: DiceMode /* Edit | Browse */, error: Option<String>,
    history: VecDeque<DiceLine>,            // newest first, max 10
}
struct DiceLine { expr: String, detail: String }   // detail = DiceRoll Display, e.g. "3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5"
```
- `captures_input() = mode == Edit`. Starts in Edit (D-42).
- Edit: chars, Backspace, Delete, ←/→, Home/End edit the line. `Enter`: `DiceExpr::parse(text)`; `Err(e)` sets `error = Some(e.to_string())`, no Action; `Ok` returns `Action::Roll(RollRequest::Dice { expr })` and clears `error`. `Esc` goes to Browse.
- Browse: `/`, `i` or `Enter` back to Edit. G keys active.
- `on_event(Rolled)`: push to history (truncate to 10), keep the input text so Enter can repeat. `RollFailed`: show as `error`.
- Render: line 1 `Dice: <input with cursor>`; line 2 error in red with `!` prefix (or hint "e.g. 2d6+3, d66, 4d6kh3" when no error); then "Last results" list, one `detail` per line (clipped to width). Shows at most 10 lines; at 18 body rows nothing scrolls.
- Help: `Enter roll  Esc leave input  / edit`.

### 3.2 Oracle (T8)

```rust
pub struct OracleTab {
    likelihood: Likelihood, question: LineInput, editing: bool,
    last: Option<OracleView>, history: VecDeque<String>,     // max 10, newest first
}
struct OracleView { roll: OracleRoll, question: Option<String>, twist: TwistView }
enum TwistView { None, Event(GeneratorResult), Unavailable { generator: String, missing: String } }
```
- `←/→`, `h/l` change likelihood (clamped, no wrap). `/` edit question (`captures_input() = editing`). `Esc` finishes editing. `Enter` rolls: when editing it first finishes editing, then returns `Action::Roll(Oracle { likelihood, question: (!trim.is_empty()).then(..) })`. `t` returns `Action::OpenInLibrary(LibraryTarget::Category("Oracle".into()))` (D-40).
- `on_event(Rolled(Oracle))` stores `OracleView`, pushes `"<Likelihood> <outcome label> d100:<N>"` to history.
- Render (inner width 58): row 0 likelihood selector: five labels, the active one reversed and bracketed `[Even]`; row 1 `Question: <text>`; row 3 result panel: outcome label (bold if exceptional, green/red), `d100: N`, then a `Twist:` block using `result_block::lines` for `Event`, or `Twist unavailable: <generator> is missing <missing>` for `Unavailable`; below, "Last 10" history. Result panel is 8 rows max; history takes the rest and is clipped.
- Help: `←/→ likelihood  / question  Enter ask  t library`.

### 3.3 Journal (T8)

```rust
pub struct JournalTab {
    entries: Vec<Entry>, selected: usize, follow: bool,       // follow = stay at the end
    note: Option<LineInput>, skipped: usize, scroll_top: usize,
}
```
- `↑/↓` move selection; PgUp/PgDn move by body height - 1; `Home`/`End` jump; any upward move sets `follow = false`; reaching the last entry (or `End`) sets it true. `n` opens the note input (`captures_input() = note.is_some()`); `Enter` with non-empty trimmed text returns `Action::AppendNote(text)` and closes; empty text just closes; `Esc` cancels. `x` returns `Action::ExportJournal`.
- `on_event(JournalLoaded)` replaces entries; `JournalAppended(entry)` pushes; if `follow`, selection moves to it. `JournalWriteFailed(msg)` is shown as a toast by the App; the entry is still appended to the list (A§5.4).
- Render: one line per entry: `HH:MM  <kind>  <input> -> <result>` (UTC, via the formatter the markdown export uses; if it is private in core, expose it, no new dependency per D-24/D-34); notes `HH:MM  note  <text>`. Multi-line results are collapsed to the first line plus `…`. Title shows `Journal (<n> entries)`; a note input replaces the bottom row while open.
- Help: `n note  x export  ↑/↓ PgUp/PgDn scroll`.

### 3.4 Library (T9)

```rust
pub struct LibraryTab {
    catalog: Rc<Catalog>, warnings: Vec<LoadWarning>,
    mode: LibMode /* Browse | Filter | Slot */, filter: LineInput, category: Option<String>,
    rows: Vec<Row> /* Header(String) | Item(usize into catalog.items()) */, selected: usize, scroll_top: usize,
    view: LibView /* Detail | Entries */, entries_scroll: usize,
    last: Option<LastResult>, slot_selected: usize, warnings_open: bool,
}
struct LastResult { kind: ItemKind, id: String, result: RollResult /* Table or Generator */ }
```
- `rows` is rebuilt (never cached across a filter/category change) from `catalog.search(&Filter { text, category, kind: None })`, grouped by category in alphabetical order, items in catalog order; headers are skipped by ↑/↓. `selected` is kept on the same item id when it is still visible, else the first item.
- Browse keys: `/` Filter (`captures_input() = true` in Filter); `c` cycles `None -> categories alphabetical -> None`; `↑/↓`, PgUp/PgDn; `v` toggles Entries view (tables only; toast "entries view is for tables" otherwise); `Enter` returns `Roll(Table{id}|Generator{id})` for the selected item; `r` repeats `last` (same kind/id as the displayed result; toast "nothing rolled yet" if none); `s` enters Slot mode if `last` is a generator result (else toast); `w` toggles the warnings panel; `Esc` clears filter text and category.
- Filter keys: edit text; `Enter` keeps it and returns to Browse; `Esc` clears and returns to Browse (D-41). Filtering is live on every keystroke.
- Slot mode (D-78): shows the slot picker over `last.result.slots`; `↑/↓` select; `Enter` returns `RollRequest::RerollSlot { previous: Box<GeneratorResult>, slot: slot_selected }`; `Esc` leaves. `captures_input() = false` in Slot mode.
- `on_event(Rolled(Table|Generator))` sets `last`, resets `slot_selected` to 0 (for `RerollSlot` results keep `slot_selected`); `Focus(Item{kind,id})` clears the filter and category, selects that item; `Focus(Category(c))` sets the category filter and clears text, selects the first item.
- Render: left 40% (min 22 columns) list: category header line, then `T`/`G` marker + name (clipped with `…`); the selected row reversed. Right pane: for a table, name, `dice`, description, then (Detail) its latest roll or (Entries) the entry list with ranges for ranged tables, scrollable; for a generator: description, slots overview, then the latest result via `result_block::lines` (focus = `slot_selected` in Slot mode) and the slot picker below it in Slot mode. Bottom panel (4 rows) when `warnings_open`: one line per `LoadWarning` display string, scrollable with ↑/↓ while open; the count is always in the title (`Library  warnings 2`). An informational `Replaced` warning is shown dimmed (D-54).
- Help: `/ filter  c category  Enter roll  r repeat  s slot  v entries  w warnings`.

## 4. Terminal restore

`TerminalOps` trait: `enable_raw_mode`, `disable_raw_mode`, `enter_alt_screen`, `leave_alt_screen`, `show_cursor`. `TerminalGuard<O: TerminalOps>` holds a `restored: Cell<bool>`; `restore()` is idempotent; `Drop` calls it. `install_panic_hook(restore_fn)` chains: take the previous hook, set a new hook that calls restore then the previous hook, installed before `enter()`. Restore also runs on `Err` returned from the loop (the guard drops). Mock-based tests: (a) normal drop restores once; (b) restore twice is harmless; (c) restore on unwind (`catch_unwind` around a closure holding the guard, mock records calls); (d) hook order: restore happens before the previous hook runs. Manual QA (Rick): a local one-line `panic!()` patch in macOS Terminal and a Linux terminal leaves a usable shell with the message on the normal screen.

## 5. Test targets (Spec §9 TUI, mapped)

| Spec §9 item | Test (name) | Where |
|---|---|---|
| likelihood change | `oracle_left_right_changes_likelihood_and_clamps` | oracle.rs unit |
| invalid dice shows error, not journaled | `dice_invalid_expr_sets_error_and_emits_no_action`; `app_dice_invalid_does_not_append_journal` | dice.rs, app.rs |
| `q` while typing does not quit | `q_in_dice_edit_is_text_not_quit`; `q_in_browse_quits`; `ctrl_c_while_editing_cancels_then_quits` | app.rs |
| digits and `?` ignored while typing | `digits_are_text_in_note_input` | app.rs |
| Tab always switches | `tab_switches_while_editing` | app.rs |
| Library filtering | `library_filter_matches_id_name_category_tag`; `library_esc_clears_text_then_category` | library.rs |
| category cycle | `library_c_cycles_all_then_alphabetical_then_all` | library.rs |
| slot re-roll | `library_s_enter_emits_reroll_slot_with_selected_index`; `app_reroll_slot_changes_only_that_slot` (ScriptedRoller) | library.rs, app.rs |
| slot picker with dropped slot (D-76/D-78) | `slot_picker_lists_absent_slot_from_result_slots` | library.rs |
| roll journals | `app_roll_appends_entry_and_notifies_journal_tab` | app.rs |
| journal write failure keeps running | `app_journal_write_failure_toasts_and_shows_entry` (store path made unwritable) | app.rs |
| `Focus` / `t` | `oracle_t_opens_library_on_oracle_category` | app.rs |
| toast expiry | `toast_expires_after_three_seconds_with_fixed_clock` | app.rs |
| TestBackend: key strings present | `render_tab_bar_shows_four_tabs_seed_and_warnings`; `render_dice_60x20_shows_input_and_history`; `render_oracle_shows_outcome_label_roll_and_twist`; `render_library_shows_markers_and_category_headers`; `render_help_overlay_lists_global_and_tab_keys`; `render_too_small_under_60x20`; `render_all_tabs_at_60x20_80x24_200x60_do_not_panic` | widgets/, tabs/ |
| terminal restore | the four tests in section 4 | terminal.rs |

Render tests assert that strings are present in the `TestBackend` buffer (not full-screen snapshots), so layout tweaks do not churn them.

## 6. Pull requests

Each PR ends green on the three gates. Rick tests after Stack marks it done. Acceptance extends the Rick lines in A§13 (T7/T8/T9), which stay authoritative.

| PR | Contents | Acceptance (Rick) |
|---|---|---|
| **T7.1** shell (1.5 d) | `terminal.rs` + mock tests, event loop, `App` shell with `handle_key` order, `Tab` trait, `Action`/`TabEvent`, tab bar, status line, help overlay, toast, too-small, `LineInput`, theme, `testkit`, `main.rs` launches the TUI with four placeholder tabs | `1`-`4` and Tab/Shift-Tab switch; `?` opens, any key closes; `q` quits; under 60x20 shows "terminal too small" and recovers when resized; terminal restored on `q`, `Ctrl-C` and a forced panic (macOS Terminal + Linux); section 4 tests pass |
| **T7.2** Dice tab + roll plumbing (1.5 d) | `DiceTab`; `Action::Roll` execution in the App (Engine call, journal append, `Rolled` / `RollFailed` / `JournalAppended` events); `JournalLoaded` at startup | `q` in the input types a `q`; `Esc` then `q` quits; invalid input (`2d`, `101d6`, empty) shows an inline error, nothing journaled (check the journal file); valid roll prints exactly the CLI's detail line; history keeps 10; with `--seed 7` the same keystrokes give the same results across runs |
| **T8.1** Oracle tab + result block (1.5 d) | `OracleTab`, `widgets::result_block`, twist rendering, `t` wiring emitting `OpenInLibrary` (App shows toast "Library not available yet" until T9) | `←/→` change likelihood; `/` edits the question, `Esc` ends; Enter rolls and shows label, `d100: N`; a doubles roll (use a seed that hits one, found with `oraculum ask`) shows a twist block; missing `event` generator shows the "unavailable" text (user override removing it); no color needed to read the answer |
| **T8.2** Journal tab (1.5 d) | `JournalTab`, `AppendNote`, `ExportJournal`, startup toast for skipped lines | `n` adds a note and it appears live; `x` writes `export.md` and shows the path; a session then `oraculum export` shows the same entries; a journal with a truncated last line shows the skipped-lines toast; PgUp/PgDn scroll and the list follows new entries only when at the end |
| **T9.1** Library browse (2 d) | `LibraryTab` list, filter, category cycle, detail and entries views, `Enter` roll, `r` repeat, `Focus` and `OpenInLibrary` wiring (Oracle `t` now works), user-added items appear | `/` filters by id, name, category, tag and `Esc` clears; `c` cycles; `v` shows ranged entries with ranges; Enter rolls and journals; `r` repeats the displayed item; a user table dropped into the data dir appears; `t` from Oracle lands on category `Oracle` |
| **T9.2** Generator results, slot picker, warnings (2 d) | highlighted generator results, slot mode over `result.slots`, `RerollSlot` journaled as `<id> [slot <key>]` (D-45/D-53), warnings panel and header count | `s` then Enter re-rolls only the chosen slot (all other slot values identical); a generator with an optional slot that was dropped still lists it as `(absent)` in the picker and can be re-rolled; `w` shows loader warnings from a deliberately broken user file; `Replaced` shows as informational; nothing journaled when `s` is used with no generator result (toast only) |
| **T9.3** Hardening (0.5 d) | all-tabs render fuzz at three sizes, `builtin_smoke` tests (D-72), remove the T8.1 placeholder toast, help text audit | no panic resizing from 200x60 down to 59x19 and back in each tab; help lists every key the tab accepts |

T7.1 first, then T7.2. T8.1, T8.2 can proceed in either order after T7.2 (they share only the App). T9.1 needs T8.1 (result block); T9.2 needs T9.1. D-66 fixes the nominal order T7, T8, T9; the table keeps it.

## 7. Risks

- **Journal formatter:** if the UTC `YYYY-MM-DD HH:MM` formatter is private in core, expose it as `pub` in the T8.2 PR; no new dependency.
- **Wide glyphs:** cursor alignment in `LineInput` is a known limitation (D-34); built-in content is ASCII (D-69), user content may not be.
- **Library at 60 columns** is tight (24-column list). Names are clipped; the item `id` is shown in the detail pane so nothing is unreachable.
- **Event loop** is only manually verified plus the mock; keep it under 60 lines and free of logic.
