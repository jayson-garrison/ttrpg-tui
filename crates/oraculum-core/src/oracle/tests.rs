use super::*;
use crate::rng::{RollCall, ScriptedRoller};

/// Spec 5.3 rate table: (likelihood, yes chance, exceptional yes max, exceptional no min).
const SPEC_TABLE: [(Likelihood, u8, u8, u8); 5] = [
    (Likelihood::VeryUnlikely, 10, 2, 83),
    (Likelihood::Unlikely, 30, 6, 87),
    (Likelihood::Even, 50, 10, 91),
    (Likelihood::Likely, 70, 14, 95),
    (Likelihood::VeryLikely, 90, 18, 99),
];

fn check(l: Likelihood, roll: u8, outcome: Outcome, twist: bool) {
    let r = evaluate(l, roll);
    assert_eq!(
        (r.outcome, r.twist),
        (outcome, twist),
        "{} / {roll}",
        l.label()
    );
    assert_eq!((r.likelihood, r.roll), (l, roll));
}

#[test]
fn spec_worked_examples() {
    use Likelihood::*;
    use Outcome::*;
    check(Even, 10, ExceptionalYes, false);
    check(Even, 11, Yes, true);
    check(Even, 50, Yes, false);
    check(Even, 51, No, false);
    check(Even, 90, No, false);
    check(Even, 91, ExceptionalNo, false);
    check(VeryLikely, 18, ExceptionalYes, false);
    check(VeryLikely, 99, ExceptionalNo, true);
    check(VeryUnlikely, 2, ExceptionalYes, false);
    check(VeryUnlikely, 83, ExceptionalNo, false);
}

#[test]
fn spec_table_equals_formula() {
    for (l, chance, exc_yes_max, exc_no_min) in SPEC_TABLE {
        assert_eq!(l.yes_chance(), chance);
        let mut yes_max_exc = 0;
        let mut no_min_exc = 101;
        for roll in 1..=100u8 {
            match evaluate(l, roll).outcome {
                Outcome::ExceptionalYes => yes_max_exc = yes_max_exc.max(roll),
                Outcome::ExceptionalNo => no_min_exc = no_min_exc.min(roll),
                _ => {}
            }
        }
        assert_eq!(yes_max_exc, exc_yes_max, "{}", l.label());
        assert_eq!(no_min_exc, exc_no_min, "{}", l.label());
    }
}

#[test]
fn exhaustive_counts_per_likelihood() {
    let mut yes_counts = Vec::new();
    for l in Likelihood::ALL {
        let results: Vec<_> = (1..=100u8).map(|r| evaluate(l, r)).collect();
        let yes = results.iter().filter(|r| r.outcome.is_yes()).count();
        assert_eq!(yes, usize::from(l.yes_chance()), "{}", l.label());
        let chance = usize::from(l.yes_chance());
        let exc_yes = results
            .iter()
            .filter(|r| r.outcome == Outcome::ExceptionalYes)
            .count();
        let exc_no = results
            .iter()
            .filter(|r| r.outcome == Outcome::ExceptionalNo)
            .count();
        assert_eq!(exc_yes, chance / 5, "{}", l.label());
        assert_eq!(exc_no, (100 - chance) / 5, "{}", l.label());
        let twists: Vec<u8> = results.iter().filter(|r| r.twist).map(|r| r.roll).collect();
        assert_eq!(
            twists,
            [11, 22, 33, 44, 55, 66, 77, 88, 99],
            "{}",
            l.label()
        );
        yes_counts.push(yes);
    }
    assert!(yes_counts.windows(2).all(|w| w[0] < w[1]), "{yes_counts:?}");
}

#[test]
fn hundred_is_not_a_twist() {
    for l in Likelihood::ALL {
        assert!(!evaluate(l, 100).twist);
    }
}

#[test]
fn evaluate_clamps_out_of_range_input() {
    assert_eq!(evaluate(Likelihood::Even, 0).roll, 1);
    assert_eq!(evaluate(Likelihood::Even, 200).roll, 100);
}

#[test]
fn labels_and_slugs() {
    let labels: Vec<_> = Likelihood::ALL.iter().map(|l| l.label()).collect();
    assert_eq!(
        labels,
        ["Very unlikely", "Unlikely", "Even", "Likely", "Very likely"]
    );
    for l in Likelihood::ALL {
        assert_eq!(Likelihood::from_slug(l.slug()), Some(l));
    }
    assert_eq!(
        Likelihood::from_slug("Very-Likely"),
        Some(Likelihood::VeryLikely)
    );
    assert_eq!(Likelihood::from_slug("maybe"), None);
    assert_eq!(Outcome::ExceptionalYes.label(), "Exceptional Yes");
}

/// Test twist source: rolls one d6 and returns it, or reports a missing id.
struct Source {
    missing: Option<&'static str>,
}

impl TwistSource for Source {
    type Event = u32;
    fn roll_twist(&self, generator: &str, roller: &mut dyn Roller) -> Result<u32, String> {
        match self.missing {
            Some(m) => Err(m.to_string()),
            None => {
                assert_eq!(generator, "event");
                Ok(roller.roll(6))
            }
        }
    }
}

#[test]
fn ask_without_twist_consumes_exactly_one_roll() {
    let mut r = ScriptedRoller::new([50]);
    let res = ask(
        &Source { missing: None },
        &mut r,
        Likelihood::Even,
        Some("Is it locked?"),
        "event",
    );
    assert_eq!(res.roll.outcome, Outcome::Yes);
    assert_eq!(res.twist, Twist::None);
    assert_eq!(res.question.as_deref(), Some("Is it locked?"));
    assert_eq!(
        r.calls(),
        &[RollCall {
            sides: 100,
            returned: 50
        }]
    );
    assert!(r.is_consumed() && r.faults().is_empty());
}

#[test]
fn ask_with_twist_rolls_the_generator_after_the_d100() {
    let mut r = ScriptedRoller::new([11, 4]);
    let res = ask(
        &Source { missing: None },
        &mut r,
        Likelihood::Even,
        None,
        "event",
    );
    assert_eq!(res.roll.outcome, Outcome::Yes);
    assert!(res.roll.twist);
    assert_eq!(res.twist, Twist::Event(4));
    assert_eq!(res.question, None);
    assert_eq!(
        r.calls(),
        &[
            RollCall {
                sides: 100,
                returned: 11
            },
            RollCall {
                sides: 6,
                returned: 4
            },
        ]
    );
}

#[test]
fn missing_generator_is_a_message_not_an_error() {
    let mut r = ScriptedRoller::new([99]);
    let res = ask(
        &Source {
            missing: Some("event"),
        },
        &mut r,
        Likelihood::VeryLikely,
        None,
        "event",
    );
    assert_eq!(res.roll.outcome, Outcome::ExceptionalNo);
    assert_eq!(
        res.twist,
        Twist::Unavailable {
            generator: "event".into(),
            missing: "event".into()
        }
    );
}

#[test]
fn missing_table_is_named() {
    let mut r = ScriptedRoller::new([22]);
    let res = ask(
        &Source {
            missing: Some("meaning_action"),
        },
        &mut r,
        Likelihood::Even,
        None,
        "event",
    );
    assert_eq!(
        res.twist,
        Twist::Unavailable {
            generator: "event".into(),
            missing: "meaning_action".into()
        }
    );
}
