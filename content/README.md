# Built-in content

Tables (`tables/<id>.toml`) and generators (`generators/<id>.toml`) shipped inside the binary.
File stem == id. Every file is listed in `crates/oraculum-core/src/builtin.rs` (alphabetical, one line each);
`tests/builtin.rs` fails if a file or a list entry is missing.

## Authoring rules (docs/T6_PLAN.md sections 4-5, D-68..D-73)

1. Write from scratch. Do not reword any published table, oracle, bestiary or setting; no SRD or Creative Commons text.
2. No names from published games or fiction; no real nations, faiths, peoples, living people or brands. No slurs, no sexual content.
3. Header on every file: the two-line "Written for Oraculum" comment with `Author:` and `Form:` (noun phrase, clause, name, fragment).
4. Entries: 3-80 ASCII characters, no leading/trailing/double space, no final period, no `{` or `}`, no case-insensitive duplicates. Entries carry their own article; templates never add one.
5. Tables used with `count > 1` contain no commas.
6. `category` is one of Characters, Creatures, Places, Sites, Adventure, Items, Organizations, Oracle. `tags`: 1-3 lowercase words.
7. Every table is used by at least one generator.
8. American spelling throughout (color, favor, traveling, behavior).

## Optional slots (chance below 100)

D-76: a template line whose placeholders all render empty is dropped with its newline. Put an optional slot
(table or generator) on its own labelled line, e.g. `Rumor: {rumor}`, never mid-sentence and never sharing a line
with a mandatory slot. (`npc_secret` predates D-76 and uses `; secretly ...` entries; both patterns work.)

## Tests and snapshots

`cargo test -p oraculum-core --test builtin`. Generator output for seeds 1..3 is pinned in `crates/oraculum-core/tests/snapshots/`;
after a content change review the diff and accept with `INSTA_UPDATE=always cargo test -p oraculum-core --test builtin`.
Nothing else in the repo may assert built-in text (D-72).
