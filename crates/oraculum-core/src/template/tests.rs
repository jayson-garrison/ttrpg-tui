use super::*;
use TemplateContext::{GeneratorTemplate as G, TableEntry as T};

fn parse(s: &str, ctx: TemplateContext) -> Template {
    Template::parse(s, ctx).unwrap_or_else(|e| panic!("{s:?}: {e}"))
}

fn err(s: &str, ctx: TemplateContext) -> TemplateError {
    match Template::parse(s, ctx) {
        Ok(t) => panic!("{s:?} should fail: {t:?}"),
        Err(e) => e,
    }
}

fn lit(s: &str) -> Segment {
    Segment::Literal(s.to_string())
}

#[test]
fn plain_text_and_empty() {
    assert_eq!(parse("hello", T).segments(), [lit("hello")]);
    assert!(parse("", T).segments().is_empty());
    assert_eq!(parse("naïve ✓", T).segments(), [lit("naïve ✓")]);
}

#[test]
fn escaped_braces() {
    assert_eq!(parse("{{x}}", T).segments(), [lit("{x}")]);
    assert_eq!(parse("a{{", T).segments(), [lit("a{")]);
    assert_eq!(parse("}}b", T).segments(), [lit("}b")]);
    assert_eq!(parse("{{{{}}}}", T).segments(), [lit("{{}}")]);
}

#[test]
fn table_and_roll_refs() {
    let t = parse("A {table:npc-name}, {roll:2d6} gold", T);
    assert_eq!(
        t.segments(),
        [
            lit("A "),
            Segment::TableRef("npc-name".into()),
            lit(", "),
            Segment::Roll(DiceExpr::parse("2d6").unwrap()),
            lit(" gold"),
        ]
    );
    assert_eq!(t.table_refs().collect::<Vec<_>>(), ["npc-name"]);
}

#[test]
fn slots_in_generator_templates() {
    let t = parse("{a} and {b_2} {{lit}} {roll:d4}", G);
    assert_eq!(t.slot_keys().collect::<Vec<_>>(), ["a", "b_2"]);
    assert_eq!(t.segments().len(), 5);
}

#[test]
fn braces_next_to_placeholders() {
    // "{{" is a literal '{', then "{x}" is a slot, then "}}" a literal '}'.
    let t = parse("{{{x}}}", G);
    assert_eq!(
        t.segments(),
        [lit("{"), Segment::Slot("x".into()), lit("}")]
    );
}

#[test]
fn errors_have_positions() {
    assert_eq!(err("ab}", T), TemplateError::UnmatchedClose { pos: 2 });
    assert_eq!(err("ab{", T), TemplateError::UnmatchedOpen { pos: 2 });
    assert_eq!(err("{abc", T), TemplateError::UnmatchedOpen { pos: 0 });
    assert_eq!(err("{a{b}", T), TemplateError::UnmatchedOpen { pos: 0 });
    assert_eq!(err("x{}", T), TemplateError::EmptyPlaceholder { pos: 1 });
    assert_eq!(
        err("{foo}", T),
        TemplateError::UnknownPlaceholder {
            pos: 0,
            text: "foo".into()
        }
    );
    assert_eq!(
        err("{table:x}", G),
        TemplateError::WrongContext {
            pos: 0,
            text: "table:x".into()
        }
    );
    assert_eq!(
        err("{table:Bad Id}", T),
        TemplateError::BadId {
            pos: 0,
            text: "table:Bad Id".into()
        }
    );
    assert_eq!(
        err("{Foo}", G),
        TemplateError::BadId {
            pos: 0,
            text: "Foo".into()
        }
    );
    assert!(matches!(
        err("{roll:}", T),
        TemplateError::BadDice { pos: 0, .. }
    ));
    assert!(matches!(
        err("ok {roll:d1}", G),
        TemplateError::BadDice { pos: 3, .. }
    ));
}

#[test]
fn identifier_rules() {
    for ok in ["a", "0", "npc_name", "a-b", &"a".repeat(64)] {
        assert!(is_valid_item_id(ok), "{ok}");
    }
    for bad in ["", "_a", "-a", "A", "a b", "é", &"a".repeat(65)] {
        assert!(!is_valid_item_id(bad), "{bad}");
    }
    for ok in ["a", "_", "a1", "_x_y", &"a".repeat(32)] {
        assert!(is_valid_slot_key(ok), "{ok}");
    }
    for bad in ["", "1a", "a-b", "A", &"a".repeat(33)] {
        assert!(!is_valid_slot_key(bad), "{bad}");
    }
}

#[test]
fn arbitrary_text_never_panics() {
    let pieces = [
        "{", "}", "{{", "}}", "table:", "roll:", "d6", "x", "é", " ", ":",
    ];
    for a in pieces {
        for b in pieces {
            for c in pieces {
                let s = format!("{a}{b}{c}");
                let _ = Template::parse(&s, T);
                let _ = Template::parse(&s, G);
            }
        }
    }
}
