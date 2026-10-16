//! Moves: a turn of one or more layers, plus the names they are written with.

use crate::geom::{Axis, Face};
use core::f64::consts::FRAC_PI_2;

/// A turn of the layers `lo..=hi` about `axis`, by `quarters` quarter turns
/// clockwise seen from the positive end of the axis.
///
/// This one shape covers face turns, wide turns, slice moves and whole-cube
/// rotations, for any cube size.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Turn {
    pub axis: Axis,
    pub lo: u8,
    pub hi: u8,
    pub quarters: i8,
}

impl Turn {
    /// A turn of the layers `depth_lo..=depth_hi` counted from `face` (depth 0
    /// is the face itself), `quarters` clockwise as seen from outside `face`.
    pub fn from_face(n: u8, face: Face, depth_lo: u8, depth_hi: u8, quarters: i8) -> Self {
        let (axis, positive) = face.axis();
        let (lo, hi, quarters) = if positive {
            (n - 1 - depth_hi, n - 1 - depth_lo, quarters)
        } else {
            (depth_lo, depth_hi, -quarters)
        };
        Self {
            axis,
            lo,
            hi,
            quarters,
        }
    }

    /// Outer layer of `face` only.
    pub fn face(n: u8, face: Face, quarters: i8) -> Self {
        Self::from_face(n, face, 0, 0, quarters)
    }

    /// Every layer strictly between `face` and its opposite.
    pub fn slice(n: u8, face: Face, quarters: i8) -> Self {
        Self::from_face(n, face, 1, n.saturating_sub(2), quarters)
    }

    /// The whole cube, reoriented as if `face` were turned.
    pub fn whole(n: u8, face: Face, quarters: i8) -> Self {
        Self::from_face(n, face, 0, n - 1, quarters)
    }

    pub fn inverse(self) -> Self {
        Self {
            quarters: -self.quarters,
            ..self
        }
    }

    pub const fn covers(&self, layer: u8) -> bool {
        self.lo <= layer && layer <= self.hi
    }

    /// Signed rotation angle in radians, clockwise seen from `+axis`.
    pub fn angle(&self) -> f64 {
        self.quarters as f64 * FRAC_PI_2
    }

    pub const fn is_identity(&self) -> bool {
        self.quarters % 4 == 0 || self.lo > self.hi
    }
}

/// A turn together with the text it was written as, for display and replay.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Move {
    pub turn: Turn,
    pub name: String,
}

impl Move {
    pub fn new(turn: Turn, name: impl Into<String>) -> Self {
        Self {
            turn,
            name: name.into(),
        }
    }

    pub fn inverse(&self) -> Self {
        Self {
            turn: self.turn.inverse(),
            name: invert_name(&self.name),
        }
    }
}

/// Flip the modifier of a written move, so `R` becomes `R'` and `R2` stays.
fn invert_name(name: &str) -> String {
    match name.strip_suffix('\'') {
        Some(base) => base.to_string(),
        None if name.ends_with('2') => name.to_string(),
        None => format!("{name}'"),
    }
}

/// Canonical name of a face turn, in the notation the parser accepts.
pub fn face_move_name(face: Face, depth: u8, quarters: i8) -> String {
    let suffix = match quarters.rem_euclid(4) {
        1 => "",
        2 => "2",
        3 => "'",
        _ => "",
    };
    let prefix = if depth == 0 {
        String::new()
    } else {
        format!("{}", depth + 1)
    };
    format!("{prefix}{}{suffix}", face.letter())
}
