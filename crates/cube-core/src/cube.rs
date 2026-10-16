//! The cube state: one color per facelet.

use crate::geom::{Face, facelet_at, facelet_index, facelet_position, layer_index};
use crate::moves::{Move, Turn};

/// An `n`x`n`x`n` cube, stored as `6 * n * n` facelet colors. A color is just
/// the face it starts on, so a solved cube has every facelet showing its own
/// face.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cube {
    n: u8,
    stickers: Vec<Face>,
}

impl Cube {
    pub fn solved(n: u8) -> Self {
        assert!(n >= 1, "cube size must be positive");
        let mut stickers = Vec::with_capacity(6 * n as usize * n as usize);
        for f in Face::ALL {
            stickers.extend(core::iter::repeat_n(f, n as usize * n as usize));
        }
        Self { n, stickers }
    }

    pub const fn n(&self) -> u8 {
        self.n
    }

    pub fn stickers(&self) -> &[Face] {
        &self.stickers
    }

    pub fn get(&self, f: Face, row: u8, col: u8) -> Face {
        self.stickers[facelet_index(self.n, f, row, col)]
    }

    pub fn set(&mut self, f: Face, row: u8, col: u8, color: Face) {
        let i = facelet_index(self.n, f, row, col);
        self.stickers[i] = color;
    }

    pub fn is_solved(&self) -> bool {
        let per_face = self.n as usize * self.n as usize;
        self.stickers
            .chunks(per_face)
            .enumerate()
            .all(|(i, face)| face.iter().all(|c| *c as usize == i))
    }

    /// Every face a single color: solved, possibly re-oriented by a whole-cube
    /// turn.
    pub fn is_uniform(&self) -> bool {
        let per_face = self.n as usize * self.n as usize;
        self.stickers
            .chunks(per_face)
            .all(|face| face.iter().all(|c| *c == face[0]))
    }

    /// Turn the layers named by `t`, carrying the facelets round with them.
    pub fn apply(&mut self, t: &Turn) {
        if t.is_identity() {
            return;
        }
        let n = self.n;
        let mut next = self.stickers.clone();
        for f in Face::ALL {
            for row in 0..n {
                for col in 0..n {
                    let p = facelet_position(n, f, row, col);
                    if !t.covers(layer_index(n, t.axis, p)) {
                        continue;
                    }
                    let moved = t.axis.rotate(p, t.quarters as i32);
                    let normal = t.axis.rotate(f.normal(), t.quarters as i32);
                    let (f2, r2, c2) =
                        facelet_at(n, moved, normal).expect("a turn keeps facelets on the surface");
                    next[facelet_index(n, f2, r2, c2)] =
                        self.stickers[facelet_index(n, f, row, col)];
                }
            }
        }
        self.stickers = next;
    }

    pub fn apply_moves<'a>(&mut self, moves: impl IntoIterator<Item = &'a Move>) {
        for m in moves {
            self.apply(&m.turn);
        }
    }

    /// Facelet colors as letters in numbering order (U, L, F, R, B, D,
    /// row-major), centers included.
    pub fn to_letters(&self) -> String {
        self.stickers
            .iter()
            .map(|f| f.letter().to_ascii_lowercase())
            .collect()
    }

    /// Inverse of [`Cube::to_letters`]; only checks shape, not solvability.
    pub fn from_letters(n: u8, s: &str) -> Option<Self> {
        let want = 6 * n as usize * n as usize;
        let stickers: Option<Vec<Face>> = s.chars().map(Face::from_letter).collect();
        let stickers = stickers?;
        (stickers.len() == want).then_some(Self { n, stickers })
    }
}
