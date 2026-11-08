//! Generators (spec 5.5; A§3.7; D-12..D-16).

use std::collections::BTreeSet;

use serde::Deserialize;
use thiserror::Error;

use crate::dice::{DiceError, DiceExpr, DiceRoll};
use crate::error::{EvalError, MAX_DEPTH};
use crate::rng::Roller;
use crate::table::{TableLookup, TableRoll};
use crate::template::{
    is_valid_item_id, is_valid_slot_key, Segment, Template, TemplateContext, TemplateError,
};

/// Largest `count` a slot may produce (D-14).
pub const MAX_COUNT: i64 = 50;
/// Re-rolls allowed per value when `distinct` (D-12).
const MAX_DISTINCT_ATTEMPTS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generator {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub template: Template,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub key: String,
    pub source: SlotSource,
    pub count: Count,
    pub distinct: bool,
    pub join: String,
    pub list: bool,
    /// Percent, `1..=100`.
    pub chance: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotSource {
    Table(String),
    Generator(String),
    Dice(DiceExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Count {
    Fixed(u32),
    Dice(DiceExpr),
}

/// Validation errors that exclude a generator.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GeneratorError {
    #[error("a generator needs a `name`")]
    MissingName,
    #[error("a generator needs a `template`")]
    MissingTemplate,
    #[error(
        "invalid generator id {0:?} (use a-z, 0-9, '_' and '-', starting with a letter or digit)"
    )]
    BadId(String),
    #[error("invalid slot key {0:?} (use a-z, 0-9 and '_', starting with a letter or '_')")]
    BadSlotKey(String),
    #[error("duplicate slot key {0:?}")]
    DuplicateSlotKey(String),
    #[error("slot {slot:?} needs exactly one of `table`, `generator` or `dice`")]
    BadSource { slot: String },
    #[error("slot {slot:?} has chance {got}, which is out of range (1 to 100)")]
    BadChance { slot: String, got: i64 },
    #[error("slot {slot:?} has a count that can exceed {max} or go below 0")]
    BadCount { slot: String, max: i64 },
    #[error("slot {slot:?} has invalid dice: {source}")]
    BadDice {
        slot: String,
        #[source]
        source: DiceError,
    },
    #[error("the template uses {{{0}}} but no slot has that key")]
    UnknownTemplateKey(String),
    #[error("invalid template: {0}")]
    BadTemplate(#[source] TemplateError),
    #[error("invalid generator file: {0}")]
    Toml(String),
}

/// Problems that keep the generator but are worth telling the author about.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GeneratorWarning {
    #[error("slot {0:?} is never used in the template")]
    UnusedSlot(String),
    #[error("slot {slot:?}: `{field}` is ignored ({why})")]
    IgnoredField {
        slot: String,
        field: &'static str,
        why: &'static str,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenerator {
    id: Option<String>,
    name: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
    template: Option<String>,
    slots: Option<Vec<RawSlot>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawCount {
    Int(i64),
    Expr(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    key: String,
    table: Option<String>,
    generator: Option<String>,
    dice: Option<String>,
    count: Option<RawCount>,
    distinct: Option<bool>,
    join: Option<String>,
    list: Option<bool>,
    chance: Option<i64>,
}

impl Generator {
    /// Parse and validate. `default_id` is the file stem.
    pub fn from_toml(
        src: &str,
        default_id: &str,
    ) -> Result<(Generator, Vec<GeneratorWarning>), GeneratorError> {
        let raw: RawGenerator = toml::from_str(src)
            .map_err(|e| GeneratorError::Toml(crate::table::toml_message(src, &e)))?;
        let id = raw.id.unwrap_or_else(|| default_id.to_string());
        if !is_valid_item_id(&id) {
            return Err(GeneratorError::BadId(id));
        }
        let name = match raw.name {
            Some(n) if !n.trim().is_empty() => n,
            _ => return Err(GeneratorError::MissingName),
        };
        let template_src = raw.template.ok_or(GeneratorError::MissingTemplate)?;
        let template = Template::parse(&template_src, TemplateContext::GeneratorTemplate)
            .map_err(GeneratorError::BadTemplate)?;

        let mut warnings = Vec::new();
        let mut slots: Vec<Slot> = Vec::new();
        for raw_slot in raw.slots.unwrap_or_default() {
            let slot = build_slot(raw_slot, &mut warnings)?;
            if slots.iter().any(|s| s.key == slot.key) {
                return Err(GeneratorError::DuplicateSlotKey(slot.key));
            }
            slots.push(slot);
        }

        let used: BTreeSet<&str> = template.slot_keys().collect();
        if let Some(missing) = used.iter().find(|k| !slots.iter().any(|s| s.key == **k)) {
            return Err(GeneratorError::UnknownTemplateKey((*missing).to_string()));
        }
        for s in &slots {
            if !used.contains(s.key.as_str()) {
                warnings.push(GeneratorWarning::UnusedSlot(s.key.clone()));
            }
        }

        Ok((
            Generator {
                id,
                name,
                category: raw
                    .category
                    .filter(|c| !c.trim().is_empty())
                    .unwrap_or_else(|| "Uncategorized".to_string()),
                tags: raw.tags.unwrap_or_default(),
                description: raw.description,
                template,
                slots,
            },
            warnings,
        ))
    }

    /// Table and generator ids this generator depends on.
    pub fn table_refs(&self) -> impl Iterator<Item = &str> {
        self.slots.iter().filter_map(|s| match &s.source {
            SlotSource::Table(id) => Some(id.as_str()),
            _ => None,
        })
    }

    pub fn generator_refs(&self) -> impl Iterator<Item = &str> {
        self.slots.iter().filter_map(|s| match &s.source {
            SlotSource::Generator(id) => Some(id.as_str()),
            _ => None,
        })
    }
}

fn build_slot(raw: RawSlot, warnings: &mut Vec<GeneratorWarning>) -> Result<Slot, GeneratorError> {
    let key = raw.key;
    if !is_valid_slot_key(&key) {
        return Err(GeneratorError::BadSlotKey(key));
    }
    let parse_dice = |src: &str| {
        DiceExpr::parse(src).map_err(|source| GeneratorError::BadDice {
            slot: key.clone(),
            source,
        })
    };
    let source = match (raw.table, raw.generator, raw.dice) {
        (Some(t), None, None) => SlotSource::Table(t),
        (None, Some(g), None) => SlotSource::Generator(g),
        (None, None, Some(d)) => SlotSource::Dice(parse_dice(&d)?),
        _ => return Err(GeneratorError::BadSource { slot: key }),
    };

    let chance = match raw.chance {
        None => 100,
        Some(c) if (1..=100).contains(&c) => c as u8,
        Some(got) => return Err(GeneratorError::BadChance { slot: key, got }),
    };

    let bad_count = || GeneratorError::BadCount {
        slot: key.clone(),
        max: MAX_COUNT,
    };
    let count = match raw.count {
        None => Count::Fixed(1),
        Some(RawCount::Int(n)) if (0..=MAX_COUNT).contains(&n) => Count::Fixed(n as u32),
        Some(RawCount::Int(_)) => return Err(bad_count()),
        Some(RawCount::Expr(src)) => {
            let expr = parse_dice(&src)?;
            if expr.bounds().1 > MAX_COUNT {
                return Err(bad_count());
            }
            Count::Dice(expr)
        }
    };

    let distinct_given = raw.distinct.is_some();
    let list = raw.list.unwrap_or(false);
    if distinct_given && !matches!(source, SlotSource::Table(_)) {
        warnings.push(GeneratorWarning::IgnoredField {
            slot: key.clone(),
            field: "distinct",
            why: "only table slots can be distinct",
        });
    }
    if list && raw.join.is_some() {
        warnings.push(GeneratorWarning::IgnoredField {
            slot: key.clone(),
            field: "join",
            why: "`list` takes precedence",
        });
    }
    Ok(Slot {
        key,
        source,
        count,
        distinct: raw.distinct.unwrap_or(true),
        join: raw.join.unwrap_or_else(|| ", ".to_string()),
        list,
        chance,
    })
}

/// Everything a generator can reference.
pub trait Content: TableLookup {
    fn generator(&self, id: &str) -> Option<&Generator>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GeneratorResult {
    pub id: String,
    /// Concatenation of the segments' text.
    pub text: String,
    pub segments: Vec<RenderedSegment>,
    /// Declaration order.
    pub slots: Vec<SlotResult>,
    /// `{roll:..}` in the template, kept when a slot is re-rolled (D-15).
    pub template_rolls: Vec<DiceRoll>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RenderedSegment {
    pub text: String,
    /// Index into `GeneratorResult::slots` when the text came from a slot.
    pub slot: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum SlotSourceRef {
    Table(String),
    Generator(String),
    Dice(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SlotResult {
    pub key: String,
    pub source: SlotSourceRef,
    /// False when `chance` failed.
    pub present: bool,
    pub count_roll: Option<DiceRoll>,
    pub values: Vec<String>,
    /// Parallel to `values`.
    pub detail: Vec<ValueDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ValueDetail {
    Table(TableRoll),
    Generator(Box<GeneratorResult>),
    Dice(DiceRoll),
}

impl Generator {
    pub fn generate(
        &self,
        content: &dyn Content,
        roller: &mut dyn Roller,
    ) -> Result<GeneratorResult, EvalError> {
        self.generate_at(content, roller, 0)
    }

    /// Re-roll exactly one slot, forcing it present (D-13). Other slots and
    /// template rolls are kept as they were.
    pub fn reroll_slot(
        &self,
        prev: &GeneratorResult,
        slot: usize,
        content: &dyn Content,
        roller: &mut dyn Roller,
    ) -> Result<GeneratorResult, EvalError> {
        let def = self.slots.get(slot).ok_or(EvalError::UnknownSlot(slot))?;
        if prev.slots.len() != self.slots.len() {
            return Err(EvalError::UnknownSlot(slot));
        }
        let mut slots = prev.slots.clone();
        slots[slot] = roll_slot(def, content, roller, 0, true)?;
        Ok(self.render(slots, prev.template_rolls.clone()))
    }

    fn generate_at(
        &self,
        content: &dyn Content,
        roller: &mut dyn Roller,
        depth: usize,
    ) -> Result<GeneratorResult, EvalError> {
        let mut slots = Vec::with_capacity(self.slots.len());
        for slot in &self.slots {
            slots.push(roll_slot(slot, content, roller, depth, false)?);
        }
        let template_rolls = self
            .template
            .segments()
            .iter()
            .filter_map(|s| match s {
                Segment::Roll(expr) => Some(expr.roll(roller)),
                _ => None,
            })
            .collect();
        Ok(self.render(slots, template_rolls))
    }

    fn render(&self, slots: Vec<SlotResult>, template_rolls: Vec<DiceRoll>) -> GeneratorResult {
        let mut segments = Vec::new();
        let mut rolls = template_rolls.iter();
        for seg in self.template.segments() {
            match seg {
                Segment::Literal(text) => segments.push(RenderedSegment {
                    text: text.clone(),
                    slot: None,
                }),
                Segment::Roll(_) => {
                    if let Some(r) = rolls.next() {
                        segments.push(RenderedSegment {
                            text: r.total.to_string(),
                            slot: None,
                        });
                    }
                }
                Segment::Slot(key) => {
                    if let Some((i, def)) =
                        self.slots.iter().enumerate().find(|(_, s)| &s.key == key)
                    {
                        segments.push(RenderedSegment {
                            text: join_values(def, &slots[i].values),
                            slot: Some(i),
                        });
                    }
                }
                // Not producible in a generator template (parse rejects it).
                Segment::TableRef(_) => {}
            }
        }
        let text = segments.iter().map(|s| s.text.as_str()).collect();
        GeneratorResult {
            id: self.id.clone(),
            text,
            segments,
            slots,
            template_rolls,
        }
    }
}

fn join_values(slot: &Slot, values: &[String]) -> String {
    if slot.list {
        values
            .iter()
            .map(|v| format!("- {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        values.join(&slot.join)
    }
}

fn roll_slot(
    slot: &Slot,
    content: &dyn Content,
    roller: &mut dyn Roller,
    depth: usize,
    force_present: bool,
) -> Result<SlotResult, EvalError> {
    let source = match &slot.source {
        SlotSource::Table(id) => SlotSourceRef::Table(id.clone()),
        SlotSource::Generator(id) => SlotSourceRef::Generator(id.clone()),
        SlotSource::Dice(expr) => SlotSourceRef::Dice(expr.canonical()),
    };
    let mut result = SlotResult {
        key: slot.key.clone(),
        source,
        present: true,
        count_roll: None,
        values: Vec::new(),
        detail: Vec::new(),
    };

    if slot.chance < 100 && !force_present && roller.roll(100) > u32::from(slot.chance) {
        result.present = false;
        return Ok(result);
    }

    let count = match &slot.count {
        Count::Fixed(n) => *n as usize,
        Count::Dice(expr) => {
            let roll = expr.roll(roller);
            let n = roll.total.clamp(0, MAX_COUNT) as usize;
            result.count_roll = Some(roll);
            n
        }
    };

    let mut seen_entries = BTreeSet::new();
    for _ in 0..count {
        match &slot.source {
            SlotSource::Table(id) => {
                let table = content
                    .table(id)
                    .ok_or_else(|| EvalError::UnknownTable(id.clone()))?;
                let attempts = if slot.distinct && count > 1 {
                    MAX_DISTINCT_ATTEMPTS
                } else {
                    1
                };
                for attempt in 1..=attempts {
                    let roll = table.roll(content, roller)?;
                    if attempts > 1 && !seen_entries.insert(roll.entry_index) {
                        if attempt == attempts {
                            break; // accept fewer
                        }
                        continue;
                    }
                    result.values.push(roll.text.clone());
                    result.detail.push(ValueDetail::Table(roll));
                    break;
                }
            }
            SlotSource::Generator(id) => {
                if depth + 1 > MAX_DEPTH {
                    return Err(EvalError::DepthExceeded { max: MAX_DEPTH });
                }
                let nested = content
                    .generator(id)
                    .ok_or_else(|| EvalError::UnknownGenerator(id.clone()))?
                    .generate_at(content, roller, depth + 1)?;
                result.values.push(nested.text.clone());
                result.detail.push(ValueDetail::Generator(Box::new(nested)));
            }
            SlotSource::Dice(expr) => {
                let roll = expr.roll(roller);
                result.values.push(roll.total.to_string());
                result.detail.push(ValueDetail::Dice(roll));
            }
        }
    }
    Ok(result)
}
