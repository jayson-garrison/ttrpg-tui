# Pre-release security review (QA, v0.1.0)

Scope: the `oraculum` and `oraculum-core` code on main at 957efc6. Method: code reading of path handling, file writes and TOML loading, adversarial runs of the release binary on macOS, lint and dependency checks. Result: no blocking findings.

## Checked

| Area | Result |
|---|---|
| `--session` path handling | Names are restricted to 1-64 of `[A-Za-z0-9_-]`; `../x` and `/etc/passwd` exit 2. The journal path is always `<data-dir>/sessions/<name>.jsonl`. |
| `--data-dir` | Taken literally from the user (their own machine and account); no traversal concern. |
| File writes | Journal is append-only with a repaired mid-line start; export writes `<out>.tmp` then renames. Unwritable target (`export --out /`) exits 1 with a message. |
| Untrusted user TOML | Random binary, 200k nested `[`, 3 MB file, a self-referencing table and a symlinked file all load as warnings (skipped) with no crash or hang. Files over `MAX_CONTENT_BYTES` and non-UTF-8 are rejected. |
| Dice limits | `999999999d999999999`, `101d6` and an overflowing keep count are rejected with exit 2. |
| Panic safety | Workspace lints deny `unwrap_used`, `expect_used`, `panic` and `unsafe_code`; clippy `-D warnings` is green. Terminal restore on panic verified on a pty (PR #27). |
| Dependencies | `cargo deny check` advisories, bans and sources pass. No C, async or network crates. |

## Findings (all non-blocking)

1. **Symlinks in the data folder are followed.** A `tables/link.toml` pointing at another file is read, and the TOML parse error can quote one line number and key hint of it in a warning. Local-user only, low severity. Suggested: skip symlinks in `load_user_sources` (`is_file()` follows them; use `symlink_metadata`).
2. **`deny.toml` has no `[licenses]` section**, so `cargo deny check licenses` rejects everything. CI only runs `bans`, so it is not a gate. Dependencies are MIT/Apache-2.0 plus one MPL-2.0 (`option-ext`) and `r-efi` (MIT/Apache/LGPL, dual-licensed). Suggested: add an allow list or keep CI on `bans`.
3. `export --out` accepts any path the user can write, including overwriting an existing file. Expected for a CLI.

## Not covered

No fuzzing beyond the manual cases above; no Windows run; real macOS Terminal and Linux terminal sessions are for a human.
