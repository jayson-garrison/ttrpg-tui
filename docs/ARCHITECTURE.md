# Oraculum: Architecture

Status: draft 1, iterates `docs/DESIGN.md` (the spec). The spec stays the source of truth for *what* to build; this document says *how*. Where this document refines or fills a gap in the spec, the item is recorded in `DECISIONS.md` as `D-nn` and referenced here. No spec hard constraint (spec §3) is changed. Items that need the human are in §12.

Notation: `§n` = spec section unless prefixed `A§` (this document). Rust blocks are signatures to implement, not final code. Crate versions are not pinned here; pin them at M0 with `cargo add` and record them in `Cargo.lock`.

---

## 1. Principles

1. **Pure core, thin shell.** `oraculum-core` is deterministic functions over `&str` and injected `Roller`/`Clock`. All I/O (files, terminal, OS entropy, system time) lives in `oraculum`.
2. **One execution path.** CLI and TUI both call the same `Engine` methods, so "headless prints what the journal stores" (§8) holds by construction.
3. **Validate at load, evaluate infallibly.** Content is fully validated when the `Catalog` is built. Rolling validated content cannot fail except through defensive depth guards.
4. **Never crash on user data.** Every user-file problem becomes a `LoadWarning`. Every journal problem becomes a skipped-line report.
5. **Stable output.** No `HashMap` iteration, no unordered directory reads, no wall-clock in rolling code. Same seed + same content => same bytes.

```
            +-------------------------- oraculum (binary crate) --------------------------+
 keys ----> | tui::App --Action--> Engine calls --> JournalStore (append JSONL)            |
 argv ----> | cli::run ----------> Engine calls --> stdout/stderr, export.md               |
            | storage: DataDir, SessionName, content file reader, SystemClock, OS seed      |
            +-------------------------------------+---------------------------------------+
                                                   | uses (no I/O inside)
            +-------------------------- oraculum-core --------------------------------------+
            | Engine -> { dice, oracle, template, table, generator, catalog, journal codec } |
            | injected: &mut dyn Roller, &dyn Clock;  builtin content via include_str!        |
            +---------------------------------------------------------------------------------+
```

---

## 2. Workspace layout

```
oraculum/                         (repo root; workspace)
  Cargo.toml                      [workspace], [workspace.lints], resolver = "2"
  clippy.toml                     allow-unwrap-in-tests / allow-expect-in-tests = true
  deny.toml                       cargo-deny: bans (see §9.4)
  rust-toolchain.toml             stable channel (version: see §12 Q5)
  .github/workflows/ci.yml        fmt, clippy, test on macos-14 + ubuntu-latest
  content/{tables,generators}/*.toml
  crates/
    oraculum-core/
      src/{lib,error,rng,clock,template,table,generator,oracle,engine,builtin}.rs
      src/dice/{mod,parse,roll,display}.rs
      src/catalog/{mod,load,validate,search}.rs
      src/journal/{mod,jsonl,markdown,time}.rs
      tests/                      integration tests (builtin content, goldens)
    oraculum/                     package "oraculum"; lib target `oraculum_app` + bin `oraculum`
      src/main.rs                 anyhow; clap parse; exit codes
      src/lib.rs
      src/clock.rs                SystemClock
      src/error.rs                AppError, StorageError
      src/cli/{mod,run,output}.rs
      src/storage/{mod,data_dir,session,journal_store,content,export}.rs
      src/tui/{mod,app,action,terminal,input,theme,help}.rs
      src/tui/tabs/{mod,oracle,library,dice,journal}.rs
      tests/{cli.rs,storage.rs,tui.rs}
```

Why a lib target in the binary crate (D-02): integration tests in `tests/` can only import a library target. `main.rs` stays ~40 lines. The library is named `oraculum_app` to avoid the bin/lib name collision. The spec's two-crate layout is unchanged.

**Lints (workspace `[workspace.lints]`, inherited by both crates):**

```toml
[workspace.lints.rust]   unsafe_code = "forbid"
[workspace.lints.clippy] unwrap_used = "deny"
                         expect_used = "deny"
                         panic = "deny"
                         todo = "deny"
                         unimplemented = "deny"
                         dbg_macro = "deny"
                         print_stdout = "warn"   # core: no printing at all; binary allows in cli/output.rs
```
`clippy.toml` sets `allow-unwrap-in-tests`/`allow-expect-in-tests`. This enforces the §3 "no unwrap/expect in non-test code" rule through the existing clippy gate. Indexing (`v[i]`) is permitted but every use must be guarded or follow a checked invariant; reviewers look for it, proptests (A§9) try to break it.

**Dependencies:** exactly the spec's list (§2). No additions are proposed (D-34). Core uses `serde`, `serde_json`, `toml`, `rand`, `rand_chacha`, `thiserror`. The binary adds `ratatui`, `crossterm`, `clap`, `anyhow`, `directories`. Core does not depend on `clap`, `ratatui`, `crossterm`, `directories`, or `anyhow`.

---

## 3. `oraculum-core`

### 3.1 Randomness: `rng.rs` (spec §5.1)

```rust
pub trait Roller {
    /// Uniform integer in 1..=sides. Contract: sides >= 2.
    /// For sides < 2 implementations return 1 and never panic (D-05).
    fn roll(&mut self, sides: u32) -> u32;
}

pub struct RngRoller { rng: ChaCha8Rng }
impl RngRoller {
    pub fn from_seed(seed: u64) -> Self;      // seed expanded to 32 bytes by our own SplitMix64 (D-04)
}
// RngRoller::roll: draws next_u32 and applies rejection sampling:
//   limit = (u32::MAX / sides) * sides; loop { x = next_u32(); if x < limit { return x % sides + 1 } }
// We deliberately do NOT use rand's range sampling, so the value stream is owned by this repo
// and cannot shift when `rand` is upgraded.

pub struct ScriptedRoller { script: VecDeque<u32>, calls: Vec<RollCall>, faults: Vec<RollFault> }
pub struct RollCall { pub sides: u32, pub returned: u32 }
pub enum RollFault { Exhausted { sides: u32 }, OutOfRange { sides: u32, value: u32 } }
impl ScriptedRoller {
    pub fn new(values: impl IntoIterator<Item = u32>) -> Self;
    pub fn calls(&self) -> &[RollCall];
    pub fn is_consumed(&self) -> bool;             // script fully used
    pub fn faults(&self) -> &[RollFault];          // tests assert empty
}
// On exhaustion or out-of-range value it records a fault and returns 1 (never panics).
```

The seed is always concrete: with no `--seed`, the binary draws a random `u64` from OS entropy, uses it, and shows it in the TUI help overlay so any session can be reproduced (D-06).

### 3.2 Time: `clock.rs`

```rust
pub trait Clock { fn now_secs(&self) -> u64; }   // Unix epoch seconds, UTC
pub struct FixedClock(pub u64);                   // tests; implements Clock
// SystemClock lives in the binary crate (reads std::time::SystemTime).
```

### 3.3 Dice: `dice/` (spec §5.2)

```rust
pub struct DiceExpr { terms: Vec<SignedTerm> }            // always validated
pub struct SignedTerm { pub sign: Sign, pub term: Term }
pub enum Sign { Plus, Minus }
pub enum Term { Const(u32), Dice(DiceSpec), D66 }
pub struct DiceSpec { pub count: u8, pub sides: u16, pub keep: Option<Keep> }   // 1..=100, 2..=1000
pub struct Keep { pub which: KeepKind, pub n: u8 }                              // 1..=count
pub enum KeepKind { Highest, Lowest }

impl DiceExpr {
    pub fn parse(src: &str) -> Result<Self, DiceError>;
    pub fn canonical(&self) -> String;                    // "3d20kl1+2", lowercase, no spaces; also Display
    pub fn roll(&self, roller: &mut dyn Roller) -> DiceRoll;       // infallible after parse
    pub fn bounds(&self) -> (i64, i64);                   // min and max total
    pub fn totals(&self) -> Totals;                       // for table coverage
}
pub enum Totals { Contiguous { min: i64, max: i64 }, D66 }   // D66 only for the bare term "d66"
```

**Grammar, as implemented (D-07, D-08):**
- Tokens: integers, `d`, `kh`, `kl`, `+`, `-`. ASCII case-insensitive. Whitespace is allowed *between* tokens and ignored; it is not allowed inside an integer (`1 2` is an error, `d 6` is `d6`).
- No unary minus: `-3` and `+3` at the start are errors. Integers are decimal, leading zeros allowed, max 1,000,000 for constants.
- `d66` means the tens/units roll **only** when written bare: no count, no keep suffix. `1d66` and `d66kh1` are an ordinary 66-sided die. `d66` may appear as a term inside a larger expression (`d66+1`).
- Aggregate limits (spec gives per-term only): at most 32 terms and 1,000 dice in total (D-09).
- `kh`/`kl` without K means 1; K must satisfy 1 <= K <= N.

```rust
pub enum DiceError {
    Empty,
    UnexpectedChar { pos: usize, ch: char },
    UnexpectedToken { pos: usize, expected: &'static str },
    NumberTooLarge { pos: usize },
    CountOutOfRange { pos: usize, got: u64 },       // N not in 1..=100
    SidesOutOfRange { pos: usize, got: u64 },       // M not in 2..=1000
    KeepOutOfRange { pos: usize, k: u64, n: u8 },   // K not in 1..=N
    TooManyTerms { max: usize },
    TooManyDice { max: usize },
}   // thiserror; messages are user-facing and include position

pub struct DiceRoll { pub expr: String /*canonical*/, pub terms: Vec<TermRoll>, pub total: i64 }
pub struct TermRoll { pub sign: Sign, pub kind: TermRollKind }
pub enum TermRollKind {
    Const(u32),
    Dice { spec: DiceSpec, rolls: Vec<u32> /*in roll order*/, kept: Vec<bool>, subtotal: u32 },
    D66 { tens: u32, units: u32, subtotal: u32 },       // subtotal = tens*10 + units
}
```

- Roll order: terms left to right; within a `NdM` term, N sequential `roll(M)` calls. `d66` calls `roll(6)` twice, tens first.
- Keep selection: sort indices by value (descending for `kh`, ascending for `kl`), ties broken by lower index first; those K indices are `kept`. The set of kept *values* is what the spec's example shows.
- Display (`impl Display for DiceRoll`), format is fixed for snapshot tests:
  - term text: constant => `2`; dice without keep => `[3, 5]`; with keep => `[14, 3, 17] keep [3]`; d66 => `[3, 5]=35`.
  - terms joined by single spaces, every term after the first prefixed with its sign (`+`/`-`) attached to the term text.
  - full line: `{canonical} => {terms} = {total}`. Example: `3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5`.
- `Totals` for tables: `Contiguous{min,max}` where min/max come from `bounds()` (sums of contiguous ranges are contiguous); `D66` only for the bare `d66` expression; any other expression containing `D66` is rejected as a table `dice` value.

### 3.4 Oracle: `oracle.rs` (spec §5.3)

```rust
pub enum Likelihood { VeryUnlikely, Unlikely, Even, Likely, VeryLikely }
impl Likelihood {
    pub const ALL: [Likelihood; 5];
    pub fn yes_chance(self) -> u8;            // 10, 30, 50, 70, 90
    pub fn label(self) -> &'static str;       // "Very unlikely", ...
    pub fn slug(self) -> &'static str;        // "very-unlikely", ... (journal detail, CLI display)
}
pub enum Outcome { ExceptionalYes, Yes, No, ExceptionalNo }     // names per spec
pub struct OracleRoll { pub likelihood: Likelihood, pub roll: u8, pub outcome: Outcome, pub twist: bool }

/// Pure; the exhaustive tests iterate roll 1..=100 over this.
pub fn evaluate(likelihood: Likelihood, roll: u8) -> OracleRoll;

pub enum Twist {
    None,
    Event(GeneratorResult),
    Unavailable { generator: String, missing: String },   // never an error (spec §5.3.4)
}
pub struct OracleResult { pub roll: OracleRoll, pub question: Option<String>, pub twist: Twist }

pub fn ask(catalog: &Catalog, roller: &mut dyn Roller, likelihood: Likelihood,
           question: Option<&str>, twist_generator: &str) -> OracleResult;
```

Rules: `roll = roller.roll(100)` first. Only when `twist` is true is `catalog.generate(twist_generator, roller)` called, so a non-twist question consumes exactly one roll. A missing generator, or a generator excluded at load, yields `Twist::Unavailable` whose `missing` names the generator id (or the missing table id, taken from the load warning). `evaluate` implements the spec formula verbatim:

```
yes     = roll <= chance
exc     = yes ? roll <= chance/5 : roll > 100 - (100-chance)/5       // integer division
twist   = roll in {11,22,...,99}
```
The spec's rate table is a *test fixture* that must equal what the formula yields (verified for all five rows; no discrepancy found, D-33).

### 3.5 Templates: `template.rs`

One parser serves table entries and generator templates.

```rust
pub struct Template { segments: Vec<Segment> }
pub enum Segment {
    Literal(String),
    TableRef(String),          // {table:id}      table entries only
    Roll(DiceExpr),            // {roll:expr}     both
    Slot(String),              // {key}           generator templates only
}
pub enum TemplateContext { TableEntry, GeneratorTemplate }

impl Template {
    pub fn parse(src: &str, ctx: TemplateContext) -> Result<Template, TemplateError>;
    pub fn segments(&self) -> &[Segment];
}
pub enum TemplateError {
    UnmatchedOpen { pos: usize }, UnmatchedClose { pos: usize },     // lone '{' or '}'
    EmptyPlaceholder { pos: usize },
    UnknownPlaceholder { pos: usize, text: String },                 // e.g. {foo} in a table entry
    WrongContext { pos: usize, text: String },                       // {table:x} in generator template, etc.
    BadId { pos: usize, text: String },
    BadDice { pos: usize, source: DiceError },
}
```
`{{` and `}}` are literal braces. **Substitution is single-pass: substituted text is never re-scanned** (D-10), so a user table entry cannot inject placeholders through another entry's output and literal braces survive nested resolution. Identifier syntax: `[a-z0-9][a-z0-9_-]{0,63}` for item ids; slot keys `[a-z_][a-z0-9_]{0,31}`.

### 3.6 Tables: `table.rs` (spec §5.4)

```rust
pub struct Table {
    pub id: String, pub name: String, pub category: String, pub tags: Vec<String>,
    pub description: Option<String>,
    pub dice: DiceExpr,                 // list tables: 1dN derived from items.len()
    pub form: TableForm,                // Ranged | List (kept for display)
    pub entries: Vec<TableEntry>,       // sorted by lo; validated gap/overlap free
}
pub struct TableEntry { pub lo: i64, pub hi: i64, pub text: Template, pub raw: String }
pub enum TableForm { Ranged, List }

pub struct TableRoll {
    pub table_id: String, pub dice: DiceRoll, pub entry_index: usize,
    pub raw: String,                    // entry text before resolution
    pub text: String,                   // fully resolved
    pub refs: Vec<ResolvedRef>,         // detail of {table:..} / {roll:..} in order
}
pub enum ResolvedRef { Table(TableRoll), Dice(DiceRoll) }
```

Validation (all errors name the entry index and/or value): `EmptyTable`, `EmptyText{entry}`, `InvalidRange{entry}` (lo > hi), `Gap{from,to}`, `Overlap{entry}`, `OutOfRange{entry,range,allowed}`, `BadD66Value{entry,value}` (tens or units digit not 1..=6), `ListSize{got}` (not 2..=1000), `ConflictingFields` (`items` with `entries`/`dice`), `MissingDice`, `MissingName`, `BadId`, `UnsupportedDice` (D66 inside a compound expression), `BadDice(DiceError)`, `BadTemplate{entry, source}`, `Toml(String)` (parse, unknown key, wrong type).

Coverage rule (D-11): for `Totals::Contiguous{min,max}` every integer in `min..=max` is covered exactly once; for `Totals::D66` the 36 values with both digits in 1..=6 are covered exactly once and ranges that include other numbers (e.g. `[11, 66]`) are rejected by `BadD66Value` naming the first bad value. Ranges are `i64` (an expression like `1d6-2` has min -1).

Rolling: `dice.roll(roller)`; entry found by binary search on `lo`; entry text rendered by walking segments: `Literal` copies, `Roll` evaluates, `TableRef` recurses with depth+1. Depth > 5 returns `EvalError::DepthExceeded` (defensive; load-time checks make it unreachable for catalog content).

### 3.7 Generators: `generator.rs` (spec §5.5)

```rust
pub struct Generator {
    pub id: String, pub name: String, pub category: String, pub tags: Vec<String>,
    pub description: Option<String>,
    pub template: Template, pub slots: Vec<Slot>,
}
pub struct Slot {
    pub key: String, pub source: SlotSource, pub count: Count,
    pub distinct: bool /*default true*/, pub join: String /*default ", "*/,
    pub list: bool, pub chance: u8 /*1..=100, default 100*/,
}
pub enum SlotSource { Table(String), Generator(String), Dice(DiceExpr) }
pub enum Count { Fixed(u32), Dice(DiceExpr) }          // validated: max <= 50 (D-14)

pub struct GeneratorResult {
    pub id: String,
    pub text: String,                          // == concat of segments' text
    pub segments: Vec<RenderedSegment>,        // lets the UI highlight slot values
    pub slots: Vec<SlotResult>,                // declaration order
    pub template_rolls: Vec<DiceRoll>,         // {roll:..} in the template, kept on slot re-roll (D-15)
}
pub struct RenderedSegment { pub text: String, pub slot: Option<usize> }   // slot index when text came from a slot
pub struct SlotResult {
    pub key: String,
    pub source: SlotSourceRef,                 // kind + id or expression, for display
    pub present: bool,                         // false when chance failed
    pub count_roll: Option<DiceRoll>,
    pub values: Vec<String>,                   // spec: values
    pub detail: Vec<ValueDetail>,              // spec: detail (parallel to values)
}
pub enum ValueDetail { Table(TableRoll), Generator(Box<GeneratorResult>), Dice(DiceRoll) }
```

**Evaluation order for one slot** (D-12; fixed so scripted tests are exact):
1. If `chance < 100`: `roll(100)`; present iff roll <= chance. Absent => values empty, nothing else rolled.
2. Count: `Fixed(n)` consumes no roll; `Dice(expr)` rolls expr; result clamped to `0..=50` (load check prevents upper overflow).
3. For each of `count` values: roll the source. Table with `distinct` and count > 1: a duplicate is detected by **entry index**, not text; a duplicate triggers a re-roll, at most 10 rolls per value in total (including the first); if all 10 collide the value is dropped ("accept fewer"). `distinct` is ignored for generator and dice slots.
4. Rendering of values: `list` => each value as `- {value}` joined with `\n` (no leading or trailing newline; template author controls surrounding newlines); otherwise values joined with `join`. `list` wins over `join`.
5. After all slots, each `{roll:expr}` in the template is evaluated once, left to right, and stored in `template_rolls`.
6. Segments: template literals, slot substitutions (tagged with their slot index), template roll totals. Substitution is single-pass (D-10).

**Slot re-roll** (`reroll_slot`): replaces exactly one `SlotResult`, **forces the slot present** (the user asked for it, D-13; `chance` is not re-applied), then re-renders the template from stored slot values and stored `template_rolls`. Other slots and template rolls are bit-identical to before. Nested-generator slots re-roll the whole nested generator.

Depth: nesting depth counts generator-in-generator; root is 0, max 5 (D-16). Table-in-table depth (A§3.6) is counted separately and restarts at 0 for each table rolled by a slot.

### 3.8 Catalog and the load pipeline: `catalog/` (spec §5.7)

```rust
pub enum ItemKind { Table, Generator }                 // separate id namespaces (D-17)
pub struct Source { pub kind: ItemKind, pub origin: String, pub file_stem: String, pub text: String }
pub struct LoadWarning { pub origin: String, pub item: Option<(ItemKind, String)>, pub kind: WarningKind, pub message: String }
pub enum WarningKind { Parse, Invalid, Replaced, DuplicateUserId, Cycle, MissingReference, DepthExceeded,
                       UnusedSlot, IgnoredField, FileUnreadable, FileTooLarge }

pub struct Catalog { /* BTreeMap<String, Table>, BTreeMap<String, Generator>, ordered summaries */ }
pub struct ItemSummary { pub kind: ItemKind, pub id: String, pub name: String, pub category: String,
                         pub tags: Vec<String>, pub description: Option<String> }
pub struct Filter { pub text: Option<String>, pub category: Option<String>, pub kind: Option<ItemKind> }

impl Catalog {
    pub fn build(builtin: &[Source], user: &[Source]) -> (Catalog, Vec<LoadWarning>);   // never fails
    pub fn table(&self, id: &str) -> Option<&Table>;
    pub fn generator(&self, id: &str) -> Option<&Generator>;
    pub fn items(&self) -> &[ItemSummary];                         // sorted: category, then name (case-insens.), then id
    pub fn categories(&self) -> Vec<String>;                       // sorted
    pub fn search(&self, f: &Filter) -> Vec<&ItemSummary>;         // text = case-insens. substring over id,name,category,tags
    pub fn roll_table(&self, id: &str, r: &mut dyn Roller) -> Result<TableRoll, EvalError>;
    pub fn generate(&self, id: &str, r: &mut dyn Roller) -> Result<GeneratorResult, EvalError>;
    pub fn reroll_slot(&self, prev: &GeneratorResult, slot: usize, r: &mut dyn Roller)
        -> Result<GeneratorResult, EvalError>;
}
pub enum EvalError { UnknownTable(String), UnknownGenerator(String), UnknownSlot(usize), DepthExceeded }
```

**Pipeline** (every stage appends warnings; none returns `Err`):

1. **Parse + local validation** per source: TOML => raw serde structs (`#[serde(deny_unknown_fields)]`, D-18) => `Table`/`Generator`. `id` defaults to the file stem and must match the id syntax. Failures drop that item only.
2. **Merge by (kind, id).** Built-ins first (sorted by id). User sources are applied in filename order. A valid user item replaces the built-in of the same kind and id (`Replaced` warning is informational, shown in the Library). Two user files with the same (kind, id): the later filename wins with a `DuplicateUserId` warning (D-19).
3. **Graph validation to fixpoint.** Edges: table => `{table:id}` targets; generator => slot `table`/`generator` targets. Repeat until no change:
   - *Dangling reference*: item references an id that does not exist or was removed => item removed (`MissingReference`). This also handles tables referencing missing tables, which the spec leaves open (D-20).
   - *Cycle* (strongly connected component of size > 1, or self-loop): if the cycle contains any user-origin item, **all user-origin members are removed and any built-in they replaced is restored**; if it contains only built-ins (cannot happen for shipped content, tested) or only user items, all members are removed. (D-21: a user override must never take a built-in generator out of the Library.)
   - *Depth*: generator whose nesting height (longest chain of generator slots below it) exceeds 5 is removed (`DepthExceeded`); same user-first preference.
   - Static check: every `{table:..}` chain is also bounded by depth 5.
4. **Template/slot cross-checks** (local, stage 1): every `{key}` in the template has a slot (error); duplicate slot keys (error); a slot not used in the template (warning `UnusedSlot`, slot kept); `count` dice max > 50 (error); `chance` outside 1..=100 (error); slot with zero or multiple of `table`/`generator`/`dice` (error); `distinct` on a non-table slot or `join` together with `list` (warning `IgnoredField`).
5. **Index** summaries, sorted for stable Library order.

The pipeline is deterministic and order-independent of the OS: callers pass `Source`s already sorted by the reader (A§5.3).

### 3.9 Journal: `journal/` (spec §5.6)

```rust
#[derive(Serialize, Deserialize)]
pub struct Entry {
    #[serde(default = "one")] pub v: u8,                  // schema version, additive (D-22)
    pub ts: u64,
    pub kind: Kind,                                       // "oracle" | "table" | "generator" | "dice" | "note"
    pub input: String,
    pub result: String,                                   // exactly what headless commands print
    #[serde(default)] pub detail: serde_json::Value,      // informational, write-mostly (D-23)
}
pub enum Kind { Oracle, Table, Generator, Dice, Note }    // serde lowercase

pub struct JournalLoad { pub entries: Vec<Entry>, pub skipped: Vec<SkippedLine> }
pub struct SkippedLine { pub line_no: usize, pub reason: String }

pub fn encode_line(e: &Entry) -> Result<String, JournalError>;      // one line, no trailing "\n"
pub fn decode(text: &str) -> JournalLoad;                           // skips blank lines silently, malformed with report
pub fn export_markdown(session: &str, entries: &[Entry]) -> String;   // pure
pub fn format_utc(ts: u64) -> String;                               // "YYYY-MM-DD HH:MM", hand-written civil-from-days, no time crate (D-24)
```

Per-kind field conventions (`input` / `result` / `detail` keys):

| kind | `input` | `result` | `detail` |
|---|---|---|---|
| dice | expression as typed, trimmed | `3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5` | serialized `DiceRoll` |
| oracle | `Even` or `Even: <question>` (label, then question when present) | line 1 `Even: Yes (d100: 11)`; line 2 (only with twist) `Twist: <text>` or `Twist: [unavailable: <missing>]` | `{likelihood, roll, outcome, question, twist}` |
| table | table id | resolved text | serialized `TableRoll` |
| generator | generator id; slot re-roll: `<id> [slot <key>]` | full rendered text | serialized `GeneratorResult`; re-roll adds `rerolled_slot` |
| note | empty | the note text | `null` |

Markdown export (exact, snapshot-tested; D-25):
```
# Session: {session}
<blank>
- {YYYY-MM-DD HH:MM} UTC · {kind} · {input} → {first result line}
  {continuation result lines, each indented 2 spaces}
<blank line before and after every note>
{YYYY-MM-DD HH:MM} UTC — note: {text}
<blank>
```
Empty `input` omits the `{input} →` part. Consecutive non-note entries form one contiguous bullet list. Entries keep file order. No Markdown escaping (the journal is the user's own text).

### 3.10 Engine: `engine.rs`

The single orchestration point shared by CLI and TUI.

```rust
pub struct Engine {
    catalog: Rc<Catalog>,
    roller: Box<dyn Roller>,
    clock: Box<dyn Clock>,
    twist_generator: String,                 // default "event" (D-26)
}
pub enum RollResult { Dice(DiceRoll), Oracle(OracleResult), Table(TableRoll), Generator(GeneratorResult) }
pub struct Rolled { pub result: RollResult, pub entry: Entry }

impl Engine {
    pub fn new(catalog: Rc<Catalog>, roller: Box<dyn Roller>, clock: Box<dyn Clock>) -> Self;
    pub fn with_twist_generator(self, id: impl Into<String>) -> Self;
    pub fn catalog(&self) -> &Rc<Catalog>;
    pub fn roll_dice(&mut self, expr: &str) -> Result<Rolled, DiceError>;
    pub fn ask(&mut self, l: Likelihood, question: Option<&str>) -> Rolled;
    pub fn roll_table(&mut self, id: &str) -> Result<Rolled, EvalError>;
    pub fn generate(&mut self, id: &str) -> Result<Rolled, EvalError>;
    pub fn reroll_slot(&mut self, prev: &GeneratorResult, slot: usize) -> Result<Rolled, EvalError>;
    pub fn note(&self, text: &str) -> Entry;
}
```
`Rc` is deliberate: single-threaded program, no async runtime (§3).

### 3.11 Built-in content: `builtin.rs`

```rust
pub const TABLES: &[(&str, &str)]     = &[("event_focus", include_str!("../../../content/tables/event_focus.toml")), /* ... */];
pub const GENERATORS: &[(&str, &str)] = &[ /* ... */ ];
pub fn sources() -> Vec<Source>;        // sorted by file stem
```
`include_str!` resolves at compile time, so core still performs no file I/O. There is no `build.rs`; a test compares the lists to the `content/` directory so a forgotten file fails CI (D-27). The `../../../` path leaves the crate directory, which is fine for a workspace build and means `oraculum-core` is not publishable to crates.io as-is (accepted, D-27).

### 3.12 Core error model

| type | where | surfaced as |
|---|---|---|
| `DiceError` | parse | CLI exit 2 / inline error in Dice tab |
| `TemplateError`, `TableError`, `GeneratorError` | local validation | converted to `LoadWarning` (never propagated) |
| `EvalError` | rolling | `Unknown*` => CLI exit 2 / toast; `DepthExceeded` => toast (unreachable for validated content) |
| `JournalError` | encode only | storage layer maps to `StorageError` |

All derive `thiserror::Error` + `Debug`, `Clone`, `PartialEq` (so tests compare them). Public functions in core return these types, never `anyhow` and never panic.

---

## 4. Data formats

### 4.1 Table file (TOML; spec §5.4)

Ranged: `id?`, `name`, `category?`, `tags?`, `description?`, `dice`, `[[entries]] range=[lo,hi], text`. List: `id?`, `name`, `category?`, `tags?`, `description?`, `items=[...]`. Rules and refinements:
- `name` required. `category` defaults to `"Uncategorized"`, `tags` to `[]` (D-28).
- Unknown keys are errors naming the key (typo protection, D-18).
- `items` together with `dice` or `entries` is an error; none of `items`/`entries` is an error.
- Entry text: `{table:id}`, `{roll:expr}`, `{{`, `}}` only.

### 4.2 Generator file (TOML; spec §5.5)

As in the spec. Refinements: `count` is an integer or a dice string (`serde(untagged)`); exactly one of `table`/`generator`/`dice` per slot; keys unique; `chance` 1..=100; template may use `{key}`, `{roll:expr}`, `{{`, `}}`. A generator with no slots is valid (static text) but emits an `IgnoredField`-class info only if the template contains placeholders.

### 4.3 Journal line (JSONL)

One JSON object per line, UTF-8, `\n` terminated. Unknown extra fields on load are ignored; an unknown `kind` makes the line malformed (skipped and reported). Example:
`{"v":1,"ts":1762484400,"kind":"dice","input":"2d6+1","result":"2d6+1 => [3, 5] +1 = 9","detail":{...}}`

### 4.4 Data directory

```
<data-dir>/tables/*.toml        user tables
<data-dir>/generators/*.toml    user generators
<data-dir>/sessions/<name>.jsonl
<data-dir>/export.md
```
Directories are created on first write, never on read. Only `*.toml` files directly in the folder are read (case-insensitive extension; dotfiles and `*~` ignored; not recursive).

---

## 5. `oraculum` (binary crate)

### 5.1 Entry and exit codes

`main.rs` (the only place `anyhow` appears): parse args with clap, build `SystemClock`, resolve seed, call `cli::run` or `tui::run`, map errors to exit codes:

| code | meaning |
|---|---|
| 0 | success |
| 1 | environment failure (I/O, no data dir, cannot write journal) |
| 2 | invalid input or usage (clap usage errors, bad dice expression, unknown id, bad session name) |

Error text goes to stderr as `oraculum: <message>`; warnings go to stderr as `warning: <origin>: <message>`. Running the TUI when stdout or stdin is not a terminal (`std::io::IsTerminal`) prints a hint and exits 2.

### 5.2 CLI: `cli/` (spec §8)

```rust
#[derive(Parser)] pub struct Cli {
    #[arg(long, global = true)] pub data_dir: Option<PathBuf>,
    #[arg(long, global = true, default_value = "default")] pub session: String,
    #[arg(long, global = true)] pub seed: Option<u64>,
    #[command(subcommand)] pub command: Option<Command>,           // None => TUI
}
pub enum Command { Roll { expr: String }, Ask { likelihood: LikelihoodArg, question: Option<String> },
                   Table { id: String }, Gen { id: String },
                   List { #[arg(long)] category: Option<String>, #[arg(long)] search: Option<String> },
                   Export { #[arg(long)] out: Option<PathBuf> } }

pub struct Env<'a> { pub data_dir: DataDir, pub session: SessionName, pub seed: u64,
                     pub clock: Box<dyn Clock>, pub out: &'a mut dyn Write, pub err: &'a mut dyn Write }
pub fn run(cmd: Command, env: Env<'_>) -> Result<(), AppError>;
```
`run` takes writers so most CLI tests run in-process; `tests/cli.rs` additionally uses `assert_cmd` for the real binary.

Command behavior:
- **Content loading is lazy:** `ask`, `table`, `gen`, `list` and the TUI load content (built-in + user) and print warnings to stderr; `roll` and `export` do not need content and print none (D-29).
- Each invocation builds a fresh `RngRoller::from_seed(seed)`; so `--seed 7 roll 1d20` is identical every time (D-30, confirmed by the human).
- Output: stdout gets `entry.result` plus a newline, byte-identical to the journal's `result`. `list` prints one row per item: `{T|G}  {id}  {category}  {name}` with columns padded to the widest value; same `Filter` as the Library.
- `ask` kebab names map via clap `ValueEnum` to `Likelihood`; the optional question is the second positional.
- `export`: reads the session journal, writes Markdown to `--out` (default `<data-dir>/export.md`) via write-to-temp-then-rename, prints the path on stdout. Malformed journal lines are reported on stderr.
- Every successful roll appends to `<data-dir>/sessions/<session>.jsonl`.

### 5.3 Storage: `storage/`

```rust
pub struct DataDir { root: PathBuf }
impl DataDir {
    pub fn resolve(flag: Option<PathBuf>) -> Result<DataDir, StorageError>;   // directories::ProjectDirs("", "", "oraculum"); None => error asking for --data-dir
    pub fn tables_dir(&self) -> PathBuf; pub fn generators_dir(&self) -> PathBuf;
    pub fn session_path(&self, s: &SessionName) -> PathBuf; pub fn export_path(&self) -> PathBuf;
}
pub struct SessionName(String);                    // ^[A-Za-z0-9_-]{1,64}$ ; anything else => invalid input (D-31)
impl SessionName { pub fn parse(s: &str) -> Result<Self, StorageError>; }

pub struct JournalStore { path: PathBuf }
impl JournalStore {
    pub fn load(&self) -> Result<JournalLoad, StorageError>;     // missing file => empty
    pub fn append(&self, e: &Entry) -> Result<(), StorageError>;
}
pub fn load_user_sources(dir: &DataDir) -> (Vec<Source>, Vec<LoadWarning>);
pub fn write_export(path: &Path, markdown: &str) -> Result<(), StorageError>;
pub struct SystemClock;                              // in clock.rs
```
`append`: create dirs if needed; open with `OpenOptions::append`; if the file is non-empty and its last byte is not `\n` (a crash left a partial line) write `\n` first; then **one** `write_all` of `line + "\n"` (D-32, satisfies "one write per appended entry" and keeps concurrent appenders from interleaving mid-line in practice). `load_user_sources`: reads `tables/` then `generators/`, each sorted by file name (byte order); unreadable, non-UTF-8, or >1 MiB files become `FileUnreadable`/`FileTooLarge` warnings (D-35).

`StorageError` (thiserror): `NoDataDir`, `InvalidSessionName(String)`, `Io{path, source}`, `Journal(JournalError)`.

### 5.4 TUI: `tui/` (spec §4, §7)

#### Terminal lifecycle
`terminal.rs` owns a `TerminalGuard`: `enter()` enables raw mode and the alternate screen (no mouse capture, non-goal); `Drop` and a **chained panic hook** both call an idempotent `restore()`. The hook is installed before `enter()` and calls the previous hook after restoring so the panic message prints on a normal screen. The guard hides raw terminal ops behind a small `TerminalOps` trait so a mock can verify restore-on-drop and restore-on-unwind (A§9). Event loop: `crossterm::event::poll(250 ms)` then `read()`; no async, no threads. A 250 ms tick expires toasts. Key events are processed only for `KeyEventKind::Press` and `Repeat`. Minimum size 60x20; smaller shows a "terminal too small" message instead of tabs (D-36).

#### Types

```rust
pub enum TabId { Oracle, Library, Dice, Journal }           // order = keys 1..4

pub trait Tab {
    fn id(&self) -> TabId;
    fn title(&self) -> &'static str;
    fn update(&mut self, key: KeyEvent) -> Action;                  // spec signature kept
    fn render(&self, frame: &mut Frame, area: Rect);                // spec signature kept
    fn captures_input(&self) -> bool;                               // a text field is focused
    fn on_event(&mut self, ev: &TabEvent);                          // App => tab only
    fn help(&self) -> &'static [(&'static str, &'static str)];      // (keys, description) for '?'
}

pub enum Action {
    None,
    SwitchTab(TabId),
    Quit,
    Roll(RollRequest),                  // App performs it via Engine, journals it, answers with TabEvent
    AppendNote(String),                 // the spec's "append journal entry" for notes
    Toast(Toast),
    OpenInLibrary(LibraryTarget),       // spec's "open item in Library"
}
pub enum RollRequest {
    Oracle { likelihood: Likelihood, question: Option<String> },
    Dice { expr: String },
    Table { id: String },
    Generator { id: String },
    RerollSlot { previous: Box<GeneratorResult>, slot: usize },
}
pub enum LibraryTarget { Item { kind: ItemKind, id: String }, Category(String) }

pub enum TabEvent {
    Rolled(Box<RollResult>),            // to the tab that emitted the Roll
    RollFailed(String),                 // message ready to show
    JournalLoaded(Vec<Entry>, Vec<SkippedLine>),   // to Journal tab at startup
    JournalAppended(Entry),             // to Journal tab after every append
    JournalWriteFailed(String),
    Focus(LibraryTarget),               // to Library after OpenInLibrary
}

pub struct Toast { pub text: String, pub level: ToastLevel, pub expires_at: u64 }   // 3 s via Clock
```

**Why intent-actions (D-37):** the spec's `update(&mut self, KeyEvent) -> Action` has no access to a `Roller`, `Clock`, or journal, and the spec also says tabs never read each other's state. Making tabs pure state machines that *request* rolls keeps the spec's two signatures exactly, lets `update` tests run with no RNG, and puts rolling, journaling and error handling in one place (`App`). The only addition to the spec's Action list is `Roll(..)`; "append journal entry" is realized as `AppendNote` plus the append that follows every `Roll`.

**App** (`app.rs`):
```rust
pub struct App {
    engine: Engine, store: JournalStore, active: TabId,
    oracle: OracleTab, library: LibraryTab, dice: DiceTab, journal: JournalTab,
    toast: Option<Toast>, help_open: bool, warnings_len: usize, seed: u64,
}
impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> bool;     // returns false when quitting
    pub fn tick(&mut self);                                  // expire toast
    pub fn render(&self, frame: &mut Frame);
}
```
`handle_key` order: (1) help open => any key closes it. (2) `Tab`/`Shift-Tab` always switch tabs, even while editing (D-38). (3) If the active tab does not capture input: `q` or `Ctrl-C` quits; `1`-`4` switch; `?` opens help. (4) If it captures input: `Ctrl-C` is delivered to the tab as `Esc` (cancels the edit; a second `Ctrl-C` then quits) (D-39). (5) Everything else goes to `tab.update`. The returned `Action` is executed: `Roll` => `Engine` call => `store.append` (failure => entry is still shown in the Journal tab, toast "journal write failed: ...", app continues) => `TabEvent::Rolled` to the originator and `TabEvent::JournalAppended` to the Journal tab. Tabs never see each other; they share only the immutable `Rc<Catalog>` for listing and filtering.

#### Tab behavior (keys exactly as spec §7 unless noted)

**Oracle.** State: likelihood, question (`LineInput`), editing flag, `last: Option<OracleResult>`, in-session history (10). Keys: `←/→`/`h/l` likelihood; `/` edit question (`captures_input`); `Esc` finish editing; `Enter` roll (while editing: finish editing then roll); `t` => `OpenInLibrary(Category("Oracle"))` (D-40). Rendering: outcome colored green (Yes), red (No), bold for exceptional; the outcome label is always printed so color is never the only signal; roll shown as `d100: N`; twist shown as a generator result block (segments highlighted).

**Library.** State: `Rc<Catalog>`, filter text, category filter (`None` => all), rows (category headers + items; headers are not selectable), selection, `view: Detail | Entries`, `last: Option<LibraryResult>` (Table or Generator with the item it came from), `slot_focus: Option<usize>`, `warnings_open`, scroll offsets, modes `Browse | Filter | Slot`. Keys:
- `/` => Filter mode (`captures_input`); `Enter` keeps the filter and returns to Browse; `Esc` clears the text and returns to Browse. In Browse, `Esc` clears text and category filter. `c` cycles `All -> categories alphabetical -> All`.
- `↑/↓`, PgUp/PgDn move selection; `v` toggles entries view for a table (scrollable).
- `Enter` => `Roll` of the selected item. `r` => `Roll` of the item that produced `last` (toast if none).
- `s` => Slot mode on the latest generator result (toast if none); `↑/↓` select slot; `Enter` => `RerollSlot`; `Esc` leaves Slot mode.
- `w` toggles the loader-warnings panel (key not given by the spec, D-41).
- `Focus(target)` event selects that item, or sets the category filter.
Rendering: left pane list `T`/`G` marker + name under category headers; right pane detail or entries; generator results rendered from `segments`, slot values highlighted, the focused slot emphasized; warnings panel at the bottom when open, with count always shown in the title bar.

**Dice.** State: `LineInput`, mode `Edit | Browse`, `error: Option<String>`, history (10, newest first, each with full `DiceRoll` display). Starts in Edit (otherwise `q`/`1`-`4` would be dead inside an always-focused input, D-42). `Enter` parses with `DiceExpr::parse` locally: error => inline message, no roll, nothing journaled; success => `Roll(Dice)`. `Esc` => Browse (global keys active); in Browse `/`, `i` or `Enter` re-enters Edit.

**Journal.** State: `Vec<Entry>` (loaded at startup then appended live), selection/scroll (follows the end until the user scrolls up), `note: Option<LineInput>`, `skipped` count. Keys: `↑/↓`, PgUp/PgDn; `n` note (`captures_input`), `Enter` => `AppendNote(text)` (empty text ignored), `Esc` cancels; `x` => export (the App performs it; `Toast` with the path). A startup toast reports skipped malformed lines.

#### Layout (80x24 reference)
```
 1 Oracle  2 Library  3 Dice  4 Journal            seed 8231905  warnings 2   <- row 0: tab bar
+---------------------------------------------------------------------------+
|                         active tab body                                   |
+---------------------------------------------------------------------------+
 toast / key hints                                                            <- last row
```
Library body: left 40% list, right 60% detail; bottom panel (4 rows) when warnings are open. Help overlay is a centered modal listing global keys and the active tab's `help()`.

#### Text input
`input.rs`: `LineInput { text: String, cursor: usize /*char index*/ }` with insert, backspace, delete, left/right, home/end. No third-party input crate (D-34). Wide-glyph cursor alignment is a known limitation.

---

## 6. Determinism rules (checklist for reviewers)

- Only `Roller` produces randomness; `RngRoller` consumes `next_u32` through our rejection sampler.
- `BTreeMap`/sorted `Vec` everywhere output order matters; no `HashMap`/`HashSet` iteration in core.
- File lists are sorted before use; the catalog never reads the file system.
- Time is read only through `Clock` (journal `ts`, toast expiry). `Instant` is not used.
- Roll-order contracts are written down in A§3.3 (dice), A§3.4 (oracle), A§3.7 (slots) and covered by `ScriptedRoller` tests that assert the exact `(sides, returned)` call sequence.
- Golden test: fixed seeds produce pinned outputs for one command per kind; if a dependency upgrade changes them, CI fails loudly.
- Journal `ts` is excluded from reproducibility claims; CLI tests normalise it (D-43).

---

## 7. Built-in content plan (spec §6)

15 generators and about 69 tables; every list table has at least 12 items (names and occupations 40+; given/family names 60+). All text original and setting-neutral, with a provenance header comment in each file ("written for Oraculum, original"). Reuse as specified: `atmosphere`, `terrain`, `creature_form`, `place_adjective`, `place_noun`, `given_names`, `family_names`.

| generator (category) | slots => source |
|---|---|
| `npc` (Characters) | given_names, family_names, npc_age, npc_occupation, npc_appearance, npc_trait x2 (distinct), npc_motivation, npc_secret (30%) |
| `creature` (Creatures) | creature_form, creature_trait, creature_ability, creature_weakness, creature_behavior, creature_lair |
| `encounter` (Adventure) | terrain, number (dice 1d6), creature_form, encounter_activity, encounter_disposition, twist => gen `event` (25%) |
| `settlement` (Places) | place_adjective + place_noun (name), settlement_size, settlement_government, settlement_feature, settlement_trade, settlement_problem, leader => gen `npc` |
| `tavern` (Places) | place_adjective + place_noun, proprietor => gen `npc`, atmosphere, tavern_specialty, tavern_clientele, rumor => gen `rumor` (60%) |
| `wilderness_location` (Places) | terrain, wild_landmark, wild_feature, wild_danger, wild_resident (50%) |
| `dungeon` (Sites) | dungeon_type, dungeon_origin, dungeon_state, dungeon_entrance, dungeon_inhabitant x2, hazard => gen `trap` (60%), treasure => gen `treasure`, rooms => gen `dungeon_room` (count `1d4+3`, list) |
| `dungeon_room` (Sites) | room_shape, room_feature, room_contents, room_exit (count `1d3`), atmosphere |
| `trap` (Sites) | trap_trigger, trap_mechanism, trap_effect, trap_sign, trap_severity |
| `quest` (Adventure) | quest_patron, quest_goal, quest_target, quest_complication, quest_reward, quest_stakes, location name (place_adjective + place_noun) |
| `rumor` (Adventure) | rumor_source, rumor_claim, rumor_veracity |
| `treasure` (Items) | coins (dice slot), treasure_valuable (count `1d2`), magic item => gen `magic_item` (25%) |
| `magic_item` (Items) | item_base, item_material, item_quality, item_power, item_drawback (40%), item_history |
| `faction` (Organizations) | faction_form, faction_theme, faction_goal, faction_method, faction_resource, leader => gen `npc` |
| `event` (Oracle) | event_focus (d100 ranged), meaning_action (list, 20), meaning_subject (list, 20) |

Built-in files never use `{{`/`}}`, so the "no unresolved placeholders" test can reject any `{` or `}` in rendered output. Deepest built-in nesting: `dungeon` => `treasure` => `magic_item` (depth 2). Originality sign-off is by the human (D-46).

---

## 8. Error and warning flow (summary)

```
user file -> Source -> Catalog::build -> LoadWarning[]  -> CLI stderr / Library warnings panel
journal line -> decode -> SkippedLine[]                  -> CLI stderr (export) / Journal toast
bad dice text -> DiceError                               -> exit 2 / inline error in Dice tab
unknown id    -> EvalError                               -> exit 2 / toast
append failure-> StorageError                            -> exit 1 / toast; app keeps running
panic         -> hook restores terminal, then default panic output
```

---

## 9. Test plan

### 9.1 Gates
| gate (spec §3) | how it is satisfied |
|---|---|
| `cargo fmt --check` | CI step; `rustfmt.toml` not customised |
| `cargo clippy --all-targets -- -D warnings` | CI step; workspace lints in A§2 make `unwrap`/`expect`/`panic` hard errors outside tests |
| `cargo test` | CI step on `macos-14` (Apple Silicon) and `ubuntu-latest`; also satisfies "builds and runs on both" |

### 9.2 Test map (spec §9 => where)
| spec area | location | key cases |
|---|---|---|
| Dice | `oraculum-core/src/dice/*` unit + `tests/dice_props.rs` | table-driven valid/invalid parses (incl. `d66`, `1d66`, `d66kh1`, whitespace, `-3`, limits, leading zeros); `ScriptedRoller` cases for kh/kl/ties/negatives/constants/d66; display strings; proptest: dice in 1..=M, total = kept dice + constants, `parse(canonical(x)) == x`, arbitrary strings never panic |
| Oracle | `src/oracle.rs` unit + `tests/oracle_exhaustive.rs` | the 10 worked examples verbatim; for each likelihood iterate 1..=100: yes count = chance, exceptional counts = formula, yes monotonic over likelihood, exactly 9 twists; spec table equals formula; `ask` consumes 1 roll without twist; missing event generator => `Twist::Unavailable` |
| Tables | `src/table.rs`, `tests/tables.rs` | every error class: gap, overlap, out of range, bad d66 digit, empty, conflicting fields, list size; inline refs, `{{ }}`, single-pass substitution, depth limit 5, cycle detection, dangling refs |
| Generators | `src/generator.rs`, `tests/generators.rs` | scripted tests of `count`, `distinct` (attempt limit, accept fewer), `join`, `list`, `chance` (both sides), nested; load validation (missing refs, placeholder without slot, unused slot warning, bad dice); single-slot re-roll leaves other slots and template rolls identical, forces presence; user override that creates a cycle reverts to built-in; fixpoint cascade |
| Built-in content | `tests/builtin.rs` | zero load warnings; every generator renders for seeds 0..500 with no empty value, no `{`/`}`, non-empty text; list tables >= 12 items; every item has name, category, tags; `builtin.rs` lists equal the `content/` directory; 15 required generators present |
| Journal | `src/journal/*`, `oraculum/tests/storage.rs` | encode/decode round trip; malformed lines reported with line numbers; partial last line then append; markdown export `insta` snapshot (with notes, multi-line results, empty input); `format_utc` against known epochs incl. leap day and 1970/2038 edges |
| Storage | `oraculum/tests/storage.rs` (`tempfile`) | session name validation (`../x`, empty, too long); missing dirs; non-UTF-8 file; >1 MiB file; sorted load order; export temp+rename |
| CLI | `oraculum/tests/cli.rs` (`assert_cmd` + in-process `run`) | each subcommand with `--seed` and temp `--data-dir`: stdout equals journal `result`; stderr warnings; exit codes 0/1/2; journal contents (ts masked); same seed twice gives same stdout; `roll`/`export` print no content warnings |
| TUI logic | `oraculum/tests/tui.rs` + unit | synthetic `KeyEvent`s: likelihood change; invalid dice shows error and emits no `Roll`; `q` while typing does not quit; `Tab` works while typing; `Ctrl-C` while typing cancels edit then quits; Library filter by id/name/category/tag; `c` cycling; `Enter` vs `r`; slot focus and `RerollSlot` action; App-level test with `ScriptedRoller` + temp store verifying roll => journal append => tab event; write failure => toast and app alive |
| TUI render | `TestBackend` | per tab at 80x24 assert key strings (outcome label, `d100:`, `T`/`G` markers, slot values, help overlay, "terminal too small"); one `insta` snapshot per tab as regression |
| Terminal safety | `tests/tui.rs` | `TerminalGuard` with mock ops: restore on drop and on unwind (`catch_unwind`); `restore()` idempotent |
| Hard constraints | `deny.toml` check in CI | no C `-sys` crates, no async runtime, no network crates |

### 9.3 Fuzz-lite
`proptest` feeds arbitrary byte strings to `DiceExpr::parse`, `Template::parse`, the TOML loaders and `Catalog::build` (random mutations of a valid file), and `journal::decode`; the properties are "does not panic" and "returns warnings, never `Err`/abort".

### 9.4 Non-spec CI additions (flagged, D-44)
`cargo deny check bans` with `deny.toml` banning `openssl`, `openssl-sys`, `libsqlite3-sys`, `git2`, `libgit2-sys`, `tokio`, `async-std`, `reqwest`, `hyper`. This is a tool run in CI, not a dependency. It makes spec §3 "no C deps / no async / no network" mechanically checkable. Advisory if the human prefers only the three spec gates.

### 9.5 Manual QA checklist (per release candidate)
Terminals: macOS Terminal and iTerm2, Linux gnome-terminal/xterm and `tmux`. Check: resize during use, 60x20 minimum message, Ctrl-C in each mode, quit restores the screen and cursor, forced panic (local patch adding `panic!()`) leaves a usable shell, key table in help matches behavior, journal survives `kill -9` mid-session, broken user TOML shows a warning but app runs, content originality spot-check (D-46 review packet).

---

## 10. Build order and milestones

Sizes are *estimates* in ideal developer-days for one developer who knows Rust; QA works one milestone behind. Not commitments (the spec says nothing about schedule). Each milestone ends green on the three gates.

| # | Milestone | Dev | Deliverable and acceptance (QA verifies) |
|---|---|---|---|
| M0 | Scaffold and gates | 1 | workspace, lints, `clippy.toml`, `deny.toml`, CI on macOS+Linux, empty crates compile, versions pinned. QA: CI green on both OSes, a deliberate `unwrap()` fails CI |
| M1 | Rng, clock, dice | 2 | `Roller` impls with golden seed vectors, `DiceExpr` parse/roll/display, proptests. QA: review grammar edge cases against A§3.3 |
| M2 | Oracle | 1 | `evaluate`, exhaustive tests, `ask` with stub catalog. QA: re-derive 10 worked examples by hand |
| M3 | Templates and tables | 3 | `Template`, `Table` validation (all error classes), rolling with refs and depth. QA: write bad-table corpus |
| M4 | Generators and catalog | 4 | slots, nested, segments, re-roll, `Catalog::build` pipeline with cycle/dangling/override handling, `Filter`. QA: adversarial user files |
| M5 | Journal, storage, Engine, CLI | 4 | JSONL codec, markdown export, data dir, `JournalStore`, `Engine`, all subcommands, assert_cmd tests. **First usable headless product** (with test fixtures as content). QA: CLI matrix, exit codes |
| M6 | Built-in content | 5 | 15 generators, ~69 tables, provenance headers, content tests. Can run in parallel with M7-M9 if QA/content help is available. QA: originality and tone review, cross-seed read-through |
| M7 | TUI shell and Dice tab | 3 | terminal guard, event loop, `App`, `Action`/`TabEvent`, tab bar, help, toast, `LineInput`, Dice tab. QA: terminal matrix, panic check |
| M8 | Oracle and Journal tabs | 3 | both tabs, journal live feed, note entry, export key. QA: end-to-end session then `oraculum export` |
| M9 | Library tab | 4 | list/filter/categories, detail and entries view, generator result with highlights, slot re-roll, warnings panel. QA: filter and slot-focus scripts |
| M10 | Hardening and docs | 3 | fuzz-lite, README (usage, data layout, table/generator authoring guide with examples), release checklist, full manual QA pass. QA: sign-off |
| | **Total** | **~33** | roughly 7 weeks for one developer; M6 overlap could trim about a week |

Dependency notes: M3-M4 freeze the file formats, which M6 content authoring needs. M5 needs M1-M4. M7-M9 need M5 (the Engine); M8's Oracle tab needs M6's `event` generator only for its twist display (use a test fixture before then).

---

## 11. Spec gaps and contradictions found, with resolutions

Full reasoning is in `DECISIONS.md`. "Verified" means checked, no problem.

| # | Finding | Resolution (decision) |
|---|---|---|
| 1 | Goals say "at least ten" generators; §6.1 requires fifteen | Fifteen is the requirement; ten is satisfied (D-01) |
| 2 | `update(KeyEvent) -> Action` cannot roll (no Roller/Clock/journal) while tabs may not share state | Tabs emit intent `Action::Roll(..)`; App executes (D-37) |
| 3 | `q`/`1`-`4` vs always-focused Dice input; "Tab/Shift-Tab" vs text fields | Dice starts in Edit with Esc to Browse; Tab always switches; Ctrl-C cancels an edit first (D-38, D-39, D-42) |
| 4 | `d66` ambiguous with a 66-sided die | Bare `d66` only (D-07) |
| 5 | Table coverage for arbitrary expressions undefined | Contiguous min..max, or the 36 d66 values (D-11) |
| 6 | Dice grammar silent on unary minus, whitespace, aggregate limits | No unary minus; token-level whitespace; 32 terms / 1,000 dice (D-08, D-09) |
| 7 | Same-`id` replacement ignores tables vs generators sharing an id | Separate namespaces, replace within kind (D-17) |
| 8 | Missing `{table:x}` target in a *table* unspecified | Table excluded with warning, to fixpoint (D-20) |
| 9 | A user override can create a cycle that removes built-in generators | Revert-user-first rule (D-21) |
| 10 | Slot re-roll vs template `{roll:..}` and slot `chance` | Template rolls stored and kept; re-roll forces presence (D-13, D-15) |
| 11 | `distinct` comparison and "up to 10 attempts" ambiguous | Compare entry index; 10 total rolls per value (D-12) |
| 12 | `count` unbounded | Max 50, load-time check (D-14) |
| 13 | "id configurable" for the twist generator but no config mechanism | Engine parameter, default `event`, override by user file with same id (D-26, confirmed) |
| 14 | Journal `input`/`result`/`detail` types undefined; "headless prints the same text" | Per-kind table in A§3.9; `result` is exactly stdout (D-22, D-23) |
| 15 | Markdown layout for multi-line results and notes | Exact format in A§3.9 (D-25) |
| 16 | Crash can leave a partial last JSONL line; appends would corrupt the next entry | Prepend `\n` when needed (D-32) |
| 17 | `--session NAME` becomes a filename (path traversal) | `^[A-Za-z0-9_-]{1,64}$` (D-31) |
| 18 | `ts` makes outputs time-dependent while "identical output" is promised | `ts` excluded from the claim; tests mask it (D-43) |
| 19 | Seed per process vs per session; unseeded runs not reproducible | Fresh RNG per invocation; random seed is always a visible `u64` (D-06, D-30, confirmed) |
| 20 | `rand` range sampling can change across versions, breaking pinned expectations | Own seed expansion and rejection sampler (D-04) |
| 21 | `include_str!` of a path outside the crate | Accepted; directory-vs-list test replaces `build.rs` (D-27) |
| 22 | Library key for loader warnings, Esc semantics in filter, `t` target, `r` vs Enter unspecified | `w`; Enter keeps / Esc clears; Oracle category; `r` repeats displayed item (D-40, D-41, D-45) |
| 23 | Unknown TOML keys, `name`/`category` requiredness, user file hygiene | `deny_unknown_fields`; `name` required; defaults; 1 MiB cap (D-18, D-28, D-35) |
| 24 | Event Focus / meaning tables mirror the concept of a published product | Originality is a content-review gate; the human signs off (D-46) |
| 25 | Verified, no issue: the exceptional-range table equals the formula for all rows; all ten worked examples hold; 9 doubles in 1..=100; `Roller` d100 and list tables fit the dice limits | none |

---

## 12. Open questions and resolved questions

**Resolved by the human (card HAG-1):**
1. CLI seed: every invocation restarts the RNG from `--seed`; no persisted stream position (D-30).
2. Twist generator: engine parameter defaulting to `event`, overridable by a user generator with id `event`; no `config.toml`, no flag (D-26, D-47).
3. Content originality: the human signs off all built-in content before release (D-46).

**Still open (non-blocking; defaults apply unless the human objects):**
1. **Product vs repo name.** Keeping "Oraculum" as binary, crate, and data-dir name (`~/.local/share/oraculum`) as instructed. Confirm no rename is pending; changing it later touches the data-dir path.
2. **Toolchain.** Which Rust edition and minimum supported Rust version (MSRV)? Proposed: latest stable at M0, edition 2021 unless a dependency needs 2024, MSRV = whatever ratatui requires then.

---

## 13. Build order for one developer

For Stack (developer) and Rick (QA). Take one task at a time, in order; each ends green on the three gates (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`). Rick tests each task after Stack marks it done. Sizes are estimates in ideal days (see §10 for detail and dependencies). "Spec" means `docs/DESIGN.md`.

**T0 Scaffold and gates (1 d).** Workspace, workspace lints, `clippy.toml`, `deny.toml`, CI on macOS and Linux, empty crates, versions pinned.
- Rick: CI is green on both OSes; a temporary `unwrap()` in non-test code fails CI; `cargo deny check bans` passes.

**T1 Randomness, clock, dice (2 d).** `Roller`, `RngRoller`, `ScriptedRoller`, `Clock`, `FixedClock`, `DiceExpr` parse/roll/display (§3.1-3.3).
- Rick: `3d20kl1+2` with scripted rolls `14,3,17` prints exactly `3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5`; `d66`, `1d66`, `d66kh1`, `-3`, `d1`, `101d6`, `2d6kh3` behave as D-07/D-08 and the spec limits; a fixed-seed golden vector test exists and passes; proptests pass.

**T2 Oracle (1 d).** `evaluate` and `ask` (§3.4).
- Rick: all ten spec worked examples pass; for each likelihood, rolls 1..=100 give yes count = chance and exactly 9 twist rolls; a missing `event` generator yields a twist message naming what is missing, no error.

**T3 Templates and tables (3 d).** `Template`, `Table` validation and rolling (§3.5-3.6).
- Rick: a corpus of bad tables (gap, overlap, out of range, bad d66 digit, empty, `items`+`entries`, 1 or 1001 items, unknown key) each yields its own typed error naming the entry; `{{`/`}}`, `{table:x}`, `{roll:2d6}` resolve; recursion deeper than 5 is rejected; no input panics.

**T4 Generators and catalog (4 d).** `Generator`, slots, segments, slot re-roll, `Catalog::build` pipeline, `Filter` (§3.7-3.8).
- Rick: scripted tests prove `count`, `distinct`, `join`, `list`, `chance` (both outcomes), nested generators; re-rolling one slot leaves all other slots identical; a user file that creates a cycle or dangling reference is dropped with a warning and the built-in survives (D-21); library order is stable across runs.

**T5 Journal, storage, Engine, CLI (4 d).** JSONL codec, markdown export, data dir, `JournalStore`, `Engine`, all spec §8 subcommands (§3.9-3.10, §5.1-5.3).
- Rick: with `--seed 7 --data-dir <tmp>` each of `roll`, `ask`, `table`, `gen`, `list`, `export` prints what the journal stores in `result`; running the same command twice gives identical stdout (D-30); exit codes 0/1/2 as in §5.1; `--session ../x` exits 2; a truncated last journal line is skipped, reported, and the next append starts on a new line; export matches the snapshot. First usable headless build.

**T6 Built-in content (5 d, can overlap T7-T9).** 15 generators and about 69 tables with provenance headers (§7).
- Rick: zero load warnings; every generator renders for seeds 0..500 with no empty value and no `{` or `}`; list tables have at least 12 items; the 15 required generators exist; Rick assembles the originality review packet for the human (D-46). Release gate: the human's sign-off is recorded in `content/PROVENANCE.md`.

**T7 TUI shell and Dice tab (3 d).** `TerminalGuard`, event loop, `App`, `Action`/`TabEvent`, tab bar, help overlay, toast, `LineInput`, Dice tab (§5.4).
- Rick: `1`-`4` and Tab switch tabs; `?` shows help; `q` quits only when no text field is focused; `Ctrl-C` while typing cancels the edit, a second quits; invalid dice input shows an inline error and is not journaled; the terminal is restored on quit and after a forced panic (local patch) in macOS Terminal and a Linux terminal; under 60x20 shows "terminal too small".

**T8 Oracle and Journal tabs (3 d).** Both tabs, live journal feed, notes, export key.
- Rick: `←/→` change likelihood; `/` edits the question; Enter rolls; result shows outcome label, `d100: N`, and a twist block when the roll is a double; `n` adds a note; `x` writes `export.md` and shows the path; a TUI session then `oraculum export` shows the same entries.

**T9 Library tab (4 d).** List, filter, category cycling, detail and entries views, highlighted generator results, slot re-roll, warnings panel.
- Rick: `/` filters by id, name, category, tag; `Esc` clears; `c` cycles categories; `Enter` rolls the selected item and `r` repeats the displayed one; `s` then Enter re-rolls only the chosen slot; `w` shows loader warnings from a deliberately broken user file; user-added tables and generators appear automatically; `t` from the Oracle tab opens the Library on category `Oracle`.

**T10 Hardening and docs (3 d).** Fuzz-lite proptests, README with authoring guide, release checklist, full manual QA pass (§9.5).
- Rick: §9.5 checklist passes on macOS (Apple Silicon) and Linux; README examples run as written; no open defects above minor; the human's content sign-off (T6) is on file. Release candidate.
