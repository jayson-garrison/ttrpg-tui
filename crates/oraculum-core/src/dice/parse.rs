//! Tokenizer and parser. Byte positions; ASCII case-insensitive.

use super::{
    DiceError, DiceSpec, Keep, KeepKind, Sign, SignedTerm, Term, MAX_CONSTANT, MAX_COUNT,
    MAX_SIDES, MAX_TERMS, MAX_TOTAL_DICE, MIN_SIDES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Num(u64),
    D,
    Kh,
    Kl,
    Plus,
    Minus,
}

type Spanned = (usize, Tok);

fn lex(src: &str) -> Result<Vec<Spanned>, DiceError> {
    let bytes = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'+' => {
                toks.push((i, Tok::Plus));
                i += 1;
            }
            b'-' => {
                toks.push((i, Tok::Minus));
                i += 1;
            }
            b'0'..=b'9' => {
                let start = i;
                let mut value: u64 = 0;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(u64::from(bytes[i] - b'0')))
                        .ok_or(DiceError::NumberTooLarge { pos: start })?;
                    i += 1;
                }
                toks.push((start, Tok::Num(value)));
            }
            b'd' | b'D' => {
                toks.push((i, Tok::D));
                i += 1;
            }
            b'k' | b'K' => match bytes.get(i + 1).map(u8::to_ascii_lowercase) {
                Some(b'h') => {
                    toks.push((i, Tok::Kh));
                    i += 2;
                }
                Some(b'l') => {
                    toks.push((i, Tok::Kl));
                    i += 2;
                }
                Some(_) => return Err(unexpected_char(src, i + 1)),
                None => {
                    return Err(DiceError::UnexpectedToken {
                        pos: src.len(),
                        expected: "'h' or 'l' after 'k'",
                    })
                }
            },
            _ => return Err(unexpected_char(src, i)),
        }
    }
    Ok(toks)
}

fn unexpected_char(src: &str, pos: usize) -> DiceError {
    // `pos` is always a char boundary: only ASCII bytes are consumed before it.
    let ch = src
        .get(pos..)
        .and_then(|s| s.chars().next())
        .unwrap_or('\u{fffd}');
    DiceError::UnexpectedChar { pos, ch }
}

struct Parser {
    toks: Vec<Spanned>,
    idx: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<Spanned> {
        self.toks.get(self.idx).copied()
    }

    fn bump(&mut self) -> Option<Spanned> {
        let t = self.peek();
        if t.is_some() {
            self.idx += 1;
        }
        t
    }

    fn term(&mut self) -> Result<(Term, usize), DiceError> {
        const WANT: &str = "a number or dice";
        let (start, tok) = self.bump().ok_or(DiceError::UnexpectedToken {
            pos: self.end,
            expected: WANT,
        })?;
        match tok {
            Tok::Num(n) => {
                if matches!(self.peek(), Some((_, Tok::D))) {
                    self.bump();
                    self.dice(Some((start, n)))
                } else if n > u64::from(MAX_CONSTANT) {
                    Err(DiceError::NumberTooLarge { pos: start })
                } else {
                    Ok((Term::Const(n as u32), 0))
                }
            }
            Tok::D => self.dice(None),
            _ => Err(DiceError::UnexpectedToken {
                pos: start,
                expected: WANT,
            }),
        }
    }

    /// After `[N] d`: parse sides and optional keep suffix.
    fn dice(&mut self, count: Option<(usize, u64)>) -> Result<(Term, usize), DiceError> {
        let (sides_pos, sides) = match self.bump() {
            Some((p, Tok::Num(n))) => (p, n),
            Some((p, _)) => {
                return Err(DiceError::UnexpectedToken {
                    pos: p,
                    expected: "number of sides",
                })
            }
            None => {
                return Err(DiceError::UnexpectedToken {
                    pos: self.end,
                    expected: "number of sides",
                })
            }
        };
        let keep_tok = match self.peek() {
            Some((p, Tok::Kh)) => Some((p, KeepKind::Highest)),
            Some((p, Tok::Kl)) => Some((p, KeepKind::Lowest)),
            _ => None,
        };
        let mut keep_n: Option<(usize, u64)> = None;
        if keep_tok.is_some() {
            self.bump();
            if let Some((p, Tok::Num(k))) = self.peek() {
                self.bump();
                keep_n = Some((p, k));
            }
        }

        // D-07: bare d66 only (no count, no keep suffix).
        if count.is_none() && keep_tok.is_none() && sides == 66 {
            return Ok((Term::D66, 2));
        }

        let (count_pos, count_val) = count.unwrap_or((sides_pos, 1));
        if !(1..=u64::from(MAX_COUNT)).contains(&count_val) {
            return Err(DiceError::CountOutOfRange {
                pos: count_pos,
                got: count_val,
            });
        }
        if !(u64::from(MIN_SIDES)..=u64::from(MAX_SIDES)).contains(&sides) {
            return Err(DiceError::SidesOutOfRange {
                pos: sides_pos,
                got: sides,
            });
        }
        let n = count_val as u8;
        let keep = match keep_tok {
            None => None,
            Some((kpos, which)) => {
                let (pos, k) = keep_n.unwrap_or((kpos, 1));
                if !(1..=u64::from(n)).contains(&k) {
                    return Err(DiceError::KeepOutOfRange { pos, k, n });
                }
                Some(Keep { which, n: k as u8 })
            }
        };
        Ok((
            Term::Dice(DiceSpec {
                count: n,
                sides: sides as u16,
                keep,
            }),
            usize::from(n),
        ))
    }
}

pub(super) fn parse(src: &str) -> Result<Vec<SignedTerm>, DiceError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(DiceError::Empty);
    }
    let mut p = Parser {
        toks,
        idx: 0,
        end: src.len(),
    };
    let mut terms = Vec::new();
    let mut total_dice = 0usize;
    let mut sign = Sign::Plus;
    loop {
        let (term, dice) = p.term()?;
        total_dice += dice;
        terms.push(SignedTerm { sign, term });
        if terms.len() > MAX_TERMS {
            return Err(DiceError::TooManyTerms { max: MAX_TERMS });
        }
        if total_dice > MAX_TOTAL_DICE {
            return Err(DiceError::TooManyDice {
                max: MAX_TOTAL_DICE,
            });
        }
        match p.bump() {
            None => break,
            Some((_, Tok::Plus)) => sign = Sign::Plus,
            Some((_, Tok::Minus)) => sign = Sign::Minus,
            Some((pos, _)) => {
                return Err(DiceError::UnexpectedToken {
                    pos,
                    expected: "'+', '-' or end of expression",
                })
            }
        }
    }
    Ok(terms)
}
