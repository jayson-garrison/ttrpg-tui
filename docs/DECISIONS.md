# Oraculum: Decisions

Records where `docs/DESIGN.md` (the spec) is ambiguous, silent, or internally inconsistent, as the spec requires (§3, last bullet). Each entry: spec reference, decision, reason. Referenced from `docs/ARCHITECTURE.md` as `D-nn`.

**No spec hard constraint (§3) is changed by any entry.** Entries that touch spec text beyond filling a gap are marked **[refines]**. Entries the human decided are marked **[human-confirmed]** (card HAG-1); entries still awaiting the human are listed in ARCHITECTURE §12.

---

**D-01 Generator count.** Spec §1 says "at least ten", §6.1 lists and requires fifteen. *Decision:* the fifteen are required; the goal of ten is met. *Reason:* the later, specific list wins.

**D-02 Library target in the binary crate. [refines §4]** *Decision:* package `oraculum` has `src/lib.rs` (lib name `oraculum_app`) and a thin `src/main.rs`. Still two crates. *Reason:* `tests/` can only import a library; avoids lib/bin name collision.

**D-03 One shared `Engine` in core.** *Decision:* CLI and TUI call the same `Engine` methods, which build the journal `Entry`. *Reason:* guarantees "headless prints what the journal stores" (§8) and keeps logic out of the shell.

**D-04 Own the random stream.** Spec §5.1 requires `ChaCha8Rng`. *Decision:* expand `--seed` (u64) to the 32-byte ChaCha seed with our own SplitMix64, and map `next_u32` to `1..=sides` with our own rejection sampler instead of `rand`'s range API. *Reason:* pinned expectations (goldens, scripted tests) must not shift when `rand` changes its sampling.

**D-05 `Roller::roll(sides < 2)`.** Spec says "sides >= 2" but no behavior otherwise. *Decision:* return 1, never panic. Validated dice make this unreachable.

**D-06 Seed is always concrete.** Spec: `--seed` optional. *Decision:* without it the binary draws a random u64 from OS entropy, uses it, and shows it (TUI help/status). *Reason:* any session can be reproduced afterward. OS entropy is only used to pick the seed; every roll still goes through `Roller`.

**D-07 `d66` token. [refines §5.2]** Grammar allows `[N]dM` and `d66`, which collide. *Decision:* bare `d66` (no count, no keep suffix) is tens/units; `1d66`, `2d66`, `d66kh1` are ordinary 66-sided dice. `d66` may be a term in a larger expression.

**D-08 Dice syntax details.** *Decision:* no unary minus or plus (`-3` is an error); decimal integers, leading zeros accepted, constants up to 1,000,000; whitespace allowed between tokens, not inside an integer; `kh`/`kl` without K means 1.

**D-09 Aggregate dice limits.** Spec limits are per term. *Decision:* also max 32 terms and 1,000 dice per expression. *Reason:* bounds work and display size; far above real use.

**D-10 Single-pass substitution.** *Decision:* text substituted for a placeholder is never re-scanned. *Reason:* literal braces from `{{` survive nested resolution; a user table entry cannot inject placeholders.

**D-11 Table coverage rule. [refines §5.4]** "Cover exactly the possible totals" is undefined for arbitrary expressions. *Decision:* for non-d66 expressions the totals are every integer from the minimum to the maximum (sums of contiguous ranges are contiguous; kh/kl keeps this); for bare `d66` the 36 values with both digits 1-6; `d66` inside a compound table dice is rejected. Ranges are signed (`1d6-2` has min -1).

**D-12 Slot evaluation order and `distinct`.** *Decision, in order:* chance roll (if <100) => count (dice roll if an expression, clamped 0..=50) => each value. Duplicate detection compares table **entry index**, not text. "Up to 10 attempts" = at most 10 rolls per value including the first; then the value is dropped. `distinct` is ignored on generator and dice slots. *Reason:* fixes the roll sequence so scripted tests are exact.

**D-13 Slot re-roll forces presence.** Spec: re-roll one slot (§5.5, §7). *Decision:* the re-rolled slot is present even if `chance` would have hidden it. *Reason:* the user explicitly asked; otherwise an absent slot could never be re-rolled.

**D-14 `count` bound.** *Decision:* load-time error if a count's maximum exceeds 50. *Reason:* unbounded output.

**D-15 Template `{roll:..}` and re-render.** Spec requires single-slot re-roll without re-rolling the rest, but templates may contain `{roll:expr}`. *Decision:* template rolls are evaluated once, stored in the result, and reused on slot re-roll.

**D-16 Depth counters.** *Decision:* generator-in-generator depth (root 0, max 5) and table-in-table depth (root 0, max 5) are separate; table depth restarts for each slot. Both are checked at load; the runtime guard is defensive.

**D-17 Separate id namespaces. [refines §5.7]** "Same `id` replaces the built-in" doesn't say whether a table and generator may share an id. *Decision:* ids are unique per kind; replacement is within kind; the Library key is (kind, id).

**D-18 Strict TOML keys.** *Decision:* unknown keys are errors naming the key (`deny_unknown_fields`). *Reason:* typos like `chanse` would otherwise silently change behavior; the item is excluded with a warning, per spec.

**D-19 Duplicate user ids.** *Decision:* files load in filename order; the later wins and a warning is recorded.

**D-20 Tables referencing missing tables.** Spec only validates generator references. *Decision:* a table with a dangling `{table:id}` is excluded with a warning; exclusion cascades to a fixpoint.

**D-21 Revert-user-first. [refines §5.4]** Spec: a table in a cycle is excluded. A user override could create a cycle through built-ins and delete them. *Decision:* in a cycle (or dangling/depth failure) that includes user-origin items, drop the user-origin members and restore any built-in they replaced; if only user items are involved, exclude them all (spec behavior). A built-in set alone is always valid (tested).

**D-22 Journal entry fields.** Spec lists the fields without types. *Decision:* `input`/`result` are strings with per-kind conventions (ARCHITECTURE §3.9); `detail` is JSON; an additive `v: 1` field is written; unknown fields on load are ignored. **[refines §5.6]** (adds `v`).

**D-23 `detail` is informational.** *Decision:* nothing in the app reads `detail` back; its keys are documented but not a stable API. *Reason:* avoids schema-compatibility burden on old journals.

**D-24 UTC formatting without a date crate.** Spec §2 forbids extra dependencies without reason. *Decision:* implement civil-from-days by hand and test against known epochs.

**D-25 Markdown export format.** *Decision:* exact layout in ARCHITECTURE §3.9: `# Session: <name>`, bullets with `YYYY-MM-DD HH:MM UTC · kind · input → first line`, continuation lines indented two spaces, notes as separate paragraphs, file order, no escaping.

**D-26 Twist generator id. [human-confirmed]** Spec: "id configurable", no mechanism given. *Decision:* engine parameter defaulting to `event`; users change behavior by supplying their own generator with id `event`. No `config.toml` and no flag: the human confirmed this (HAG-1).

**D-27 Embedding content.** *Decision:* `include_str!` lists in `oraculum-core/src/builtin.rs` pointing to the repo-root `content/`; no `build.rs`; a test compares the lists with the directory. *Consequence:* core cannot be published to crates.io unchanged (not a goal).

**D-28 Table metadata defaults.** *Decision:* `name` required; `category` defaults to `Uncategorized`; `tags` to empty; `description` optional.

**D-29 Lazy content loading in the CLI.** Spec says warnings print to stderr. *Decision:* only commands that need content (`ask`, `table`, `gen`, `list`, TUI) load it and print warnings; `roll` and `export` do not.

**D-30 RNG restarts each CLI invocation. [human-confirmed]** *Decision:* each process builds a fresh RNG from `--seed`; there is no persisted stream position. *Reason:* simplest reading of "same seed => identical output". Confirmed by the human (HAG-1).

**D-31 Session names.** `--session` becomes a filename. *Decision:* must match `^[A-Za-z0-9_-]{1,64}$`, otherwise exit 2. *Reason:* blocks path traversal (`../x`).

**D-32 Safe append.** *Decision:* append with `O_APPEND`, one `write_all` of `line + "\n"`; if the file's last byte isn't `\n` (crash-truncated line), write `\n` first. *Reason:* otherwise the next entry would merge with the partial line and both be lost.

**D-33 Oracle table verified.** *Check made:* for all five rows the spec's exceptional ranges equal the §5.3 formula; all ten worked examples hold; there are exactly nine doubles in 1..=100. *Decision:* the table is a test fixture that must equal the formula. No change to spec.

**D-34 No new dependencies.** *Decision:* none beyond spec §2. Rejected: `chrono`/`time` (D-24), `tui-input` (own `LineInput`), `unicode-width` (ratatui measures widths), `getrandom` directly (via `rand`). Known limitation: wide-glyph cursor alignment in text inputs.

**D-35 User file hygiene.** *Decision:* only `*.toml` directly in the folder, dotfiles and `*~` ignored, files over 1 MiB or unreadable/non-UTF-8 become warnings. *Reason:* "broken user files never crash".

**D-36 Minimum terminal size.** Spec silent. *Decision:* 60x20; smaller shows a "terminal too small" message.

**D-37 Tabs emit intent actions. [refines §4]** Spec: `update(&mut self, KeyEvent) -> Action`, tabs never read each other's state, yet rolls need `Roller`/`Clock`/journal. *Decision:* keep both signatures; add `Action::Roll(RollRequest)` and `Action::AppendNote`; the `App` runs the Engine, appends to the journal, then sends `TabEvent`s back. Tabs share only the immutable `Rc<Catalog>`. *Reason:* tabs stay pure and testable with no RNG.

**D-38 Tab key always switches. [refines §7]** Spec: `Tab`/`Shift-Tab` switch with no qualification, `q`/`Ctrl-C` only "when no text field is focused". *Decision:* `Tab`/`Shift-Tab` work even while editing; `1`-`4`, `?`, `q` only when no field is focused.

**D-39 Ctrl-C while editing. [refines §7]** *Decision:* delivered to the field as `Esc` (cancels the edit); a second `Ctrl-C` quits. *Reason:* in raw mode Ctrl-C is a key event; ignoring it would feel like a hang, quitting would break the spec's rule.

**D-40 Oracle `t`.** Spec: jump to the Library for freeform rolling. *Decision:* open the Library filtered to category `Oracle` (built-in oracle tables and the `event` generator).

**D-41 Library keys not in spec.** *Decision:* `w` toggles the warnings panel; in the filter field `Enter` keeps the text and `Esc` clears it; in Browse `Esc` clears text and category.

**D-42 Dice input focus.** Spec: "expression input line". An always-focused input would disable `q` and `1`-`4`. *Decision:* the tab starts in Edit; `Esc` goes to Browse; `/`, `i` or `Enter` re-enters Edit.

**D-43 Timestamps and reproducibility.** *Decision:* "identical output" covers stdout and journal `result`/`input`; `ts` is excluded. CLI tests mask `ts`; unit tests use `FixedClock`.

**D-44 Mechanical constraint checks (non-spec CI step).** *Decision:* add `cargo deny check bans` (C `-sys` crates, async runtimes, network crates). Tool only, no dependency. Advisory; drop if only the three spec gates are wanted.

**D-45 Library and history details.** *Decision:* `Enter` rolls the selected item, `r` repeats the item that produced the displayed result; a slot re-roll is journaled as kind `generator` with input `<id> [slot <key>]`; Oracle and Dice "last ten" lists are in-memory for the session (not rehydrated from the journal); the Journal tab loads the full file at startup.

**D-46 Content originality sign-off. [human-confirmed]** Spec §3: built-in content must be original. *Decision:* the human personally signs off originality of all built-in tables and generators before release (HAG-1). The developer adds a provenance header comment to every content file; Rick (QA) prepares a review packet (all content files plus rendered samples across seeds) for the human. Release is blocked until the human signs off; the sign-off is recorded in `content/PROVENANCE.md` (reviewer, date, commit).

**D-47 No settings file. [human-confirmed]** Consequence of D-26: v1 has no `config.toml`. The only user-visible configuration is the CLI flags in spec §8 and files in the data directory.

**D-49 Pinned toolchain. [refines A§2, closes A§12 Q2 for now]** *Decision:* `rust-toolchain.toml` pins a concrete release (1.99.1, edition 2021) instead of `stable`. *Reason:* clippy adds lints between releases; CI (newer than a developer's local install) failed T1 on `chunks_exact_to_as_chunks` that local clippy did not know. With the pin, rustup installs the same toolchain locally and in CI. *Maintenance:* bump the pin deliberately in its own PR and fix any new lints there.
