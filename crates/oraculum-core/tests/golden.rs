//! Golden vectors: the value stream is owned by this repo (D-04). If a
//! dependency upgrade changes any of these, this test fails loudly.
//! The vectors were recorded from this implementation, not from an
//! independent source; the SplitMix64 step itself is checked against Vigna's
//! reference outputs in `rng::tests`.

use oraculum_core::dice::DiceExpr;
use oraculum_core::rng::{RngRoller, Roller};

fn stream(seed: u64, sides: u32, n: usize) -> Vec<u32> {
    let mut r = RngRoller::from_seed(seed);
    (0..n).map(|_| r.roll(sides)).collect()
}

fn roll_line(src: &str, seed: u64) -> String {
    match DiceExpr::parse(src) {
        Ok(expr) => expr.roll(&mut RngRoller::from_seed(seed)).to_string(),
        Err(e) => format!("parse error: {e}"),
    }
}

#[test]
fn golden_streams() {
    assert_eq!(
        stream(0, 20, 12),
        [9, 20, 6, 6, 15, 13, 20, 8, 16, 20, 15, 16]
    );
    assert_eq!(stream(42, 6, 12), [3, 4, 6, 5, 4, 6, 1, 4, 6, 4, 2, 6]);
    assert_eq!(
        stream(u64::MAX, 1000, 8),
        [213, 693, 175, 645, 634, 843, 952, 411]
    );
}

#[test]
fn golden_roll_lines() {
    assert_eq!(
        roll_line("3d20kl1+2", 7),
        "3d20kl1+2 => [11, 17, 2] keep [2] +2 = 4"
    );
    assert_eq!(roll_line("d66", 7), "d66 => [5, 3]=53 = 53");
}
