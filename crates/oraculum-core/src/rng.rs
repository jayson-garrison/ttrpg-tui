//! Randomness (spec 5.1, D-04, D-05).
//!
//! The value stream is owned by this crate: the seed is expanded with our own
//! SplitMix64 and dice values come from our own rejection sampler over
//! `ChaCha8Rng::next_u32`, so upgrading `rand` cannot shift pinned outputs.

use std::collections::VecDeque;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Source of dice rolls.
pub trait Roller {
    /// Uniform integer in `1..=sides`. Contract: `sides >= 2`.
    /// For `sides < 2` implementations return 1 and never panic (D-05).
    fn roll(&mut self, sides: u32) -> u32;
}

/// One step of SplitMix64 (Vigna). Public to the crate for golden tests.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Expand a `u64` seed to the 32-byte ChaCha seed (four SplitMix64 outputs, little endian).
fn expand_seed(seed: u64) -> [u8; 32] {
    let mut state = seed;
    let mut out = [0u8; 32];
    for chunk in out.chunks_exact_mut(8) {
        chunk.copy_from_slice(&splitmix64(&mut state).to_le_bytes());
    }
    out
}

/// Seedable roller backed by `ChaCha8Rng`.
#[derive(Debug, Clone)]
pub struct RngRoller {
    rng: ChaCha8Rng,
}

impl RngRoller {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::from_seed(expand_seed(seed)),
        }
    }
}

impl Roller for RngRoller {
    fn roll(&mut self, sides: u32) -> u32 {
        if sides < 2 {
            return 1;
        }
        let limit = (u32::MAX / sides) * sides;
        loop {
            let x = self.rng.next_u32();
            if x < limit {
                return x % sides + 1;
            }
        }
    }
}

/// A recorded call to [`ScriptedRoller::roll`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollCall {
    pub sides: u32,
    pub returned: u32,
}

/// A script problem noticed by [`ScriptedRoller`]. Tests assert there are none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollFault {
    /// `roll` was called after the script ran out.
    Exhausted { sides: u32 },
    /// The scripted value was not in `1..=sides`.
    OutOfRange { sides: u32, value: u32 },
}

/// Returns a fixed sequence of values. For tests that assert exact results.
///
/// On exhaustion or an out-of-range value it records a fault and returns 1;
/// it never panics.
#[derive(Debug, Clone, Default)]
pub struct ScriptedRoller {
    script: VecDeque<u32>,
    calls: Vec<RollCall>,
    faults: Vec<RollFault>,
}

impl ScriptedRoller {
    pub fn new(values: impl IntoIterator<Item = u32>) -> Self {
        Self {
            script: values.into_iter().collect(),
            calls: Vec::new(),
            faults: Vec::new(),
        }
    }

    /// Every call made so far, in order.
    pub fn calls(&self) -> &[RollCall] {
        &self.calls
    }

    /// True when the script has been fully used.
    pub fn is_consumed(&self) -> bool {
        self.script.is_empty()
    }

    /// Faults recorded so far. Tests assert this is empty.
    pub fn faults(&self) -> &[RollFault] {
        &self.faults
    }
}

impl Roller for ScriptedRoller {
    fn roll(&mut self, sides: u32) -> u32 {
        let returned = match self.script.pop_front() {
            None => {
                self.faults.push(RollFault::Exhausted { sides });
                1
            }
            Some(value) if value >= 1 && value <= sides => value,
            Some(value) => {
                self.faults.push(RollFault::OutOfRange { sides, value });
                1
            }
        };
        self.calls.push(RollCall { sides, returned });
        returned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_reference_vector() {
        // Reference outputs for seed 0 (Vigna's splitmix64.c).
        let mut s = 0u64;
        assert_eq!(splitmix64(&mut s), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(&mut s), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(splitmix64(&mut s), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn expand_seed_is_four_little_endian_outputs() {
        let bytes = expand_seed(0);
        assert_eq!(&bytes[..8], &0xE220_A839_7B1D_CDAFu64.to_le_bytes());
        assert_eq!(&bytes[8..16], &0x6E78_9E6A_A1B9_65F4u64.to_le_bytes());
    }

    #[test]
    fn same_seed_same_stream_different_seed_differs() {
        let draw = |seed| {
            let mut r = RngRoller::from_seed(seed);
            (0..32).map(|_| r.roll(20)).collect::<Vec<_>>()
        };
        assert_eq!(draw(42), draw(42));
        assert_ne!(draw(42), draw(43));
    }

    #[test]
    fn rolls_stay_in_range() {
        let mut r = RngRoller::from_seed(1);
        for sides in [2u32, 3, 6, 20, 100, 1000] {
            for _ in 0..500 {
                let v = r.roll(sides);
                assert!((1..=sides).contains(&v), "{v} not in 1..={sides}");
            }
        }
    }

    #[test]
    fn degenerate_sides_return_one_without_drawing() {
        let mut a = RngRoller::from_seed(9);
        let mut b = RngRoller::from_seed(9);
        assert_eq!(a.roll(0), 1);
        assert_eq!(a.roll(1), 1);
        // No entropy was consumed, so the streams still agree.
        assert_eq!(a.roll(100), b.roll(100));
    }

    #[test]
    fn scripted_returns_values_and_records_calls() {
        let mut r = ScriptedRoller::new([3, 6, 1]);
        assert_eq!(r.roll(6), 3);
        assert_eq!(r.roll(6), 6);
        assert!(!r.is_consumed());
        assert_eq!(r.roll(4), 1);
        assert!(r.is_consumed());
        assert!(r.faults().is_empty());
        assert_eq!(
            r.calls(),
            &[
                RollCall {
                    sides: 6,
                    returned: 3
                },
                RollCall {
                    sides: 6,
                    returned: 6
                },
                RollCall {
                    sides: 4,
                    returned: 1
                },
            ]
        );
    }

    #[test]
    fn scripted_exhaustion_records_fault_and_returns_one() {
        let mut r = ScriptedRoller::new([2]);
        assert_eq!(r.roll(6), 2);
        assert_eq!(r.roll(8), 1);
        assert_eq!(r.faults(), &[RollFault::Exhausted { sides: 8 }]);
    }

    #[test]
    fn scripted_out_of_range_records_fault_and_returns_one() {
        let mut r = ScriptedRoller::new([7, 0]);
        assert_eq!(r.roll(6), 1);
        assert_eq!(r.roll(6), 1);
        assert_eq!(
            r.faults(),
            &[
                RollFault::OutOfRange { sides: 6, value: 7 },
                RollFault::OutOfRange { sides: 6, value: 0 },
            ]
        );
    }
}
