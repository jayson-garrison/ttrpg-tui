use oraculum_core::dice::{DiceExpr, TermRollKind};
use oraculum_core::rng::RngRoller;
use proptest::prelude::*;

fn arb_term() -> impl Strategy<Value = String> {
    prop_oneof![
        (0u32..=1_000_000).prop_map(|c| c.to_string()),
        Just("d66".to_string()),
        (1u32..=100, 2u32..=1000).prop_map(|(n, m)| format!("{n}d{m}")),
        (1u32..=100, 2u32..=1000, 0u32..2, 1u32..=100).prop_map(|(n, m, w, k)| {
            let k = (k - 1) % n + 1;
            format!("{n}d{m}{}{k}", if w == 0 { "kh" } else { "kl" })
        }),
    ]
}

fn arb_expr() -> impl Strategy<Value = String> {
    (
        arb_term(),
        prop::collection::vec((prop::bool::ANY, arb_term()), 0..4),
    )
        .prop_map(|(first, rest)| {
            let mut s = first;
            for (plus, t) in rest {
                s.push(if plus { '+' } else { '-' });
                s.push_str(&t);
            }
            s
        })
}

proptest! {
    #[test]
    fn arbitrary_strings_never_panic(s in ".*") {
        let _ = DiceExpr::parse(&s);
    }

    #[test]
    fn arbitrary_bytes_never_panic(b in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = DiceExpr::parse(&String::from_utf8_lossy(&b));
    }

    #[test]
    fn dice_in_range_and_total_is_kept_plus_constants(src in arb_expr(), seed in any::<u64>()) {
        // Skip expressions that exceed the aggregate dice limit.
        let Ok(expr) = DiceExpr::parse(&src) else { return Ok(()); };
        let mut roller = RngRoller::from_seed(seed);
        let roll = expr.roll(&mut roller);
        let mut total = 0i64;
        for t in &roll.terms {
            let sign = match t.sign {
                oraculum_core::dice::Sign::Plus => 1i64,
                oraculum_core::dice::Sign::Minus => -1i64,
            };
            let sub = match &t.kind {
                TermRollKind::Const(c) => i64::from(*c),
                TermRollKind::Dice { spec, rolls, kept, subtotal } => {
                    prop_assert_eq!(rolls.len(), usize::from(spec.count));
                    for &v in rolls {
                        prop_assert!((1..=u32::from(spec.sides)).contains(&v));
                    }
                    let want = spec.keep.map_or(spec.count, |k| k.n);
                    prop_assert_eq!(kept.iter().filter(|&&k| k).count(), usize::from(want));
                    let sum: u32 = rolls.iter().zip(kept).filter(|(_, &k)| k).map(|(&v, _)| v).sum();
                    prop_assert_eq!(sum, *subtotal);
                    i64::from(sum)
                }
                TermRollKind::D66 { tens, units, subtotal } => {
                    prop_assert!((1..=6).contains(tens) && (1..=6).contains(units));
                    prop_assert_eq!(tens * 10 + units, *subtotal);
                    i64::from(*subtotal)
                }
            };
            total += sign * sub;
        }
        prop_assert_eq!(total, roll.total);
        let (lo, hi) = expr.bounds();
        prop_assert!(lo <= roll.total && roll.total <= hi);
    }

    #[test]
    fn kept_dice_are_extremes(n in 1u32..=20, m in 2u32..=100, k_raw in 1u32..=20, high in any::<bool>(), seed in any::<u64>()) {
        let k = (k_raw - 1) % n + 1;
        let src = format!("{n}d{m}{}{k}", if high { "kh" } else { "kl" });
        let expr = DiceExpr::parse(&src).unwrap();
        let roll = expr.roll(&mut RngRoller::from_seed(seed));
        let TermRollKind::Dice { rolls, kept, .. } = &roll.terms[0].kind else {
            return Err(TestCaseError::fail("expected dice term"));
        };
        let mut sorted = rolls.clone();
        sorted.sort_unstable();
        let want: u32 = if high {
            sorted.iter().rev().take(k as usize).sum()
        } else {
            sorted.iter().take(k as usize).sum()
        };
        let got: u32 = rolls.iter().zip(kept).filter(|(_, &k)| k).map(|(&v, _)| v).sum();
        prop_assert_eq!(got, want);
    }

    #[test]
    fn canonical_round_trips(src in arb_expr()) {
        if let Ok(expr) = DiceExpr::parse(&src) {
            let again = DiceExpr::parse(&expr.canonical());
            prop_assert_eq!(again, Ok(expr));
        }
    }

    #[test]
    fn rolling_is_deterministic_per_seed(src in arb_expr(), seed in any::<u64>()) {
        if let Ok(expr) = DiceExpr::parse(&src) {
            let a = expr.roll(&mut RngRoller::from_seed(seed));
            let b = expr.roll(&mut RngRoller::from_seed(seed));
            prop_assert_eq!(a, b);
        }
    }
}
