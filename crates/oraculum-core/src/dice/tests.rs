use super::*;
use crate::rng::{RollCall, Roller, ScriptedRoller};

fn parse(s: &str) -> DiceExpr {
    DiceExpr::parse(s).unwrap_or_else(|e| panic!("{s:?} should parse: {e}"))
}

fn err(s: &str) -> DiceError {
    match DiceExpr::parse(s) {
        Ok(e) => panic!("{s:?} should fail, got {e:?}"),
        Err(e) => e,
    }
}

fn run(src: &str, script: &[u32]) -> (DiceRoll, ScriptedRoller) {
    let mut r = ScriptedRoller::new(script.iter().copied());
    let roll = parse(src).roll(&mut r);
    assert!(r.faults().is_empty(), "faults: {:?}", r.faults());
    assert!(r.is_consumed(), "script not fully used");
    (roll, r)
}

#[test]
fn canonical_forms() {
    let cases = [
        ("d6", "1d6"),
        ("2D6", "2d6"),
        (" 3 d 20 KL 1 + 2 ", "3d20kl1+2"),
        ("4d6kh3", "4d6kh3"),
        ("2d20kh", "2d20kh1"),
        ("d66", "d66"),
        ("1d66", "1d66"),
        ("d66kh1", "1d66kh1"),
        ("d66+1", "d66+1"),
        ("007", "7"),
        ("1d6-1+3", "1d6-1+3"),
        ("d 6", "1d6"),
    ];
    for (src, want) in cases {
        assert_eq!(parse(src).canonical(), want, "{src:?}");
        assert_eq!(parse(src).to_string(), want);
    }
}

#[test]
fn canonical_round_trips() {
    for src in ["3d20kl1+2", "d66", "1d66", "1d6-1+3", "2d6kh1", "5"] {
        let e = parse(src);
        assert_eq!(parse(&e.canonical()), e, "{src}");
    }
}

#[test]
fn spec_example_prints_exactly() {
    let (roll, r) = run("3d20kl1+2", &[14, 3, 17]);
    assert_eq!(roll.to_string(), "3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5");
    assert_eq!(roll.total, 5);
    assert_eq!(
        r.calls(),
        &[
            RollCall {
                sides: 20,
                returned: 14
            },
            RollCall {
                sides: 20,
                returned: 3
            },
            RollCall {
                sides: 20,
                returned: 17
            },
        ]
    );
}

#[test]
fn d66_reads_tens_then_units() {
    let (roll, r) = run("d66", &[3, 5]);
    assert_eq!(roll.total, 35);
    assert_eq!(roll.to_string(), "d66 => [3, 5]=35 = 35");
    assert_eq!(
        r.calls(),
        &[
            RollCall {
                sides: 6,
                returned: 3
            },
            RollCall {
                sides: 6,
                returned: 5
            },
        ]
    );
}

#[test]
fn one_d66_and_d66_with_keep_are_ordinary_66_sided_dice() {
    for src in ["1d66", "d66kh1"] {
        let e = parse(src);
        assert!(!e.is_bare_d66(), "{src}");
        let (roll, r) = run(src, &[66]);
        assert_eq!(roll.total, 66);
        assert_eq!(
            r.calls(),
            &[RollCall {
                sides: 66,
                returned: 66
            }]
        );
    }
}

#[test]
fn d66_inside_larger_expression() {
    let (roll, _) = run("d66+1", &[1, 2]);
    assert_eq!(roll.total, 13);
    assert_eq!(roll.to_string(), "d66+1 => [1, 2]=12 +1 = 13");
    assert!(parse("d66+1").contains_d66());
    assert!(!parse("d66+1").is_bare_d66());
}

#[test]
fn negative_terms_and_constants() {
    let (roll, _) = run("2d6-1d4-3+10", &[6, 5, 2]);
    assert_eq!(roll.total, 11 - 2 - 3 + 10);
    assert_eq!(roll.to_string(), "2d6-1d4-3+10 => [6, 5] -[2] -3 +10 = 16");
}

#[test]
fn keep_highest_and_lowest_with_ties_prefer_lower_index() {
    // 3 3 1 keep highest 2 -> indices 0 and 1.
    let (roll, _) = run("3d6kh2", &[3, 3, 1]);
    assert_eq!(roll.to_string(), "3d6kh2 => [3, 3, 1] keep [3, 3] = 6");
    // 2 2 2 keep lowest 1 -> index 0 only.
    let (roll, _) = run("3d6kl", &[2, 2, 2]);
    match &roll.terms[0].kind {
        TermRollKind::Dice { kept, .. } => assert_eq!(kept, &[true, false, false]),
        other => panic!("{other:?}"),
    }
    // Kept values are shown in roll order.
    let (roll, _) = run("4d6kh3", &[2, 6, 1, 5]);
    assert_eq!(
        roll.to_string(),
        "4d6kh3 => [2, 6, 1, 5] keep [2, 6, 5] = 13"
    );
}

#[test]
fn boon_and_bane_forms() {
    let (roll, _) = run("2d20kl1", &[15, 4]);
    assert_eq!(roll.total, 4);
    let (roll, _) = run("2d20kh1", &[15, 4]);
    assert_eq!(roll.total, 15);
}

#[test]
fn parse_errors() {
    use DiceError::*;
    assert_eq!(err(""), Empty);
    assert_eq!(err("   "), Empty);
    assert_eq!(
        err("-3"),
        UnexpectedToken {
            pos: 0,
            expected: "a number or dice"
        }
    );
    assert_eq!(
        err("+3"),
        UnexpectedToken {
            pos: 0,
            expected: "a number or dice"
        }
    );
    assert_eq!(err("d1"), SidesOutOfRange { pos: 1, got: 1 });
    assert_eq!(err("d1001"), SidesOutOfRange { pos: 1, got: 1001 });
    assert_eq!(err("101d6"), CountOutOfRange { pos: 0, got: 101 });
    assert_eq!(err("0d6"), CountOutOfRange { pos: 0, got: 0 });
    assert_eq!(err("2d6kh3"), KeepOutOfRange { pos: 5, k: 3, n: 2 });
    assert_eq!(err("2d6kh0"), KeepOutOfRange { pos: 5, k: 0, n: 2 });
    assert_eq!(
        err("1 2"),
        UnexpectedToken {
            pos: 2,
            expected: "'+', '-' or end of expression"
        }
    );
    assert_eq!(
        err("2d6+"),
        UnexpectedToken {
            pos: 4,
            expected: "a number or dice"
        }
    );
    assert_eq!(
        err("2d"),
        UnexpectedToken {
            pos: 2,
            expected: "number of sides"
        }
    );
    assert_eq!(err("2d6x"), UnexpectedChar { pos: 3, ch: 'x' });
    assert_eq!(
        err("2d6k"),
        UnexpectedToken {
            pos: 4,
            expected: "'h' or 'l' after 'k'"
        }
    );
    assert_eq!(err("2d6kx"), UnexpectedChar { pos: 4, ch: 'x' });
    assert_eq!(err("1000001"), NumberTooLarge { pos: 0 });
    assert_eq!(err("99999999999999999999999"), NumberTooLarge { pos: 0 });
    assert_eq!(
        err("d6 d6"),
        UnexpectedToken {
            pos: 3,
            expected: "'+', '-' or end of expression"
        }
    );
    assert_eq!(err("2d6 k h"), UnexpectedChar { pos: 5, ch: ' ' });
    assert_eq!(err("é"), UnexpectedChar { pos: 0, ch: 'é' });
    assert_eq!(err("d6+é"), UnexpectedChar { pos: 3, ch: 'é' });
}

#[test]
fn constant_limit() {
    assert_eq!(parse("1000000").bounds(), (1_000_000, 1_000_000));
}

#[test]
fn aggregate_limits() {
    let ok = vec!["1"; 32].join("+");
    assert!(DiceExpr::parse(&ok).is_ok());
    let many = vec!["1"; 33].join("+");
    assert_eq!(err(&many), DiceError::TooManyTerms { max: 32 });

    let ten = ["100d6"; 10].join("+");
    assert!(DiceExpr::parse(&ten).is_ok());
    let eleven = ["100d6"; 11].join("+");
    assert_eq!(err(&eleven), DiceError::TooManyDice { max: 1000 });
    // d66 counts as two dice.
    let mixed = format!("{}+d66", ["100d6"; 10].join("+"));
    assert_eq!(err(&mixed), DiceError::TooManyDice { max: 1000 });
}

#[test]
fn error_messages_name_the_position() {
    assert!(err("2d6x").to_string().contains("position 3"));
}

#[test]
fn bounds_and_totals() {
    assert_eq!(parse("2d6").bounds(), (2, 12));
    assert_eq!(parse("4d6kh3").bounds(), (3, 18));
    assert_eq!(parse("1d6-2").bounds(), (-1, 4));
    assert_eq!(parse("1d6-1d4").bounds(), (-3, 5));
    assert_eq!(parse("d100").bounds(), (1, 100));
    assert_eq!(parse("d66").totals(), Totals::D66);
    assert_eq!(
        parse("1d66").totals(),
        Totals::Contiguous { min: 1, max: 66 }
    );
    assert_eq!(
        parse("2d6+3").totals(),
        Totals::Contiguous { min: 5, max: 15 }
    );
}

#[test]
fn bounds_match_extreme_rolls() {
    for src in ["2d6", "4d6kh3", "1d6+2", "3d8kl2", "d66+d6"] {
        let e = parse(src);
        let (lo, hi) = e.bounds();
        let mut r = MinMax(false);
        assert_eq!(e.roll(&mut r).total, lo, "{src} min");
        let mut r = MinMax(true);
        assert_eq!(e.roll(&mut r).total, hi, "{src} max");
    }
}

/// Always rolls 1 (min) or `sides` (max).
struct MinMax(bool);
impl Roller for MinMax {
    fn roll(&mut self, sides: u32) -> u32 {
        if self.0 {
            sides
        } else {
            1
        }
    }
}
