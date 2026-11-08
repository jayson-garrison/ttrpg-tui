//! Catalog tests. The seam under test is `Catalog`'s public interface:
//! build / generate / reroll_slot / roll_table / search.

use super::*;
use crate::generator::ValueDetail;
use crate::rng::{RollCall, ScriptedRoller};

pub(super) fn table(stem: &str, items: &[&str]) -> Source {
    let list = items
        .iter()
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Source::table(
        "test",
        stem,
        &format!("name = \"{stem}\"\nitems = [{list}]"),
    )
}

pub(super) fn generator(stem: &str, body: &str) -> Source {
    Source::generator("test", stem, &format!("name = \"{stem}\"\n{body}"))
}

pub(super) fn build(sources: &[Source]) -> (Catalog, Vec<LoadWarning>) {
    Catalog::build(sources, &[])
}

#[test]
fn generator_with_table_slot_renders_and_tags_slot_text() {
    let (cat, warnings) = build(&[
        table("adj", &["Red", "Blue", "Green"]),
        generator(
            "tavern",
            "template = \"The {a} Inn\"\n[[slots]]\nkey = \"a\"\ntable = \"adj\"",
        ),
    ]);
    assert!(warnings.is_empty(), "{warnings:?}");

    let mut r = ScriptedRoller::new([2]);
    let res = cat.generate("tavern", &mut r).unwrap();
    assert_eq!(res.id, "tavern");
    assert_eq!(res.text, "The Blue Inn");
    assert_eq!(
        r.calls(),
        &[RollCall {
            sides: 3,
            returned: 2
        }]
    );

    let segs: Vec<(&str, Option<usize>)> = res
        .segments
        .iter()
        .map(|s| (s.text.as_str(), s.slot))
        .collect();
    assert_eq!(segs, [("The ", None), ("Blue", Some(0)), (" Inn", None)]);
    assert_eq!(res.slots.len(), 1);
    assert_eq!(res.slots[0].key, "a");
    assert_eq!(res.slots[0].values, ["Blue"]);
    assert!(res.slots[0].present);
}

#[test]
fn unknown_generator_is_an_eval_error() {
    let (cat, _) = build(&[]);
    let mut r = ScriptedRoller::new([]);
    assert_eq!(
        cat.generate("nope", &mut r).unwrap_err(),
        EvalError::UnknownGenerator("nope".into())
    );
}

// ---- Slice B: slot features ------------------------------------------------

fn one_slot(slot_body: &str, template: &str, extra: &[Source]) -> Catalog {
    let mut sources = vec![table("t", &["A", "B", "C", "D"])];
    sources.extend_from_slice(extra);
    sources.push(generator(
        "g",
        &format!("template = \"{template}\"\n[[slots]]\nkey = \"s\"\n{slot_body}"),
    ));
    let (cat, warnings) = build(&sources);
    assert!(warnings.is_empty(), "{warnings:?}");
    cat
}

fn gen_text(cat: &Catalog, script: &[u32]) -> (GeneratorResult, ScriptedRoller) {
    let mut r = ScriptedRoller::new(script.iter().copied());
    let res = cat.generate("g", &mut r).unwrap();
    (res, r)
}

fn rc(sides: u32, returned: u32) -> RollCall {
    RollCall { sides, returned }
}

#[test]
fn count_with_distinct_rerolls_duplicates_by_entry_index() {
    let cat = one_slot("table = \"t\"\ncount = 3", "{s}", &[]);
    let (res, r) = gen_text(&cat, &[1, 1, 2, 2, 3]);
    assert_eq!(res.text, "A, B, C");
    assert_eq!(
        r.calls(),
        &[rc(4, 1), rc(4, 1), rc(4, 2), rc(4, 2), rc(4, 3)]
    );
    assert!(r.is_consumed() && r.faults().is_empty());
}

#[test]
fn distinct_gives_up_after_ten_rolls_per_value_and_accepts_fewer() {
    let cat = one_slot("table = \"t\"\ncount = 2", "{s}", &[]);
    // First value: 1 roll. Second value: 10 rolls, all duplicates -> dropped.
    let (res, r) = gen_text(&cat, &[1; 11]);
    assert_eq!(res.slots[0].values, ["A"]);
    assert_eq!(res.text, "A");
    assert_eq!(r.calls().len(), 11);
    assert!(r.is_consumed());
}

#[test]
fn distinct_false_allows_duplicates() {
    let cat = one_slot("table = \"t\"\ncount = 2\ndistinct = false", "{s}", &[]);
    let (res, _) = gen_text(&cat, &[2, 2]);
    assert_eq!(res.text, "B, B");
}

#[test]
fn join_and_list_rendering() {
    let cat = one_slot("table = \"t\"\ncount = 3\njoin = \" / \"", "{s}", &[]);
    assert_eq!(gen_text(&cat, &[1, 2, 3]).0.text, "A / B / C");

    let cat = one_slot("table = \"t\"\ncount = 2\nlist = true", "Items:\\n{s}", &[]);
    assert_eq!(gen_text(&cat, &[1, 2]).0.text, "Items:\n- A\n- B");
}

#[test]
fn chance_decides_presence_and_absent_slots_roll_nothing_else() {
    let cat = one_slot("table = \"t\"\nchance = 60", "x\\n[{s}]", &[]);
    let (res, r) = gen_text(&cat, &[60, 3]);
    assert_eq!(res.text, "x\n[C]");
    assert!(res.slots[0].present);
    assert_eq!(r.calls(), &[rc(100, 60), rc(4, 3)]);

    let (res, r) = gen_text(&cat, &[61]);
    assert_eq!(res.text, "x");
    assert!(!res.slots[0].present && res.slots[0].values.is_empty());
    assert_eq!(r.calls(), &[rc(100, 61)]);
}

#[test]
fn dice_count_is_rolled_before_values_and_clamped_at_zero() {
    let cat = one_slot(
        "table = \"t\"\ncount = \"1d3-1\"\ndistinct = false",
        "{s}",
        &[],
    );
    let (res, r) = gen_text(&cat, &[3, 4, 1]);
    // 1d3-1 with roll 3 -> 2 values.
    assert_eq!(res.text, "D, A");
    assert_eq!(r.calls(), &[rc(3, 3), rc(4, 4), rc(4, 1)]);
    // Roll 1 -> count 0 -> no values.
    let (res, r) = gen_text(&cat, &[1]);
    assert_eq!(res.text, "");
    assert_eq!(r.calls(), &[rc(3, 1)]);
}

#[test]
fn dice_slot_substitutes_the_total() {
    let cat = one_slot("dice = \"2d6+1\"", "{s} gold", &[]);
    let (res, _) = gen_text(&cat, &[4, 5]);
    assert_eq!(res.text, "10 gold");
    assert!(matches!(res.slots[0].detail[0], ValueDetail::Dice(_)));
}

#[test]
fn template_rolls_are_evaluated_after_slots_once_each() {
    let cat = one_slot("table = \"t\"", "{roll:1d6}-{s}-{roll:1d6}", &[]);
    let (res, r) = gen_text(&cat, &[2, 5, 6]);
    assert_eq!(res.text, "5-B-6");
    assert_eq!(r.calls(), &[rc(4, 2), rc(6, 5), rc(6, 6)]);
    assert_eq!(res.template_rolls.len(), 2);
}

#[test]
fn nested_generator_slot_renders_inline() {
    let inner = generator(
        "inner",
        "template = \"<{x}>\"\n[[slots]]\nkey = \"x\"\ntable = \"t\"",
    );
    let cat = one_slot("generator = \"inner\"", "got {s}", &[inner]);
    let (res, r) = gen_text(&cat, &[4]);
    assert_eq!(res.text, "got <D>");
    assert_eq!(r.calls(), &[rc(4, 4)]);
    assert!(matches!(res.slots[0].detail[0], ValueDetail::Generator(_)));
}

#[test]
fn reroll_slot_changes_only_that_slot_and_keeps_template_rolls() {
    let (cat, w) = build(&[
        table("t", &["A", "B", "C", "D"]),
        generator(
            "g",
            "template = \"{a}/{b}/{roll:1d6}\"\n\
             [[slots]]\nkey = \"a\"\ntable = \"t\"\n\
             [[slots]]\nkey = \"b\"\ntable = \"t\"",
        ),
    ]);
    assert!(w.is_empty(), "{w:?}");
    let (first, _) = gen_text(&cat, &[1, 2, 5]);
    assert_eq!(first.text, "A/B/5");

    let mut r = ScriptedRoller::new([4]);
    let again = cat.reroll_slot(&first, 1, &mut r).unwrap();
    assert_eq!(again.text, "A/D/5");
    assert_eq!(r.calls(), &[rc(4, 4)]);
    assert_eq!(again.slots[0], first.slots[0]);
    assert_eq!(again.template_rolls, first.template_rolls);
    assert_ne!(again.slots[1], first.slots[1]);

    let mut r = ScriptedRoller::new([]);
    assert_eq!(
        cat.reroll_slot(&first, 9, &mut r).unwrap_err(),
        EvalError::UnknownSlot(9)
    );
}

#[test]
fn reroll_forces_an_absent_slot_present() {
    let cat = one_slot("table = \"t\"\nchance = 1", "kept\\nLabel: {s}", &[]);
    let (first, _) = gen_text(&cat, &[50]);
    assert_eq!(first.text, "kept");
    let mut r = ScriptedRoller::new([3]);
    let again = cat.reroll_slot(&first, 0, &mut r).unwrap();
    assert_eq!(again.text, "kept\nLabel: C");
    assert!(again.slots[0].present);
    assert_eq!(r.calls(), &[rc(4, 3)]);
}

/// D-76: a line whose placeholders all render empty is dropped with its newline.
fn absent_text(template: &str) -> String {
    let cat = one_slot("table = \"t\"\nchance = 1", template, &[]);
    gen_text(&cat, &[50]).0.text
}

#[test]
fn all_empty_line_is_dropped_first_middle_and_last() {
    assert_eq!(absent_text("{s}\\nb\\nc"), "b\nc");
    assert_eq!(absent_text("a\\nR: {s}\\nc"), "a\nc");
    assert_eq!(absent_text("a\\nb\\nR: {s}"), "a\nb");
    assert_eq!(absent_text("only {s}"), "");
}

#[test]
fn mixed_and_plain_lines_are_kept() {
    // One non-empty placeholder keeps the line (here a template roll).
    let cat = one_slot("table = \"t\"\nchance = 1", "a {s}{roll:1d2}\\nb", &[]);
    assert_eq!(gen_text(&cat, &[50, 2]).0.text, "a 2\nb");
    // A line with no placeholder is kept, even when empty.
    assert_eq!(absent_text("a\\n\\nb {s}"), "a\n");
    let cat = one_slot("table = \"t\"", "a\\nR: {s}\\nc", &[]);
    assert_eq!(gen_text(&cat, &[2]).0.text, "a\nR: B\nc");
}

#[test]
fn a_dropped_line_keeps_segments_consistent_with_text() {
    let cat = one_slot("table = \"t\"\nchance = 1", "a\\nR: {s}\\nc", &[]);
    let (res, _) = gen_text(&cat, &[50]);
    let joined: String = res.segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(joined, res.text);
    assert_eq!(res.text, "a\nc");
}

fn rejected(slot_body: &str, template: &str, needle: &str) {
    let (cat, warnings) = build(&[
        table("t", &["A", "B"]),
        generator("g", &format!("template = \"{template}\"\n{slot_body}")),
    ]);
    assert!(cat.generator("g").is_none(), "{slot_body}");
    assert!(
        warnings.iter().any(|w| w.message.contains(needle)),
        "{needle:?} not in {warnings:?}"
    );
}

#[test]
fn invalid_generators_are_excluded_with_a_specific_warning() {
    let slot = |body: &str| format!("[[slots]]\nkey = \"s\"\n{body}");
    rejected(&slot(""), "{s}", "exactly one of");
    rejected(
        &slot("table = \"t\"\ndice = \"d6\""),
        "{s}",
        "exactly one of",
    );
    rejected(&slot("table = \"t\"\nchance = 0"), "{s}", "chance");
    rejected(&slot("table = \"t\"\nchance = 101"), "{s}", "chance");
    rejected(&slot("table = \"t\"\ncount = 51"), "{s}", "count");
    rejected(&slot("table = \"t\"\ncount = \"10d6\""), "{s}", "count");
    rejected(&slot("table = \"t\"\ncount = -1"), "{s}", "count");
    rejected(&slot("dice = \"d1\""), "{s}", "dice");
    rejected(&slot("table = \"t\""), "{missing}", "no slot");
    rejected(&slot("table = \"t\"\nbogus = 1"), "{s}", "bogus");
    rejected(
        &format!("{}\n{}", slot("table = \"t\""), slot("table = \"t\"")),
        "{s}",
        "duplicate",
    );
    rejected(
        &slot("table = \"Bad Key\"").replace("\"s\"", "\"S\""),
        "x",
        "slot key",
    );
}

#[test]
fn unused_slot_and_ignored_fields_warn_but_keep_the_generator() {
    for body in [
        "table = \"t\"",
        "dice = \"d6\"\ndistinct = false",
        "table = \"t\"\ncount = 2\nlist = true\njoin = \"-\"",
    ] {
        let template = if body.contains("list") || body.contains("distinct") {
            "{s}"
        } else {
            "none"
        };
        let (cat, warnings) = build(&[
            table("t", &["A", "B"]),
            generator(
                "g",
                &format!("template = \"{template}\"\n[[slots]]\nkey = \"s\"\n{body}"),
            ),
        ]);
        assert!(cat.generator("g").is_some(), "{body}");
        assert_eq!(warnings.len(), 1, "{body}: {warnings:?}");
    }
}

// ---- Slice C: merge, graph validation, search, twist ----------------------

fn user(kind: ItemKind, stem: &str, text: &str) -> Source {
    Source {
        kind,
        origin: format!("user/{stem}.toml"),
        file_stem: stem.into(),
        text: text.into(),
    }
}

fn list_table(name: &str, items: &[&str]) -> String {
    let list = items
        .iter()
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("name = \"{name}\"\nitems = [{list}]")
}

fn kinds(w: &[LoadWarning]) -> Vec<WarningKind> {
    w.iter().map(|w| w.kind).collect()
}

fn text_of(cat: &Catalog, id: &str, script: &[u32]) -> String {
    let mut r = ScriptedRoller::new(script.iter().copied());
    cat.roll_table(id, &mut r).unwrap().text
}

#[test]
fn user_item_replaces_builtin_of_same_kind_and_id() {
    let (cat, w) = Catalog::build(
        &[table("t", &["built", "in"])],
        &[user(
            ItemKind::Table,
            "t",
            &list_table("T", &["mine", "own"]),
        )],
    );
    assert_eq!(kinds(&w), [WarningKind::Replaced]);
    assert_eq!(w[0].item, Some((ItemKind::Table, "t".to_string())));
    assert_eq!(text_of(&cat, "t", &[1]), "mine");
}

#[test]
fn later_user_file_wins_a_duplicate_user_id() {
    let (cat, w) = Catalog::build(
        &[],
        &[
            user(ItemKind::Table, "a", &list_table("first", &["one", "uno"])),
            Source {
                file_stem: "z".into(),
                ..user(
                    ItemKind::Table,
                    "a",
                    &format!("id = \"a\"\n{}", list_table("second", &["two", "dos"])),
                )
            },
        ],
    );
    assert_eq!(kinds(&w), [WarningKind::DuplicateUserId]);
    assert_eq!(text_of(&cat, "a", &[1]), "two");
}

#[test]
fn tables_and_generators_have_separate_id_namespaces() {
    let (cat, w) = build(&[table("x", &["a", "b"]), generator("x", "template = \"hi\"")]);
    assert!(w.is_empty(), "{w:?}");
    assert!(cat.table("x").is_some() && cat.generator("x").is_some());
}

#[test]
fn dangling_references_remove_the_item_and_cascade() {
    let (cat, w) = build(&[
        Source::table(
            "t",
            "bad",
            "name = \"B\"\nitems = [\"see {table:ghost}\", \"x\"]",
        ),
        Source::table(
            "t",
            "uses-bad",
            "name = \"U\"\nitems = [\"{table:bad}\", \"y\"]",
        ),
        generator(
            "g1",
            "template = \"{s}\"\n[[slots]]\nkey = \"s\"\ntable = \"nope\"",
        ),
        generator(
            "g2",
            "template = \"{s}\"\n[[slots]]\nkey = \"s\"\ngenerator = \"g1\"",
        ),
        table("fine", &["a", "b"]),
    ]);
    for gone in ["bad", "uses-bad"] {
        assert!(cat.table(gone).is_none(), "{gone}");
    }
    assert!(cat.generator("g1").is_none() && cat.generator("g2").is_none());
    assert!(cat.table("fine").is_some());
    assert_eq!(
        w.iter()
            .filter(|w| w.kind == WarningKind::MissingReference)
            .count(),
        4,
        "{w:?}"
    );
}

#[test]
fn user_override_that_creates_a_cycle_is_dropped_and_the_builtin_restored() {
    let builtin = [
        Source::table(
            "b",
            "a",
            "name = \"A\"\nitems = [\"{table:b}\", \"{table:b}\"]",
        ),
        table("b", &["leaf", "leaf2"]),
    ];
    let user_b = user(
        ItemKind::Table,
        "b",
        "name = \"B\"\nitems = [\"{table:a}\", \"{table:a}\"]",
    );
    let (cat, w) = Catalog::build(&builtin, &[user_b]);
    assert!(w.iter().any(|w| w.kind == WarningKind::Cycle), "{w:?}");
    assert!(
        w.iter()
            .filter(|w| w.kind == WarningKind::Cycle)
            .all(|w| w.kept_builtin),
        "{w:?}"
    );
    assert!(cat.table("a").is_some(), "built-in a must survive");
    assert_eq!(text_of(&cat, "a", &[1, 1]), "leaf");
}

#[test]
fn cycles_made_only_of_user_items_remove_all_members() {
    let (cat, w) = Catalog::build(
        &[table("ok", &["a", "b"])],
        &[
            user(
                ItemKind::Table,
                "p",
                "name = \"P\"\nitems = [\"{table:q}\", \"x\"]",
            ),
            user(
                ItemKind::Table,
                "q",
                "name = \"Q\"\nitems = [\"{table:p}\", \"x\"]",
            ),
            user(
                ItemKind::Table,
                "self",
                "name = \"S\"\nitems = [\"{table:self}\", \"x\"]",
            ),
        ],
    );
    for gone in ["p", "q", "self"] {
        assert!(cat.table(gone).is_none(), "{gone}");
    }
    assert!(cat.table("ok").is_some());
    assert!(
        w.iter().filter(|w| w.kind == WarningKind::Cycle).count() >= 3,
        "{w:?}"
    );
}

#[test]
fn generator_cycles_are_removed() {
    let g = |id: &str, other: &str| {
        user(ItemKind::Generator, id, &format!("name = \"{id}\"\ntemplate = \"{{s}}\"\n[[slots]]\nkey = \"s\"\ngenerator = \"{other}\""))
    };
    let (cat, w) = Catalog::build(&[], &[g("g1", "g2"), g("g2", "g1")]);
    assert!(cat.generator("g1").is_none() && cat.generator("g2").is_none());
    assert!(w.iter().any(|w| w.kind == WarningKind::Cycle));
}

fn gen_chain(prefix: &str, len: usize) -> Vec<Source> {
    (0..=len)
        .map(|i| {
            let body = if i < len {
                format!(
                    "template = \"{{s}}\"\n[[slots]]\nkey = \"s\"\ngenerator = \"{prefix}{}\"",
                    i + 1
                )
            } else {
                "template = \"end\"".to_string()
            };
            generator(&format!("{prefix}{i}"), &body)
        })
        .collect()
}

#[test]
fn generator_nesting_deeper_than_five_is_removed() {
    // g0 -> ... -> g5 is height 5: fine.
    let (cat, w) = build(&gen_chain("g", 5));
    assert!(w.is_empty(), "{w:?}");
    assert!(cat.generator("g0").is_some());
    // g0 -> ... -> g6 is height 6: the root g0 goes, the rest stay.
    let (cat, w) = build(&gen_chain("g", 6));
    assert!(cat.generator("g0").is_none() && cat.generator("g1").is_some());
    assert_eq!(kinds(&w), [WarningKind::DepthExceeded]);
}

#[test]
fn a_user_generator_that_deepens_a_builtin_chain_is_reverted_first() {
    let builtin = gen_chain("g", 5);
    let deeper = user(
        ItemKind::Generator,
        "g5",
        "name = \"g5\"\ntemplate = \"{s}\"\n[[slots]]\nkey = \"s\"\ngenerator = \"extra\"",
    );
    let extra = user(
        ItemKind::Generator,
        "extra",
        "name = \"extra\"\ntemplate = \"x\"",
    );
    let (cat, w) = Catalog::build(&builtin, &[deeper, extra]);
    assert!(
        cat.generator("g0").is_some(),
        "built-in root must survive: {w:?}"
    );
    assert!(w.iter().any(|w| w.kind == WarningKind::DepthExceeded));
    let mut r = ScriptedRoller::new([]);
    assert_eq!(cat.generate("g0", &mut r).unwrap().text, "end");
}

#[test]
fn table_reference_chains_are_depth_limited() {
    let chain = |len: usize| -> Vec<Source> {
        (0..=len)
            .map(|i| {
                let text = if i < len {
                    format!("{{table:c{}}}", i + 1)
                } else {
                    "end".into()
                };
                Source::table(
                    "t",
                    &format!("c{i}"),
                    &format!("name = \"c\"\nitems = [\"{text}\", \"{text}\"]"),
                )
            })
            .collect()
    };
    let (cat, w) = build(&chain(5));
    assert!(w.is_empty() && cat.table("c0").is_some());
    let (cat, w) = build(&chain(6));
    assert!(cat.table("c0").is_none() && cat.table("c1").is_some());
    assert_eq!(kinds(&w), [WarningKind::DepthExceeded]);
}

#[test]
fn warnings_classify_parse_versus_invalid() {
    let (_, w) = build(&[
        Source::table("t", "broken", "not toml ["),
        Source::table(
            "t",
            "gap",
            "name = \"G\"\ndice = \"d6\"\n[[entries]]\nrange = [1, 2]\ntext = \"a\"",
        ),
        generator(
            "g",
            "template = \"x\"\n[[slots]]\nkey = \"s\"\ntable = \"t\"",
        ),
    ]);
    assert_eq!(w[0].kind, WarningKind::Parse);
    assert_eq!(w[0].origin, "t");
    assert_eq!(w[1].kind, WarningKind::Invalid);
    assert_eq!(w[2].kind, WarningKind::UnusedSlot);
    assert_eq!(w[2].item, Some((ItemKind::Generator, "g".to_string())));
}

#[test]
fn a_leading_byte_order_mark_is_ignored() {
    let (cat, w) = build(&[Source::table(
        "t",
        "bom",
        &format!("\u{feff}{}", list_table("B", &["a", "b"])),
    )]);
    assert!(w.is_empty(), "{w:?}");
    assert!(cat.table("bom").is_some());
}

#[test]
fn items_are_sorted_and_searchable() {
    let t = |stem: &str, name: &str, cat: &str, tags: &str| {
        Source::table(
            "t",
            stem,
            &format!(
                "name = \"{name}\"\ncategory = \"{cat}\"\ntags = [{tags}]\nitems = [\"a\", \"b\"]"
            ),
        )
    };
    let (cat, _) = build(&[
        t("zeta", "Zeta", "Places", "\"wet\""),
        t("alpha", "alpha", "Places", ""),
        t("beta", "Beta", "Characters", "\"Quirk\""),
        generator("quest", "category = \"Characters\"\ntemplate = \"q\""),
    ]);
    let ids: Vec<&str> = cat.items().iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["beta", "quest", "alpha", "zeta"]);
    assert_eq!(cat.categories(), ["Characters", "Places"]);

    let found =
        |f: Filter| -> Vec<String> { cat.search(&f).iter().map(|i| i.id.clone()).collect() };
    assert_eq!(found(Filter::default()).len(), 4);
    assert_eq!(
        found(Filter {
            text: Some("QUIRK".into()),
            ..Filter::default()
        }),
        ["beta"]
    );
    assert_eq!(
        found(Filter {
            text: Some("lph".into()),
            ..Filter::default()
        }),
        ["alpha"]
    );
    assert_eq!(
        found(Filter {
            text: Some("places".into()),
            ..Filter::default()
        }),
        ["alpha", "zeta"]
    );
    assert_eq!(
        found(Filter {
            category: Some("characters".into()),
            ..Filter::default()
        }),
        ["beta", "quest"]
    );
    assert_eq!(
        found(Filter {
            kind: Some(ItemKind::Generator),
            ..Filter::default()
        }),
        ["quest"]
    );
    assert!(found(Filter {
        text: Some("zzz".into()),
        ..Filter::default()
    })
    .is_empty());
}

#[test]
fn build_is_deterministic() {
    let sources = [table("b", &["1", "2"]), table("a", &["3", "4"])];
    let (c1, w1) = build(&sources);
    let (c2, w2) = build(&sources);
    assert_eq!(c1.items(), c2.items());
    assert_eq!(w1, w2);
}

mod twist {
    use super::*;
    use crate::oracle::{ask, Likelihood, Twist};

    #[test]
    fn real_event_generator_is_rolled_after_the_d100() {
        let (cat, _) = build(&[
            table("what", &["A", "B", "C"]),
            generator(
                "event",
                "template = \"event: {w}\"\n[[slots]]\nkey = \"w\"\ntable = \"what\"",
            ),
        ]);
        let mut r = ScriptedRoller::new([22, 3]);
        let res = ask(&cat, &mut r, Likelihood::Even, None, "event");
        match res.twist {
            Twist::Event(g) => assert_eq!(g.text, "event: C"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_generator_is_named() {
        let (cat, _) = build(&[]);
        let mut r = ScriptedRoller::new([11]);
        let res = ask(&cat, &mut r, Likelihood::Even, None, "event");
        assert_eq!(
            res.twist,
            Twist::Unavailable {
                generator: "event".into(),
                missing: "event".into()
            }
        );
    }

    #[test]
    fn generator_excluded_at_load_names_the_missing_table() {
        let (cat, _) = build(&[generator(
            "event",
            "template = \"{w}\"\n[[slots]]\nkey = \"w\"\ntable = \"ghost\"",
        )]);
        let mut r = ScriptedRoller::new([11]);
        let res = ask(&cat, &mut r, Likelihood::Even, None, "event");
        assert_eq!(
            res.twist,
            Twist::Unavailable {
                generator: "event".into(),
                missing: "ghost".into()
            }
        );
    }
}
