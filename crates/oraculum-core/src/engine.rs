//! The single orchestration point shared by the CLI and the TUI (A§3.10, D-03).
//!
//! `Engine` rolls things and builds the journal [`Entry`] for each roll, so
//! "headless prints what the journal stores" holds by construction.

use std::rc::Rc;

use serde_json::{json, Value};

use crate::catalog::Catalog;
use crate::clock::Clock;
use crate::dice::{DiceError, DiceExpr, DiceRoll};
use crate::error::EvalError;
use crate::generator::GeneratorResult;
use crate::journal::{Entry, Kind};
use crate::oracle::{self, Likelihood, OracleResult, Twist};
use crate::rng::Roller;
use crate::table::TableRoll;

/// Default id of the generator rolled for oracle twists (D-26).
pub const DEFAULT_TWIST_GENERATOR: &str = "event";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollResult {
    Dice(DiceRoll),
    Oracle(OracleResult<GeneratorResult>),
    Table(TableRoll),
    Generator(GeneratorResult),
}

/// A roll together with the journal entry that records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rolled {
    pub result: RollResult,
    pub entry: Entry,
}

pub struct Engine {
    catalog: Rc<Catalog>,
    roller: Box<dyn Roller>,
    clock: Box<dyn Clock>,
    twist_generator: String,
}

impl Engine {
    pub fn new(catalog: Rc<Catalog>, roller: Box<dyn Roller>, clock: Box<dyn Clock>) -> Self {
        Self {
            catalog,
            roller,
            clock,
            twist_generator: DEFAULT_TWIST_GENERATOR.to_string(),
        }
    }

    pub fn with_twist_generator(mut self, id: impl Into<String>) -> Self {
        self.twist_generator = id.into();
        self
    }

    pub fn catalog(&self) -> &Rc<Catalog> {
        &self.catalog
    }

    pub fn roll_dice(&mut self, expr: &str) -> Result<Rolled, DiceError> {
        let src = expr.trim();
        let expr = DiceExpr::parse(src)?;
        let roll = expr.roll(self.roller.as_mut());
        let entry = self.entry(
            Kind::Dice,
            src.to_string(),
            roll.to_string(),
            to_json(&roll),
        );
        Ok(Rolled {
            result: RollResult::Dice(roll),
            entry,
        })
    }

    pub fn ask(&mut self, likelihood: Likelihood, question: Option<&str>) -> Rolled {
        let question = question.map(str::trim).filter(|q| !q.is_empty());
        let res = oracle::ask(
            self.catalog.as_ref(),
            self.roller.as_mut(),
            likelihood,
            question,
            &self.twist_generator,
        );
        let input = match question {
            Some(q) => format!("{}: {q}", likelihood.label()),
            None => likelihood.label().to_string(),
        };
        let mut text = format!(
            "{}: {} (d100: {})",
            likelihood.label(),
            res.roll.outcome.label(),
            res.roll.roll
        );
        match &res.twist {
            Twist::None => {}
            Twist::Event(g) => text.push_str(&format!("\nTwist: {}", g.text)),
            Twist::Unavailable { missing, .. } => {
                text.push_str(&format!("\nTwist: [unavailable: {missing}]"));
            }
        }
        let detail = json!({
            "likelihood": likelihood.slug(),
            "roll": res.roll.roll,
            "outcome": res.roll.outcome.label(),
            "question": question,
            "twist": match &res.twist {
                Twist::None => Value::Null,
                Twist::Event(g) => to_json(g),
                Twist::Unavailable { generator, missing } => {
                    json!({"unavailable": {"generator": generator, "missing": missing}})
                }
            },
        });
        let entry = self.entry(Kind::Oracle, input, text, detail);
        Rolled {
            result: RollResult::Oracle(res),
            entry,
        }
    }

    pub fn roll_table(&mut self, id: &str) -> Result<Rolled, EvalError> {
        let roll = self.catalog.roll_table(id, self.roller.as_mut())?;
        let entry = self.entry(
            Kind::Table,
            id.to_string(),
            roll.text.clone(),
            to_json(&roll),
        );
        Ok(Rolled {
            result: RollResult::Table(roll),
            entry,
        })
    }

    pub fn generate(&mut self, id: &str) -> Result<Rolled, EvalError> {
        let res = self.catalog.generate(id, self.roller.as_mut())?;
        let entry = self.entry(
            Kind::Generator,
            id.to_string(),
            res.text.clone(),
            to_json(&res),
        );
        Ok(Rolled {
            result: RollResult::Generator(res),
            entry,
        })
    }

    /// Re-roll one slot of a previous result; the entry records which slot (D-45).
    pub fn reroll_slot(
        &mut self,
        prev: &GeneratorResult,
        slot: usize,
    ) -> Result<Rolled, EvalError> {
        let res = self.catalog.reroll_slot(prev, slot, self.roller.as_mut())?;
        let key = res
            .slots
            .get(slot)
            .map_or("", |s| s.key.as_str())
            .to_string();
        let mut detail = to_json(&res);
        if let Value::Object(map) = &mut detail {
            map.insert("rerolled_slot".to_string(), Value::String(key.clone()));
        }
        let entry = self.entry(
            Kind::Generator,
            format!("{} [slot {key}]", res.id),
            res.text.clone(),
            detail,
        );
        Ok(Rolled {
            result: RollResult::Generator(res),
            entry,
        })
    }

    pub fn note(&self, text: &str) -> Entry {
        self.entry(Kind::Note, String::new(), text.to_string(), Value::Null)
    }

    fn entry(&self, kind: Kind, input: String, result: String, detail: Value) -> Entry {
        Entry::new(self.clock.now_secs(), kind, input, result, detail)
    }
}

/// Entry detail is informational (D-23); serialization cannot fail for these
/// types, but fall back to null rather than panic.
fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests;
