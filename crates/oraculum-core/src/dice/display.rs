//! Canonical form and the fixed roll display format.

use std::fmt::{self, Write};

use super::{DiceExpr, DiceRoll, KeepKind, Sign, Term, TermRoll, TermRollKind};

fn sign_char(sign: Sign) -> char {
    match sign {
        Sign::Plus => '+',
        Sign::Minus => '-',
    }
}

pub(super) fn canonical(expr: &DiceExpr) -> String {
    let mut out = String::new();
    for (i, st) in expr.terms.iter().enumerate() {
        if i > 0 {
            out.push(sign_char(st.sign));
        }
        // Writing to a String cannot fail.
        let _ = match st.term {
            Term::Const(c) => write!(out, "{c}"),
            Term::D66 => write!(out, "d66"),
            Term::Dice(spec) => {
                let _ = write!(out, "{}d{}", spec.count, spec.sides);
                match spec.keep {
                    None => Ok(()),
                    Some(k) => {
                        let tag = match k.which {
                            KeepKind::Highest => "kh",
                            KeepKind::Lowest => "kl",
                        };
                        write!(out, "{tag}{}", k.n)
                    }
                }
            }
        };
    }
    out
}

fn list(values: impl Iterator<Item = u32>) -> String {
    let items: Vec<String> = values.map(|v| v.to_string()).collect();
    format!("[{}]", items.join(", "))
}

fn term_text(t: &TermRoll) -> String {
    match &t.kind {
        TermRollKind::Const(c) => c.to_string(),
        TermRollKind::D66 {
            tens,
            units,
            subtotal,
        } => format!("[{tens}, {units}]={subtotal}"),
        TermRollKind::Dice {
            spec, rolls, kept, ..
        } => {
            let all = list(rolls.iter().copied());
            if spec.keep.is_some() {
                let kept_values = list(rolls.iter().zip(kept).filter(|(_, &k)| k).map(|(&v, _)| v));
                format!("{all} keep {kept_values}")
            } else {
                all
            }
        }
    }
}

impl fmt::Display for DiceRoll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self
            .terms
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let text = term_text(t);
                if i == 0 {
                    text
                } else {
                    format!("{}{}", sign_char(t.sign), text)
                }
            })
            .collect();
        write!(f, "{} => {} = {}", self.expr, parts.join(" "), self.total)
    }
}
