# ttrpg-tui

A terminal (ratatui) utility for solo and GMless tabletop RPG play: oracle, dice, and a searchable library of tables and generators, with a per-session journal.

See [docs/DESIGN.md](docs/DESIGN.md) for the spec and [CONTRIBUTING.md](CONTRIBUTING.md) to help.

## Build

Needs the Rust toolchain pinned in `rust-toolchain.toml`.

```sh
cargo build --release          # binary: target/release/oraculum
```

## Use

Run with no subcommand in a terminal to open the interface:

```sh
oraculum
```

Keys: `1`-`4` or Tab switch between Oracle, Library, Dice and Journal; `?` shows help; `q` quits (not while typing in a text field).

Every feature also works from the command line, which is how scripts and pipes use it:

```sh
oraculum roll 3d6+2                         # dice, e.g. 2d20kl1 or d66
oraculum ask likely "Is the door locked?"   # oracle: very-unlikely|unlikely|even|likely|very-likely
oraculum table npc_trait                    # roll one table (see `list`)
oraculum gen encounter                      # run a generator
oraculum list                               # all tables and generators
oraculum export --out session.md            # journal as Markdown
```

Global options: `--seed N` makes every roll reproducible, `--session NAME` picks the journal (letters, digits, `_`, `-`), `--data-dir DIR` moves the data folder. Add your own tables and generators as `.toml` files under `DIR/tables` and `DIR/generators`; problems are reported as warnings and the rest still loads.

Without a terminal (piped), the bare command exits with code 2 and a hint; use a subcommand instead.

License: MIT
