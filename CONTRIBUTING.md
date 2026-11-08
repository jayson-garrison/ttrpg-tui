# Contributing

The source of truth is [docs/DESIGN.md](docs/DESIGN.md). Section 3 (hard constraints) is non-negotiable. Test layers are in [docs/TEST_PLAN.md](docs/TEST_PLAN.md).

## Workflow

- Branch from `main`, open a pull request (PR). Nothing is pushed to `main` directly.
- Keep PRs small and about one thing. Record any spec ambiguity you resolved in `DECISIONS.md`.
- A new dependency needs a recorded reason in `DECISIONS.md` (spec section 2).

## Definition of Done

A PR is done when all of these hold:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes on Linux and macOS (CI runs both).
4. New behavior has tests in the layer the test plan names for it. A bug fix has a test that fails without the fix.
5. Every hard constraint below is respected.
6. Docs, `DECISIONS.md` and snapshots are updated where affected. Snapshot changes (insta) are reviewed, not blindly accepted.
7. QA has reviewed the PR against the checklist.

## Hard-constraint review checklist

Reviewers tick each item; a failure blocks merge.

- [ ] No `unwrap()` / `expect()` / `panic!` / unchecked indexing in non-test code. Library code returns typed errors (`thiserror`); `anyhow` only in the binary crate.
- [ ] No C dependencies (e.g. `rusqlite`, `git2`, `openssl`, `-sys` crates). Check `Cargo.lock` diff.
- [ ] No async runtime and no network access (`tokio`, `async-std`, `reqwest`, `std::net`, ...).
- [ ] `oraculum-core` does no terminal or file I/O; it takes `&str` and returns values.
- [ ] All randomness goes through the `Roller` trait (no `rand::thread_rng`, `random()`, direct RNG use outside the `Roller` implementation).
- [ ] All time goes through the `Clock` trait (no `SystemTime::now` / `Instant::now` / `chrono` outside the implementation).
- [ ] The terminal is restored on exit and on panic.
- [ ] Broken user files produce warnings, never crashes.
- [ ] Built-in content is original, with no text copied from published games, tables or oracle products.
- [ ] No new dependency without a `DECISIONS.md` entry; dev-only crates stay in `[dev-dependencies]`.
- [ ] Anything random or time-based is deterministic under `--seed` and testable with a scripted `Roller` / fake `Clock`.

Quick local scan before requesting review:

```sh
grep -rnE 'unwrap\(\)|expect\(|thread_rng|SystemTime::now|Instant::now' crates --include=*.rs
```

Hits inside `#[cfg(test)]` or `tests/` are fine; anything else needs justification.
