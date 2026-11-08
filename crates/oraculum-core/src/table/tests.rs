use super::*;
use crate::rng::{RollCall, ScriptedRoller};

fn load(src: &str) -> Result<Table, TableError> {
    Table::from_toml(src, "stem")
}

fn ranged(dice: &str, entries: &[(i64, i64, &str)]) -> String {
    let mut s = format!("name = \"T\"\ndice = \"{dice}\"\n");
    for (lo, hi, text) in entries {
        s.push_str(&format!(
            "[[entries]]\nrange = [{lo}, {hi}]\ntext = \"{text}\"\n"
        ));
    }
    s
}

fn err(src: &str) -> TableError {
    match load(src) {
        Ok(t) => panic!("should fail, got {t:?}"),
        Err(e) => e,
    }
}

fn lookup(tables: &[&str]) -> BTreeMap<String, Table> {
    tables
        .iter()
        .map(|src| {
            let t = Table::from_toml(src, "x").unwrap();
            (t.id.clone(), t)
        })
        .collect()
}

#[test]
fn ranged_table_loads_sorted_with_defaults() {
    let t = load(&ranged("d6", &[(4, 6, "high"), (1, 3, "low")])).unwrap();
    assert_eq!(t.id, "stem");
    assert_eq!(t.category, "Uncategorized");
    assert!(t.tags.is_empty() && t.description.is_none());
    assert_eq!(t.form, TableForm::Ranged);
    let los: Vec<_> = t.entries.iter().map(|e| (e.lo, e.hi)).collect();
    assert_eq!(los, [(1, 3), (4, 6)]);
}

#[test]
fn metadata_is_read() {
    let src = format!(
        "id = \"my-table\"\ncategory = \"Oracle\"\ntags = [\"a\", \"b\"]\ndescription = \"d\"\n{}",
        ranged("d2", &[(1, 1, "x"), (2, 2, "y")])
    );
    let t = load(&src).unwrap();
    assert_eq!((t.id.as_str(), t.category.as_str()), ("my-table", "Oracle"));
    assert_eq!(t.tags, ["a", "b"]);
    assert_eq!(t.description.as_deref(), Some("d"));
}

#[test]
fn list_table_derives_dice() {
    let t = load("name = \"L\"\nitems = [\"a\", \"b\", \"c\"]").unwrap();
    assert_eq!(t.form, TableForm::List);
    assert_eq!(t.dice.canonical(), "1d3");
    assert_eq!(t.entries.len(), 3);
}

#[test]
fn bad_tables_each_yield_their_own_error() {
    // Gap in the middle, at the start, at the end.
    assert_eq!(
        err(&ranged("d6", &[(1, 2, "a"), (4, 6, "b")])),
        TableError::Gap { from: 3, to: 3 }
    );
    assert_eq!(
        err(&ranged("d6", &[(2, 6, "a")])),
        TableError::Gap { from: 1, to: 1 }
    );
    assert_eq!(
        err(&ranged("d6", &[(1, 4, "a")])),
        TableError::Gap { from: 5, to: 6 }
    );
    // Overlap names the later entry.
    assert_eq!(
        err(&ranged("d6", &[(1, 4, "a"), (4, 6, "b")])),
        TableError::Overlap { entry: 1 }
    );
    assert_eq!(
        err(&ranged("d6", &[(1, 6, "a"), (3, 3, "b")])),
        TableError::Overlap { entry: 1 }
    );
    // Out of range.
    assert_eq!(
        err(&ranged("d6", &[(0, 6, "a")])),
        TableError::OutOfRange {
            entry: 0,
            lo: 0,
            hi: 6,
            min: 1,
            max: 6
        }
    );
    assert_eq!(
        err(&ranged("d6", &[(1, 7, "a")])),
        TableError::OutOfRange {
            entry: 0,
            lo: 1,
            hi: 7,
            min: 1,
            max: 6
        }
    );
    // Inverted range.
    assert_eq!(
        err(&ranged("d6", &[(5, 2, "a")])),
        TableError::InvalidRange {
            entry: 0,
            lo: 5,
            hi: 2
        }
    );
    // Empty text, empty table, missing dice, missing name.
    assert_eq!(
        err(&ranged("d2", &[(1, 1, "a"), (2, 2, "  ")])),
        TableError::EmptyText { entry: 1 }
    );
    assert_eq!(
        err("name = \"T\"\ndice = \"d6\"\nentries = []"),
        TableError::EmptyTable
    );
    assert_eq!(err("name = \"T\""), TableError::EmptyTable);
    assert_eq!(
        err("name = \"T\"\n[[entries]]\nrange = [1, 1]\ntext = \"a\""),
        TableError::MissingDice
    );
    assert_eq!(err("items = [\"a\", \"b\"]"), TableError::MissingName);
    assert_eq!(
        err("name = \" \"\nitems = [\"a\", \"b\"]"),
        TableError::MissingName
    );
}

#[test]
fn d66_tables() {
    let all: Vec<(i64, i64, &str)> = (1..=6)
        .flat_map(|t| (1..=6).map(move |u| (t * 10 + u, t * 10 + u, "x")))
        .collect();
    let t = load(&ranged("d66", &all)).unwrap();
    assert_eq!(t.entries.len(), 36);

    // A range spanning invalid values names the first bad one.
    assert_eq!(
        err(&ranged("d66", &[(11, 66, "x")])),
        TableError::BadD66Value {
            entry: 0,
            value: 17
        }
    );
    assert_eq!(
        err(&ranged("d66", &[(7, 7, "x")])),
        TableError::BadD66Value { entry: 0, value: 7 }
    );
    assert_eq!(
        err(&ranged("d66", &[(0, 11, "x")])),
        TableError::BadD66Value { entry: 0, value: 0 }
    );
    assert_eq!(
        err(&ranged("d66", &[(1, 1_000_000_000, "x")])),
        TableError::BadD66Value { entry: 0, value: 1 }
    );

    // Valid ranges within one decade work; missing values are a gap.
    let mut some: Vec<(i64, i64, &str)> = (1..=6).map(|t| (t * 10 + 1, t * 10 + 6, "x")).collect();
    assert!(load(&ranged("d66", &some)).is_ok());
    some.pop();
    assert_eq!(
        err(&ranged("d66", &some)),
        TableError::Gap { from: 61, to: 61 }
    );
    // Overlap.
    let mut dup = all.clone();
    dup.push((11, 11, "again"));
    assert_eq!(err(&ranged("d66", &dup)), TableError::Overlap { entry: 36 });
}

#[test]
fn unsupported_and_bad_dice() {
    assert_eq!(
        err(&ranged("d66+1", &[(12, 67, "x")])),
        TableError::UnsupportedDice
    );
    // 1d66 is an ordinary 66-sided die (D-07), so a 1-66 table is valid.
    assert!(load(&ranged("1d66", &[(1, 66, "x")])).is_ok());
    assert!(matches!(
        err(&ranged("d1", &[(1, 1, "x")])),
        TableError::BadDice(_)
    ));
    assert!(matches!(
        err(&ranged("nope", &[(1, 1, "x")])),
        TableError::BadDice(_)
    ));
}

#[test]
fn negative_and_compound_dice_ranges() {
    let t = load(&ranged("1d6-2", &[(-1, 0, "low"), (1, 4, "high")])).unwrap();
    assert_eq!(t.entries.len(), 2);
    let t = load(&ranged("2d6", &[(2, 6, "a"), (7, 12, "b")])).unwrap();
    assert_eq!(t.dice.canonical(), "2d6");
}

#[test]
fn list_size_limits() {
    assert_eq!(
        err("name = \"L\"\nitems = [\"only\"]"),
        TableError::ListSize { got: 1 }
    );
    assert_eq!(
        err("name = \"L\"\nitems = []"),
        TableError::ListSize { got: 0 }
    );
    let many = vec!["\"x\""; 1001].join(",");
    assert_eq!(
        err(&format!("name = \"L\"\nitems = [{many}]")),
        TableError::ListSize { got: 1001 }
    );
    let max = vec!["\"x\""; 1000].join(",");
    assert!(load(&format!("name = \"L\"\nitems = [{max}]")).is_ok());
    assert_eq!(
        err("name = \"L\"\nitems = [\"a\", \" \"]"),
        TableError::EmptyText { entry: 1 }
    );
}

#[test]
fn conflicting_fields() {
    assert_eq!(
        err(&format!(
            "items = [\"a\", \"b\"]\n{}",
            ranged("d2", &[(1, 1, "x"), (2, 2, "y")])
        )),
        TableError::ConflictingFields
    );
    assert_eq!(
        err("name = \"L\"\ndice = \"d6\"\nitems = [\"a\", \"b\"]"),
        TableError::ConflictingFields
    );
}

#[test]
fn unknown_keys_and_types_are_toml_errors() {
    let e = err("name = \"L\"\nitems = [\"a\", \"b\"]\nchanse = 5");
    assert!(
        matches!(&e, TableError::Toml(m) if m.contains("chanse")),
        "{e}"
    );
    let e = err(
        "name = \"L\"\nitems = [\"a\", \"b\"]\n[[entries]]\nrange = [1,1]\ntext = \"a\"\nbogus = 1",
    );
    assert!(
        matches!(&e, TableError::Toml(m) if m.contains("bogus")),
        "{e}"
    );
    assert!(matches!(err("name = 5"), TableError::Toml(_)));
    assert!(matches!(err("not toml ["), TableError::Toml(_)));
    assert!(matches!(
        err("name = \"T\"\ndice = \"d2\"\n[[entries]]\nrange = [1]\ntext = \"a\""),
        TableError::Toml(_)
    ));
}

#[test]
fn bad_ids_and_templates() {
    assert_eq!(
        Table::from_toml("name = \"L\"\nitems = [\"a\", \"b\"]", "Bad Stem").unwrap_err(),
        TableError::BadId("Bad Stem".into())
    );
    assert_eq!(
        err("id = \"X\"\nname = \"L\"\nitems = [\"a\", \"b\"]"),
        TableError::BadId("X".into())
    );
    assert!(matches!(
        err("name = \"L\"\nitems = [\"ok\", \"{nope}\"]"),
        TableError::BadTemplate { entry: 1, .. }
    ));
    assert!(matches!(
        err(&ranged("d2", &[(1, 1, "{table:A}"), (2, 2, "y")])),
        TableError::BadTemplate { entry: 0, .. }
    ));
}

#[test]
fn roll_picks_entry_by_total() {
    let t = load(&ranged(
        "d6",
        &[(1, 2, "low"), (3, 5, "mid"), (6, 6, "top")],
    ))
    .unwrap();
    let none = BTreeMap::new();
    for (v, want, idx) in [
        (1, "low", 0),
        (2, "low", 0),
        (3, "mid", 1),
        (5, "mid", 1),
        (6, "top", 2),
    ] {
        let mut r = ScriptedRoller::new([v]);
        let roll = t.roll(&none, &mut r).unwrap();
        assert_eq!((roll.text.as_str(), roll.entry_index), (want, idx), "{v}");
        assert_eq!(roll.dice.total, i64::from(v));
        assert!(r.faults().is_empty());
    }
}

#[test]
fn roll_d66_and_negative_ranges() {
    let all: Vec<(i64, i64, String)> = (1..=6)
        .flat_map(|t| (1..=6).map(move |u| (t * 10 + u, t * 10 + u, format!("e{}", t * 10 + u))))
        .collect();
    let refs: Vec<(i64, i64, &str)> = all.iter().map(|(a, b, s)| (*a, *b, s.as_str())).collect();
    let t = load(&ranged("d66", &refs)).unwrap();
    let mut r = ScriptedRoller::new([3, 5]);
    assert_eq!(t.roll(&BTreeMap::new(), &mut r).unwrap().text, "e35");

    let t = load(&ranged("1d6-2", &[(-1, 0, "neg"), (1, 4, "pos")])).unwrap();
    let mut r = ScriptedRoller::new([1]);
    assert_eq!(t.roll(&BTreeMap::new(), &mut r).unwrap().text, "neg");
}

#[test]
fn inline_references_resolve_in_order_with_exact_roll_sequence() {
    let tables = lookup(&[
        "id = \"main\"\nname = \"M\"\nitems = [\"A {table:name} has {roll:2d6} coins {{ok}}\", \"unused\"]",
        "id = \"name\"\nname = \"N\"\nitems = [\"Ann\", \"Bo\", \"Cy\"]",
    ]);
    let main = tables.table("main").unwrap();
    let mut r = ScriptedRoller::new([1, 2, 4, 5]);
    let roll = main.roll(&tables, &mut r).unwrap();
    assert_eq!(roll.text, "A Bo has 9 coins {ok}");
    assert_eq!(roll.raw, "A {table:name} has {roll:2d6} coins {{ok}}");
    assert_eq!(roll.refs.len(), 2);
    assert!(
        matches!(&roll.refs[0], ResolvedRef::Table(t) if t.text == "Bo" && t.table_id == "name")
    );
    assert!(matches!(&roll.refs[1], ResolvedRef::Dice(d) if d.total == 9));
    assert_eq!(
        r.calls(),
        &[
            RollCall {
                sides: 2,
                returned: 1
            },
            RollCall {
                sides: 3,
                returned: 2
            },
            RollCall {
                sides: 6,
                returned: 4
            },
            RollCall {
                sides: 6,
                returned: 5
            },
        ]
    );
}

#[test]
fn substituted_text_is_not_rescanned() {
    // The referenced entry contains an escaped brace pair; it must stay literal
    // and must not be re-parsed as a placeholder.
    let tables = lookup(&[
        "id = \"a\"\nname = \"A\"\nitems = [\"[{table:b}]\", \"x\"]",
        "id = \"b\"\nname = \"B\"\nitems = [\"{{table:a}}\", \"y\"]",
    ]);
    let mut r = ScriptedRoller::new([1, 1]);
    let roll = tables.table("a").unwrap().roll(&tables, &mut r).unwrap();
    assert_eq!(roll.text, "[{table:a}]");
}

#[test]
fn depth_limit_is_five_levels() {
    // t0 -> t1 -> ... -> tN, each a one-entry-ish list table.
    let chain = |n: usize| -> BTreeMap<String, Table> {
        let mut srcs = Vec::new();
        for i in 0..=n {
            let text = if i < n {
                format!("{{table:t{}}}", i + 1)
            } else {
                "end".into()
            };
            srcs.push(format!(
                "id = \"t{i}\"\nname = \"T\"\nitems = [\"{text}\", \"{text}\"]"
            ));
        }
        let refs: Vec<&str> = srcs.iter().map(String::as_str).collect();
        lookup(&refs)
    };
    // Five nested references (t0 -> t5) are allowed.
    let tables = chain(5);
    let mut r = ScriptedRoller::new([1; 6]);
    assert_eq!(
        tables
            .table("t0")
            .unwrap()
            .roll(&tables, &mut r)
            .unwrap()
            .text,
        "end"
    );
    // Six are rejected.
    let tables = chain(6);
    let mut r = ScriptedRoller::new([1; 7]);
    assert_eq!(
        tables
            .table("t0")
            .unwrap()
            .roll(&tables, &mut r)
            .unwrap_err(),
        EvalError::DepthExceeded { max: 5 }
    );
}

#[test]
fn self_reference_hits_the_guard_instead_of_looping() {
    let tables =
        lookup(&["id = \"loop\"\nname = \"L\"\nitems = [\"{table:loop}\", \"{table:loop}\"]"]);
    let mut r = ScriptedRoller::new([1; 10]);
    assert_eq!(
        tables
            .table("loop")
            .unwrap()
            .roll(&tables, &mut r)
            .unwrap_err(),
        EvalError::DepthExceeded { max: 5 }
    );
}

#[test]
fn unknown_table_reference_is_an_eval_error() {
    let tables = lookup(&["id = \"a\"\nname = \"A\"\nitems = [\"{table:ghost}\", \"x\"]"]);
    let mut r = ScriptedRoller::new([1]);
    assert_eq!(
        tables
            .table("a")
            .unwrap()
            .roll(&tables, &mut r)
            .unwrap_err(),
        EvalError::UnknownTable("ghost".into())
    );
}

#[test]
fn table_refs_lists_sorted_unique_ids() {
    let t = load("name = \"L\"\nitems = [\"{table:b} {table:a}\", \"{table:b}\"]").unwrap();
    assert_eq!(t.table_refs(), ["a", "b"]);
}

#[test]
fn arbitrary_toml_never_panics() {
    let pieces = [
        "name = \"T\"\n",
        "dice = \"d6\"\n",
        "items = [\"a\",\"b\"]\n",
        "[[entries]]\n",
        "range = [1, 6]\n",
        "text = \"x\"\n",
        "range = [6, 1]\n",
        "dice = \"d66\"\n",
        "zzz = 1\n",
        "[\n",
    ];
    for a in pieces {
        for b in pieces {
            for c in pieces {
                let _ = Table::from_toml(&format!("{a}{b}{c}"), "stem");
            }
        }
    }
}
