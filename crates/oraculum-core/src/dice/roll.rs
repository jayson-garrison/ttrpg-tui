//! Evaluation.

use super::{DiceExpr, DiceRoll, DiceSpec, KeepKind, Sign, Term, TermRoll, TermRollKind};
use crate::rng::Roller;

pub(super) fn roll(expr: &DiceExpr, roller: &mut dyn Roller) -> DiceRoll {
    let mut terms = Vec::with_capacity(expr.terms.len());
    let mut total = 0i64;
    for st in &expr.terms {
        let kind = match st.term {
            Term::Const(c) => TermRollKind::Const(c),
            Term::Dice(spec) => roll_dice(spec, roller),
            Term::D66 => {
                let tens = roller.roll(6);
                let units = roller.roll(6);
                TermRollKind::D66 {
                    tens,
                    units,
                    subtotal: tens * 10 + units,
                }
            }
        };
        let value = i64::from(kind.subtotal());
        match st.sign {
            Sign::Plus => total += value,
            Sign::Minus => total -= value,
        }
        terms.push(TermRoll {
            sign: st.sign,
            kind,
        });
    }
    DiceRoll {
        expr: expr.canonical(),
        terms,
        total,
    }
}

fn roll_dice(spec: DiceSpec, roller: &mut dyn Roller) -> TermRollKind {
    let sides = u32::from(spec.sides);
    let rolls: Vec<u32> = (0..spec.count).map(|_| roller.roll(sides)).collect();
    let mut kept = vec![true; rolls.len()];
    if let Some(keep) = spec.keep {
        // Highest or lowest first; ties broken by lower index first.
        let mut order: Vec<usize> = (0..rolls.len()).collect();
        match keep.which {
            KeepKind::Highest => order.sort_by(|&a, &b| rolls[b].cmp(&rolls[a]).then(a.cmp(&b))),
            KeepKind::Lowest => order.sort_by(|&a, &b| rolls[a].cmp(&rolls[b]).then(a.cmp(&b))),
        }
        kept.iter_mut().for_each(|k| *k = false);
        for &i in order.iter().take(usize::from(keep.n)) {
            kept[i] = true;
        }
    }
    let subtotal = rolls
        .iter()
        .zip(&kept)
        .filter(|(_, &k)| k)
        .map(|(&v, _)| v)
        .sum();
    TermRollKind::Dice {
        spec,
        rolls,
        kept,
        subtotal,
    }
}
