//! Yes/no oracle (spec 5.3; D-33, D-50).

use crate::rng::Roller;

/// How likely the answer is "yes".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Likelihood {
    VeryUnlikely,
    Unlikely,
    Even,
    Likely,
    VeryLikely,
}

impl Likelihood {
    pub const ALL: [Likelihood; 5] = [
        Self::VeryUnlikely,
        Self::Unlikely,
        Self::Even,
        Self::Likely,
        Self::VeryLikely,
    ];

    /// Percent chance of "yes".
    pub fn yes_chance(self) -> u8 {
        match self {
            Self::VeryUnlikely => 10,
            Self::Unlikely => 30,
            Self::Even => 50,
            Self::Likely => 70,
            Self::VeryLikely => 90,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::VeryUnlikely => "Very unlikely",
            Self::Unlikely => "Unlikely",
            Self::Even => "Even",
            Self::Likely => "Likely",
            Self::VeryLikely => "Very likely",
        }
    }

    /// CLI and journal form, e.g. `very-unlikely`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::VeryUnlikely => "very-unlikely",
            Self::Unlikely => "unlikely",
            Self::Even => "even",
            Self::Likely => "likely",
            Self::VeryLikely => "very-likely",
        }
    }

    /// Parse a slug (`even`, `very-likely`, ...). ASCII case-insensitive.
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|l| l.slug().eq_ignore_ascii_case(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    ExceptionalYes,
    Yes,
    No,
    ExceptionalNo,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::ExceptionalYes => "Exceptional Yes",
            Self::Yes => "Yes",
            Self::No => "No",
            Self::ExceptionalNo => "Exceptional No",
        }
    }

    pub fn is_yes(self) -> bool {
        matches!(self, Self::ExceptionalYes | Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleRoll {
    pub likelihood: Likelihood,
    /// The d100 result, `1..=100`.
    pub roll: u8,
    pub outcome: Outcome,
    /// True when `roll` is a double (11, 22, ..., 99); 100 is not a double.
    pub twist: bool,
}

/// Pure evaluation of a d100 roll. `roll` is clamped into `1..=100`.
pub fn evaluate(likelihood: Likelihood, roll: u8) -> OracleRoll {
    let roll = roll.clamp(1, 100);
    let chance = likelihood.yes_chance();
    let outcome = if roll <= chance {
        if roll <= chance / 5 {
            Outcome::ExceptionalYes
        } else {
            Outcome::Yes
        }
    } else if roll > 100 - (100 - chance) / 5 {
        Outcome::ExceptionalNo
    } else {
        Outcome::No
    };
    let twist = roll < 100 && roll.is_multiple_of(11);
    OracleRoll {
        likelihood,
        roll,
        outcome,
        twist,
    }
}

/// Where twist events come from. The catalog implements this (T4, D-50).
pub trait TwistSource {
    type Event;

    /// Roll the generator `generator`. On failure return the id of what is
    /// missing (the generator itself, or a table it needs).
    fn roll_twist(&self, generator: &str, roller: &mut dyn Roller) -> Result<Self::Event, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Twist<E> {
    None,
    Event(E),
    /// The twist could not be generated. Never an error (spec 5.3.4).
    Unavailable {
        generator: String,
        missing: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleResult<E> {
    pub roll: OracleRoll,
    pub question: Option<String>,
    pub twist: Twist<E>,
}

/// Ask the oracle. Rolls d100 first; only on a twist is the generator rolled,
/// so a non-twist question consumes exactly one roll.
pub fn ask<S: TwistSource>(
    source: &S,
    roller: &mut dyn Roller,
    likelihood: Likelihood,
    question: Option<&str>,
    twist_generator: &str,
) -> OracleResult<S::Event> {
    // roll(100) is in 1..=100 for a conforming roller; evaluate clamps anyway.
    let d100 = roller.roll(100).clamp(1, 100) as u8;
    let roll = evaluate(likelihood, d100);
    let twist = if roll.twist {
        match source.roll_twist(twist_generator, roller) {
            Ok(event) => Twist::Event(event),
            Err(missing) => Twist::Unavailable {
                generator: twist_generator.to_string(),
                missing,
            },
        }
    } else {
        Twist::None
    };
    OracleResult {
        roll,
        question: question.map(str::to_string),
        twist,
    }
}

#[cfg(test)]
mod tests;
