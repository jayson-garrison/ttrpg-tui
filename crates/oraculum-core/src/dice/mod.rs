//! Dice expressions (spec 5.2; D-07, D-08, D-09).
//!
//! `DiceExpr` is always validated: parsing is the only way to build one, so
//! rolling is infallible.

mod display;
mod parse;
mod roll;

use thiserror::Error;

/// Largest value a constant term may have.
pub const MAX_CONSTANT: u32 = 1_000_000;
/// Dice per term: `1..=MAX_COUNT`.
pub const MAX_COUNT: u8 = 100;
/// Sides per die: `MIN_SIDES..=MAX_SIDES`.
pub const MIN_SIDES: u16 = 2;
pub const MAX_SIDES: u16 = 1000;
/// Aggregate limits (D-09).
pub const MAX_TERMS: usize = 32;
pub const MAX_TOTAL_DICE: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Sign {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum KeepKind {
    Highest,
    Lowest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Keep {
    pub which: KeepKind,
    /// `1..=count`.
    pub n: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DiceSpec {
    pub count: u8,
    pub sides: u16,
    pub keep: Option<Keep>,
}

impl DiceSpec {
    /// Number of dice that count toward the total.
    fn kept_count(&self) -> u32 {
        u32::from(self.keep.map_or(self.count, |k| k.n))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Term {
    Const(u32),
    Dice(DiceSpec),
    /// Bare `d66`: two d6 read as tens and units (D-07).
    D66,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedTerm {
    pub sign: Sign,
    pub term: Term,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiceExpr {
    terms: Vec<SignedTerm>,
}

/// Which totals an expression can produce, for table coverage (D-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Totals {
    /// Every integer from `min` to `max`.
    Contiguous { min: i64, max: i64 },
    /// The 36 values with both digits in 1..=6. Only for the bare term `d66`.
    D66,
}

/// Parse errors. `pos` is a byte offset into the source string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiceError {
    #[error("empty dice expression")]
    Empty,
    #[error("unexpected character {ch:?} at position {pos}")]
    UnexpectedChar { pos: usize, ch: char },
    #[error("expected {expected} at position {pos}")]
    UnexpectedToken { pos: usize, expected: &'static str },
    #[error("number too large at position {pos}")]
    NumberTooLarge { pos: usize },
    #[error("dice count {got} at position {pos} is out of range (1 to 100)")]
    CountOutOfRange { pos: usize, got: u64 },
    #[error("die sides {got} at position {pos} is out of range (2 to 1000)")]
    SidesOutOfRange { pos: usize, got: u64 },
    #[error("keep count {k} at position {pos} is out of range (1 to {n})")]
    KeepOutOfRange { pos: usize, k: u64, n: u8 },
    #[error("too many terms (at most {max})")]
    TooManyTerms { max: usize },
    #[error("too many dice (at most {max} in total)")]
    TooManyDice { max: usize },
}

/// One rolled term. Mirrors `SignedTerm`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TermRoll {
    pub sign: Sign,
    pub kind: TermRollKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum TermRollKind {
    Const(u32),
    Dice {
        spec: DiceSpec,
        /// In roll order.
        rolls: Vec<u32>,
        /// Parallel to `rolls`: whether each die counts.
        kept: Vec<bool>,
        subtotal: u32,
    },
    D66 {
        tens: u32,
        units: u32,
        /// `tens * 10 + units`.
        subtotal: u32,
    },
}

impl TermRollKind {
    /// Unsigned value this term contributes.
    pub fn subtotal(&self) -> u32 {
        match self {
            Self::Const(c) => *c,
            Self::Dice { subtotal, .. } | Self::D66 { subtotal, .. } => *subtotal,
        }
    }
}

/// Full detail of one evaluated expression. `Display` is the fixed snapshot format.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiceRoll {
    /// Canonical form of the expression.
    pub expr: String,
    pub terms: Vec<TermRoll>,
    pub total: i64,
}

impl DiceExpr {
    /// Parse an expression. Never panics.
    pub fn parse(src: &str) -> Result<Self, DiceError> {
        parse::parse(src).map(|terms| Self { terms })
    }

    pub fn terms(&self) -> &[SignedTerm] {
        &self.terms
    }

    /// Lowercase, no spaces, e.g. `3d20kl1+2`. Also the `Display` form.
    pub fn canonical(&self) -> String {
        display::canonical(self)
    }

    /// Evaluate. Roll order: terms left to right; an `NdM` term makes `N`
    /// sequential `roll(M)` calls; `d66` calls `roll(6)` twice, tens first.
    pub fn roll(&self, roller: &mut dyn crate::rng::Roller) -> DiceRoll {
        roll::roll(self, roller)
    }

    /// Smallest and largest possible total.
    pub fn bounds(&self) -> (i64, i64) {
        let (mut lo, mut hi) = (0i64, 0i64);
        for st in &self.terms {
            let (tlo, thi) = match st.term {
                Term::Const(c) => (i64::from(c), i64::from(c)),
                Term::Dice(spec) => {
                    let n = i64::from(spec.kept_count());
                    (n, n * i64::from(spec.sides))
                }
                Term::D66 => (11, 66),
            };
            match st.sign {
                Sign::Plus => {
                    lo += tlo;
                    hi += thi;
                }
                Sign::Minus => {
                    lo -= thi;
                    hi -= tlo;
                }
            }
        }
        (lo, hi)
    }

    /// True for the bare expression `d66` and nothing else.
    pub fn is_bare_d66(&self) -> bool {
        matches!(
            self.terms.as_slice(),
            [SignedTerm {
                sign: Sign::Plus,
                term: Term::D66
            }]
        )
    }

    /// True if any term is `d66` (such expressions are not valid table dice, D-11).
    pub fn contains_d66(&self) -> bool {
        self.terms.iter().any(|t| t.term == Term::D66)
    }

    /// Totals for table coverage. For compound expressions containing `d66`
    /// this reports the numeric bounds; callers reject those via `contains_d66`.
    pub fn totals(&self) -> Totals {
        if self.is_bare_d66() {
            Totals::D66
        } else {
            let (min, max) = self.bounds();
            Totals::Contiguous { min, max }
        }
    }
}

impl std::fmt::Display for DiceExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.canonical())
    }
}

#[cfg(test)]
mod tests;
