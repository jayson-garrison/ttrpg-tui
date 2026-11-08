use std::rc::Rc;

use super::*;
use crate::catalog::{Catalog, Source};
use crate::clock::FixedClock;
use crate::dice::DiceError;
use crate::error::EvalError;
use crate::journal::Kind;
use crate::oracle::{Likelihood, Twist};
use crate::rng::{RngRoller, ScriptedRoller};

const TS: u64 = 1_762_484_400;

fn catalog() -> Rc<Catalog> {
    let (cat, warnings) = Catalog::build(
        &[
            Source::table("t", "what", "name = \"W\"\nitems = [\"A\", \"B\", \"C\"]"),
            Source::generator(
                "g",
                "event",
                "name = \"E\"\ntemplate = \"event: {w}\"\n[[slots]]\nkey = \"w\"\ntable = \"what\"",
            ),
            Source::generator(
                "g",
                "pair",
                "name = \"P\"\ntemplate = \"{a}/{b}\"\n[[slots]]\nkey = \"a\"\ntable = \"what\"\n[[slots]]\nkey = \"b\"\ntable = \"what\"",
            ),
        ],
        &[],
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    Rc::new(cat)
}

fn engine(script: &[u32]) -> Engine {
    Engine::new(
        catalog(),
        Box::new(ScriptedRoller::new(script.iter().copied())),
        Box::new(FixedClock(TS)),
    )
}

#[test]
fn dice_entry_carries_the_exact_result_text_and_clock_time() {
    let mut e = engine(&[14, 3, 17]);
    let rolled = e.roll_dice("  3d20kl1+2 ").unwrap();
    assert_eq!(rolled.entry.kind, Kind::Dice);
    assert_eq!(rolled.entry.input, "3d20kl1+2");
    // The input is kept as typed (trimmed), the result shows the canonical form.
    let typed = engine(&[1]).roll_dice(" 1D6 ").unwrap();
    assert_eq!(
        (typed.entry.input.as_str(), typed.entry.result.as_str()),
        ("1D6", "1d6 => [1] = 1")
    );
    assert_eq!(
        rolled.entry.result,
        "3d20kl1+2 => [14, 3, 17] keep [3] +2 = 5"
    );
    assert_eq!(rolled.entry.ts, TS);
    assert_eq!(rolled.entry.v, 1);
    assert_eq!(rolled.entry.detail["total"], 5);
    assert!(matches!(rolled.result, RollResult::Dice(ref d) if d.total == 5));
}

#[test]
fn bad_dice_is_a_typed_error_and_consumes_nothing() {
    let mut e = engine(&[]);
    assert_eq!(
        e.roll_dice("101d6").unwrap_err(),
        DiceError::CountOutOfRange { pos: 0, got: 101 }
    );
}

#[test]
fn oracle_without_twist_is_one_line() {
    let mut e = engine(&[51]);
    let r = e.ask(Likelihood::Even, None);
    assert_eq!(r.entry.kind, Kind::Oracle);
    assert_eq!(r.entry.input, "Even");
    assert_eq!(r.entry.result, "Even: No (d100: 51)");
}

#[test]
fn oracle_with_question_and_twist_event() {
    let mut e = engine(&[11, 3]);
    let r = e.ask(Likelihood::Even, Some("  Is it locked? "));
    assert_eq!(r.entry.input, "Even: Is it locked?");
    assert_eq!(r.entry.result, "Even: Yes (d100: 11)\nTwist: event: C");
    assert_eq!(r.entry.detail["roll"], 11);
    assert_eq!(r.entry.detail["outcome"], "Yes");
    assert_eq!(r.entry.detail["question"], "Is it locked?");
    assert!(matches!(r.result, RollResult::Oracle(ref o) if matches!(o.twist, Twist::Event(_))));
}

#[test]
fn blank_question_counts_as_none() {
    let mut e = engine(&[50]);
    let r = e.ask(Likelihood::Likely, Some("   "));
    assert_eq!(r.entry.input, "Likely");
    assert_eq!(r.entry.result, "Likely: Yes (d100: 50)");
}

#[test]
fn unavailable_twist_names_what_is_missing() {
    let mut e = engine(&[99]).with_twist_generator("nope");
    let r = e.ask(Likelihood::VeryLikely, None);
    assert_eq!(
        r.entry.result,
        "Very likely: Exceptional No (d100: 99)\nTwist: [unavailable: nope]"
    );
}

#[test]
fn custom_twist_generator_is_used() {
    let mut e = engine(&[22, 1, 2]).with_twist_generator("pair");
    let r = e.ask(Likelihood::Even, None);
    assert_eq!(r.entry.result, "Even: Yes (d100: 22)\nTwist: A/B");
}

#[test]
fn table_and_generator_entries() {
    let mut e = engine(&[2, 3, 1]);
    let t = e.roll_table("what").unwrap();
    assert_eq!(
        (
            t.entry.kind,
            t.entry.input.as_str(),
            t.entry.result.as_str()
        ),
        (Kind::Table, "what", "B")
    );
    let g = e.generate("pair").unwrap();
    assert_eq!(g.entry.kind, Kind::Generator);
    assert_eq!(g.entry.input, "pair");
    assert_eq!(g.entry.result, "C/A");
}

#[test]
fn unknown_ids_are_eval_errors() {
    let mut e = engine(&[]);
    assert_eq!(
        e.roll_table("zzz").unwrap_err(),
        EvalError::UnknownTable("zzz".into())
    );
    assert_eq!(
        e.generate("zzz").unwrap_err(),
        EvalError::UnknownGenerator("zzz".into())
    );
}

#[test]
fn slot_reroll_is_journaled_with_the_slot_key() {
    let mut e = engine(&[1, 2, 3]);
    let first = e.generate("pair").unwrap();
    let RollResult::Generator(prev) = &first.result else {
        panic!("not a generator result")
    };
    let again = e.reroll_slot(prev, 1).unwrap();
    assert_eq!(again.entry.input, "pair [slot b]");
    assert_eq!(again.entry.result, "A/C");
    assert_eq!(again.entry.detail["rerolled_slot"], "b");
}

#[test]
fn notes_are_entries_without_input() {
    let e = engine(&[]);
    let n = e.note("Remember the key.");
    assert_eq!(
        (n.kind, n.input.as_str(), n.result.as_str(), n.ts),
        (Kind::Note, "", "Remember the key.", TS)
    );
    assert!(n.detail.is_null());
}

#[test]
fn same_seed_gives_identical_entries() {
    let run = || {
        let mut e = Engine::new(
            catalog(),
            Box::new(RngRoller::from_seed(7)),
            Box::new(FixedClock(TS)),
        );
        let a = e.roll_dice("4d6kh3").unwrap().entry;
        let b = e.ask(Likelihood::Even, Some("q")).entry;
        let c = e.generate("pair").unwrap().entry;
        (a, b, c)
    };
    assert_eq!(run(), run());
}
