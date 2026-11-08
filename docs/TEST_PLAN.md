# Test Plan

Maps [DESIGN.md](DESIGN.md) section 9 to test layers and tools. QA owns this file. All tests run under `cargo test`; randomness comes from a scripted `Roller` or a fixed seed, time from a fake `Clock`.

## Layers and tools

| Layer | Tool | Used for |
|---|---|---|
| Unit, table-driven | plain `#[test]` over case arrays | parsers, validators, formulas |
| Property | `proptest` | invariants over generated input |
| Scripted-roller | test `Roller` returning fixed values | exact keep/drop, slot and oracle cases |
| Integration, temp dirs | `tempfile` | journal append/load, content loading |
| Snapshot | `insta` | markdown export, rendered text output |
| CLI | `assert_cmd` + `--seed` + temp `--data-dir` | stdout, stderr, exit codes, journal contents |
| TUI update | synthetic `KeyEvent`s on tab state | key handling and `Action` results |
| TUI render | ratatui `TestBackend` | key strings present in the buffer |

## Spec section 9 mapping

| Area | Tests | Layer / tool |
|---|---|---|
| Dice | Valid and invalid expression parse cases; keep-highest/lowest, `d66`, negative terms, constants with scripted rolls; every die in `1..=M`, total equals kept dice plus constants | table-driven, scripted-roller, `proptest` |
| Oracle | Every worked example in the spec; all 100 rolls at each likelihood: yes count equals the chance, exceptional counts match the formula, yes count rises with likelihood, exactly 9 twist rolls | table-driven, exhaustive loop |
| Tables | Parse/validation per error class (gap, overlap, out of range, bad `d66` digit, empty); inline reference and recursion limit; cycle detection | table-driven |
| Generators | Slot behavior (count, distinct, join, list, chance) with scripted rolls; nested generators; load-time validation; every built-in generator renders cleanly across many seeds; single-slot re-roll leaves other slots unchanged | scripted-roller, seed sweep, `proptest` |
| Journal | JSONL append/load; malformed-line reporting; markdown export | `tempfile`, `insta` |
| CLI | Each subcommand (`roll`, `ask`, `table`, `gen`, `list`, `export`) with `--seed` and temp `--data-dir`; invalid input gives non-zero exit and clear message; warnings on stderr | `assert_cmd` |
| TUI | Likelihood change; invalid dice input shows an error; `q` while typing does not quit; Library filtering and slot re-roll | `update` unit tests |
| TUI render | Key strings present per tab | `TestBackend` |

## Cross-cutting checks (spec section 3)

| Constraint | Check |
|---|---|
| Determinism under `--seed` | CLI test runs the same command twice and compares output; seed sweep over built-ins |
| Broken user files never crash | Fixture of malformed table/generator/journal files: exit 0, warning on stderr |
| Terminal restored on exit/panic | Unit test of the guard type (restore runs on drop and from the panic hook) |
| No `unwrap`/`expect` in non-test code | Clippy lints `clippy::unwrap_used`, `clippy::expect_used` denied at crate level, with `#![cfg_attr(test, allow(...))]`; reviewer grep per CONTRIBUTING.md |
| No C deps, no async/network | Reviewer checks `Cargo.lock` diffs; optional `cargo-deny` ban list later |
| Core does no I/O | `oraculum-core` has no `std::fs`/`std::io` use; reviewed per PR |
| Original built-in content | Manual review per content PR |

## CI

`.github/workflows/ci.yml` runs fmt, clippy (`-D warnings`), build and test on `ubuntu-latest` and `macos-latest`, stable Rust. Until a root `Cargo.toml` exists the job detects that and passes without running Rust steps; the gates switch on automatically once the workspace lands.
