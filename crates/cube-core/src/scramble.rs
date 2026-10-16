//! Random states, from a seed the caller supplies.

use crate::cube::Cube;
use crate::geom::{Axis, Face};
use crate::moves::{Move, Turn, face_move_name};

/// splitmix64: small, fast and good enough to shuffle a cube.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Self(seed ^ 0x9e37_79b9_7f4a_7c15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n.max(1) as u64) as u32
    }
}

/// A random maneuver: never two turns about the same axis in a row, and for
/// cubes bigger than 3 the inner layers get turned too.
pub fn scramble(n: u8, len: usize, rng: &mut Rng) -> Vec<Move> {
    let mut out = Vec::with_capacity(len);
    let mut last: Option<Axis> = None;
    while out.len() < len {
        let face = Face::ALL[rng.below(6) as usize];
        let (axis, _) = face.axis();
        if Some(axis) == last {
            continue;
        }
        last = Some(axis);
        let depth = rng.below(u32::from(n) / 2) as u8;
        let quarters = [1i8, 2, -1][rng.below(3) as usize];
        out.push(Move::new(
            Turn::from_face(n, face, depth, depth, quarters),
            face_move_name(face, depth, quarters),
        ));
    }
    out
}

/// A scrambled cube, together with the maneuver that produced it.
pub fn scrambled(n: u8, len: usize, rng: &mut Rng) -> (Cube, Vec<Move>) {
    let moves = scramble(n, len, rng);
    let mut cube = Cube::solved(n);
    cube.apply_moves(&moves);
    (cube, moves)
}
