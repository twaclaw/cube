//! Move notation.
//!
//! One liberal parser covers the three notations this project cares about:
//!
//! * Singmaster / WCA: `R`, `R'`, `R2`, `Rw`, `M`, `E`, `S`, `x`, `y`, `z`
//! * SageMath products, as used in the notebooks: `R^2*U*F^-1`
//! * the crammed macro language of geometer.org's *Rubik*: upper case is a
//!   clockwise quarter turn, lower case the counter-clockwise one (`rfu`),
//!   `*F` is a slice move and `>F` a whole-cube move
//!
//! Groups repeat and invert: `(R U)3`, `(R U R' U')2`, `(R U)'`. Deep layers of
//! a big cube are `2R` (second layer), `2-3R` (second and third) and `3Rw`
//! (outermost three).

use crate::geom::Face;
use crate::moves::{Move, Turn};
use core::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub pos: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at character {})", self.message, self.pos + 1)
    }
}

impl core::error::Error for ParseError {}

/// Parse a sequence of moves for an `n`x`n`x`n` cube.
pub fn parse(n: u8, src: &str) -> Result<Vec<Move>, ParseError> {
    let mut p = Parser {
        chars: src.chars().collect(),
        pos: 0,
        n,
    };
    let moves = p.sequence(false)?;
    Ok(moves)
}

/// Write a sequence back out, space separated.
pub fn format(moves: &[Move]) -> String {
    moves
        .iter()
        .map(|m| m.name.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Which layers a written move turns. `Layers` depths are 1-based and counted
/// from the named face, so `Layers(1, 1)` is a plain face turn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Span {
    Layers(u8, u8),
    Slice,
    Whole,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    n: u8,
}

impl Parser {
    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn peek(&self) -> Option<char> {
        self.at(self.pos)
    }

    fn err<T>(&self, pos: usize, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            pos,
            message: message.into(),
        })
    }

    /// `*` is a slice prefix only where a move may start, so that the Sage
    /// product `R*U` still reads as two moves.
    fn star_is_prefix(&self) -> bool {
        if self.peek() != Some('*') {
            return false;
        }
        let before = (self.pos > 0).then(|| self.at(self.pos - 1)).flatten();
        let opens = matches!(before, None | Some('(') | Some(',') | Some('*'))
            || before.is_some_and(char::is_whitespace);
        opens
            && self
                .at(self.pos + 1)
                .is_some_and(|c| Face::from_letter(c).is_some())
    }

    fn skip_separators(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ',' || c == '.' || (c == '*' && !self.star_is_prefix()) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn sequence(&mut self, nested: bool) -> Result<Vec<Move>, ParseError> {
        let mut out = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                None if nested => return self.err(self.pos, "missing `)`"),
                None => return Ok(out),
                Some(')') if nested => return Ok(out),
                Some(')') => return self.err(self.pos, "unmatched `)`"),
                Some('(') => {
                    self.pos += 1;
                    let group = self.sequence(true)?;
                    self.pos += 1; // the `)`
                    out.extend(self.repeat_group(group)?);
                }
                Some(_) => out.push(self.single_move()?),
            }
        }
    }

    /// `(..)3`, `(..)^3` and `(..)'` applied to an already parsed group.
    fn repeat_group(&mut self, group: Vec<Move>) -> Result<Vec<Move>, ParseError> {
        let (count, invert) = self.multiplier();
        let mut out = Vec::with_capacity(group.len() * count as usize);
        for _ in 0..count {
            if invert {
                out.extend(group.iter().rev().map(Move::inverse));
            } else {
                out.extend(group.iter().cloned());
            }
        }
        Ok(out)
    }

    fn multiplier(&mut self) -> (u32, bool) {
        let mut invert = false;
        if self.peek() == Some('^')
            || (matches!(self.peek(), Some('x') | Some('X'))
                && self.at(self.pos + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            self.pos += 1;
        }
        if matches!(self.peek(), Some('\'') | Some('-')) {
            invert = true;
            self.pos += 1;
        }
        let count = self.digits().unwrap_or(1).max(1);
        if self.peek() == Some('\'') {
            invert = !invert;
            self.pos += 1;
        }
        (count, invert)
    }

    fn digits(&mut self) -> Option<u32> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        (self.pos > start).then(|| {
            self.chars[start..self.pos]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(1)
        })
    }

    fn single_move(&mut self) -> Result<Move, ParseError> {
        let start = self.pos;
        let mut span = None;
        let mut depth = None;

        if self.star_is_prefix() {
            span = Some(Span::Slice);
            self.pos += 1;
        } else if self.peek() == Some('>') {
            span = Some(Span::Whole);
            self.pos += 1;
        } else if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let first = self.digits().unwrap_or(1).clamp(1, 255) as u8;
            if self.peek() == Some('-') {
                self.pos += 1;
                let Some(last) = self.digits() else {
                    return self.err(self.pos, "expected a layer number after `-`");
                };
                span = Some(Span::Layers(first, last.clamp(1, 255) as u8));
            } else {
                depth = Some(first);
            }
        }

        let Some(letter) = self.peek() else {
            return self.err(start, "expected a move");
        };
        self.pos += 1;

        // M/E/S are slices and x/y/z whole-cube turns, whatever the prefix said.
        let (face, mut sign, implied) = match letter {
            'M' | 'm' => (
                Face::L,
                if letter == 'M' { 1 } else { -1 },
                Some(Span::Slice),
            ),
            'E' | 'e' => (
                Face::D,
                if letter == 'E' { 1 } else { -1 },
                Some(Span::Slice),
            ),
            'S' | 's' => (
                Face::F,
                if letter == 'S' { 1 } else { -1 },
                Some(Span::Slice),
            ),
            'x' | 'X' => (Face::R, 1, Some(Span::Whole)),
            'y' | 'Y' => (Face::U, 1, Some(Span::Whole)),
            'z' | 'Z' => (Face::F, 1, Some(Span::Whole)),
            c => match Face::from_letter(c) {
                // lower case is the counter-clockwise turn, as in Rubik's macros
                Some(f) => (f, if c.is_ascii_lowercase() { -1 } else { 1 }, None),
                None => return self.err(start, format!("`{c}` is not a move")),
            },
        };

        let wide = matches!(self.peek(), Some('w') | Some('W'));
        if wide {
            self.pos += 1;
        }
        let span = match (implied, span) {
            (Some(forced), _) => forced,
            (None, Some(explicit)) => explicit,
            (None, None) if wide => Span::Layers(1, depth.unwrap_or(2)),
            (None, None) => {
                let d = depth.unwrap_or(1);
                Span::Layers(d, d)
            }
        };

        sign *= self.modifier();
        let name: String = self.chars[start..self.pos].iter().collect();

        let n = self.n;
        let turn = match span {
            Span::Layers(a, b) => {
                if a > b || b > n {
                    return self.err(start, format!("`{name}` is not a layer of a {n}-cube"));
                }
                Turn::from_face(n, face, a - 1, b - 1, sign)
            }
            Span::Slice => {
                if n < 3 {
                    return self.err(start, "a cube this small has no middle layer");
                }
                Turn::slice(n, face, sign)
            }
            Span::Whole => Turn::whole(n, face, sign),
        };
        Ok(Move::new(turn, name))
    }

    /// `'`, `2`, `3`, `i`, `^2`, `^-1`, `-1`, `2'` ... as a signed turn count.
    fn modifier(&mut self) -> i8 {
        let mut amount: i32 = 1;
        let mut negate = false;
        if self.peek() == Some('^') {
            self.pos += 1;
        }
        loop {
            match self.peek() {
                Some('\'') | Some('i') | Some('I') | Some('`') | Some('-') => {
                    negate = !negate;
                    self.pos += 1;
                }
                Some('2') if amount == 1 => {
                    amount = 2;
                    self.pos += 1;
                }
                Some('3') if amount == 1 => {
                    amount = 3;
                    self.pos += 1;
                }
                Some('1') if amount == 1 => {
                    self.pos += 1;
                }
                Some('\u{00b2}') => {
                    amount = 2;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let amount = if negate { -amount } else { amount };
        amount.clamp(-3, 3) as i8
    }
}
