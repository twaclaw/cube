//! Move and pruning tables.
//!
//! Each coordinate gets a table saying where every value goes under every
//! move, and each phase gets two pruning tables holding the exact distance to
//! the goal for a pair of coordinates. They are built once, by breadth first
//! search, and reused for every solve.

use super::cubie::{
    CubieCube, N_CORNERS, N_FLIP, N_PERM4, N_SLICE, N_TWIST, N_UD_EDGES, basic_moves,
};
use crate::geom::Face;
use crate::moves::face_move_name;

/// Faces in the solver's order, so that opposite faces are three apart.
pub const FACES: [Face; 6] = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];
/// Every face turn: face `m / 3`, turned `m % 3 + 1` quarters.
pub const N_MOVES: usize = 18;
/// Phase 2 keeps the quarter turns of U and D and the half turns of the rest.
pub const PHASE2_MOVES: [usize; 10] = [0, 1, 2, 4, 7, 9, 10, 11, 13, 16];
pub const N_PHASE2_MOVES: usize = PHASE2_MOVES.len();

/// How a move is written.
pub fn move_name(m: usize) -> String {
    face_move_name(FACES[m / 3], 0, (m % 3) as i8 + 1)
}

/// The face a move turns. Moves of the same face, or of the opposite face in
/// the wrong order, are never tried twice in a row.
pub const fn move_face(m: usize) -> usize {
    m / 3
}

pub struct Tables {
    /// The 18 face turns at the cubie level.
    pub moves: [CubieCube; N_MOVES],
    twist_move: Vec<u16>,
    flip_move: Vec<u16>,
    slice_move: Vec<u16>,
    corners_move: Vec<u16>,
    ud_edges_move: Vec<u16>,
    slice_perm_move: Vec<u16>,
    twist_slice: Vec<u8>,
    flip_slice: Vec<u8>,
    corners_slice: Vec<u8>,
    ud_edges_slice: Vec<u8>,
}

impl Tables {
    /// Build everything. Around a hundred milliseconds, once per session.
    pub fn new() -> Self {
        let moves = all_moves();

        let twist_move = move_table(
            N_TWIST,
            N_MOVES,
            &moves,
            &PHASE1,
            CubieCube::set_twist,
            |c| c.twist(),
        );
        let flip_move = move_table(N_FLIP, N_MOVES, &moves, &PHASE1, CubieCube::set_flip, |c| {
            c.flip()
        });
        // The slice coordinate ignores the order of the four edges, so any
        // representative of the class will do.
        let slice_move = move_table(
            N_SLICE,
            N_MOVES,
            &moves,
            &PHASE1,
            |c, v| c.set_slice_sorted(v * N_PERM4 as u16),
            |c| c.slice_sorted() / N_PERM4 as u16,
        );
        let corners_move = move_table(
            N_CORNERS,
            N_PHASE2_MOVES,
            &moves,
            &PHASE2_MOVES,
            CubieCube::set_corners,
            |c| c.corners(),
        );
        let ud_edges_move = move_table(
            N_UD_EDGES,
            N_PHASE2_MOVES,
            &moves,
            &PHASE2_MOVES,
            CubieCube::set_ud_edges,
            |c| c.ud_edges(),
        );
        // In phase 2 the slice edges are home, so only their order moves.
        let slice_perm_move = move_table(
            N_PERM4,
            N_PHASE2_MOVES,
            &moves,
            &PHASE2_MOVES,
            CubieCube::set_slice_sorted,
            CubieCube::slice_sorted,
        );

        let twist_slice = pruning_table(N_TWIST, N_SLICE, N_MOVES, &twist_move, &slice_move);
        let flip_slice = pruning_table(N_FLIP, N_SLICE, N_MOVES, &flip_move, &slice_move);
        let corners_slice = pruning_table(
            N_CORNERS,
            N_PERM4,
            N_PHASE2_MOVES,
            &corners_move,
            &slice_perm_move,
        );
        let ud_edges_slice = pruning_table(
            N_UD_EDGES,
            N_PERM4,
            N_PHASE2_MOVES,
            &ud_edges_move,
            &slice_perm_move,
        );

        Self {
            moves,
            twist_move,
            flip_move,
            slice_move,
            corners_move,
            ud_edges_move,
            slice_perm_move,
            twist_slice,
            flip_slice,
            corners_slice,
            ud_edges_slice,
        }
    }

    pub fn bytes(&self) -> usize {
        2 * (self.twist_move.len()
            + self.flip_move.len()
            + self.slice_move.len()
            + self.corners_move.len()
            + self.ud_edges_move.len()
            + self.slice_perm_move.len())
            + self.twist_slice.len()
            + self.flip_slice.len()
            + self.corners_slice.len()
            + self.ud_edges_slice.len()
    }

    // ----- phase 1 -------------------------------------------------------

    pub fn twist(&self, twist: u16, m: usize) -> u16 {
        self.twist_move[twist as usize * N_MOVES + m]
    }

    pub fn flip(&self, flip: u16, m: usize) -> u16 {
        self.flip_move[flip as usize * N_MOVES + m]
    }

    pub fn slice(&self, slice: u16, m: usize) -> u16 {
        self.slice_move[slice as usize * N_MOVES + m]
    }

    /// Moves still needed to reach the phase 1 goal, never an overestimate.
    pub fn phase1_distance(&self, twist: u16, flip: u16, slice: u16) -> u8 {
        let a = self.twist_slice[twist as usize * N_SLICE + slice as usize];
        let b = self.flip_slice[flip as usize * N_SLICE + slice as usize];
        a.max(b)
    }

    // ----- phase 2 -------------------------------------------------------

    pub fn corners(&self, corners: u16, m: usize) -> u16 {
        self.corners_move[corners as usize * N_PHASE2_MOVES + m]
    }

    pub fn ud_edges(&self, edges: u16, m: usize) -> u16 {
        self.ud_edges_move[edges as usize * N_PHASE2_MOVES + m]
    }

    pub fn slice_perm(&self, perm: u16, m: usize) -> u16 {
        self.slice_perm_move[perm as usize * N_PHASE2_MOVES + m]
    }

    pub fn phase2_distance(&self, corners: u16, edges: u16, slice: u16) -> u8 {
        let a = self.corners_slice[corners as usize * N_PERM4 + slice as usize];
        let b = self.ud_edges_slice[edges as usize * N_PERM4 + slice as usize];
        a.max(b)
    }
}

impl Default for Tables {
    fn default() -> Self {
        Self::new()
    }
}

const PHASE1: [usize; N_MOVES] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];

/// The 18 face turns, built from the three quarter turns of each face.
fn all_moves() -> [CubieCube; N_MOVES] {
    let quarter = basic_moves();
    core::array::from_fn(|m| {
        let base = &quarter[FACES[m / 3] as usize];
        let mut cube = CubieCube::solved();
        for _ in 0..=(m % 3) {
            cube.apply(base);
        }
        cube
    })
}

/// Where each value of a coordinate goes under each move.
fn move_table(
    size: usize,
    n_moves: usize,
    moves: &[CubieCube; N_MOVES],
    which: &[usize],
    set: impl Fn(&mut CubieCube, u16),
    get: impl Fn(&CubieCube) -> u16,
) -> Vec<u16> {
    let mut table = vec![0u16; size * n_moves];
    let mut cube = CubieCube::solved();
    for value in 0..size {
        set(&mut cube, value as u16);
        for (m, &index) in which[..n_moves].iter().enumerate() {
            let mut moved = cube;
            moved.apply(&moves[index]);
            table[value * n_moves + m] = get(&moved);
        }
    }
    table
}

/// Exact distance from every pair of coordinates to the goal, by breadth first
/// search outwards from it.
fn pruning_table(
    n_a: usize,
    n_b: usize,
    n_moves: usize,
    move_a: &[u16],
    move_b: &[u16],
) -> Vec<u8> {
    let total = n_a * n_b;
    let mut table = vec![u8::MAX; total];
    table[0] = 0;
    let mut filled = 1;
    let mut depth = 0u8;
    while filled < total {
        let mut grew = false;
        for index in 0..total {
            if table[index] != depth {
                continue;
            }
            let (a, b) = (index / n_b, index % n_b);
            for m in 0..n_moves {
                let next =
                    move_a[a * n_moves + m] as usize * n_b + move_b[b * n_moves + m] as usize;
                if table[next] == u8::MAX {
                    table[next] = depth + 1;
                    filled += 1;
                    grew = true;
                }
            }
        }
        if !grew {
            break; // states the moves cannot reach stay at u8::MAX
        }
        depth += 1;
    }
    table
}
