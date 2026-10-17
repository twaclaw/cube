//! The two iterative deepening searches.
//!
//! Phase 1 drives the cube into the subgroup where only U, D and the half
//! turns are needed, watching three coordinates: corner twist, edge flip and
//! where the four slice edges sit. Phase 2 finishes inside that subgroup,
//! watching corner permutation, U/D edge permutation and the order of the
//! slice edges. Each phase is guided by the exact distances in its two pruning
//! tables, so the heuristic never overestimates and the first solution found
//! at a depth is as short as that phase can be.

use super::cubie::CubieCube;
use super::tables::{N_MOVES, PHASE2_MOVES, Tables, move_face};

/// How hard to look.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Give up if nothing this short turns up.
    pub max_length: usize,
    /// Stop as soon as a solution is this short.
    pub good_enough: usize,
    /// Ceiling on the nodes visited, so the browser never locks up.
    pub max_nodes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_length: 24,
            good_enough: 20,
            max_nodes: 6_000_000,
        }
    }
}

/// The longest phase 1 ever needs to be.
const MAX_PHASE1: usize = 12;

pub struct Search<'a> {
    tables: &'a Tables,
    start: CubieCube,
    limits: Limits,
    stack1: Vec<usize>,
    stack2: Vec<usize>,
    best: Option<(Vec<usize>, Vec<usize>)>,
    best_len: usize,
    nodes: u64,
    stop: bool,
}

impl<'a> Search<'a> {
    pub fn new(tables: &'a Tables, start: CubieCube, limits: Limits) -> Self {
        Self {
            tables,
            start,
            limits,
            stack1: Vec::with_capacity(MAX_PHASE1),
            stack2: Vec::with_capacity(32),
            best: None,
            best_len: limits.max_length + 1,
            nodes: 0,
            stop: false,
        }
    }

    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// The two move lists and the nodes it took to find them. Phase 1 moves
    /// index the full move table, phase 2 moves index `PHASE2_MOVES`.
    pub fn run_counting(&mut self) -> Option<(Vec<usize>, Vec<usize>, u64)> {
        let best = self.run();
        best.map(|(one, two)| (one, two, self.nodes))
    }

    /// The phase 1 and phase 2 move lists, as move indices.
    pub fn run(&mut self) -> Option<(Vec<usize>, Vec<usize>)> {
        let (twist, flip, slice) = (
            self.start.twist(),
            self.start.flip(),
            self.start.slice_sorted() / 24,
        );
        for depth in 0..=MAX_PHASE1 {
            if self.stop || depth >= self.best_len {
                break;
            }
            self.phase1(twist, flip, slice, depth, usize::MAX);
        }
        self.best.clone()
    }

    /// Depth first to exactly `depth`, calling phase 2 at every state that has
    /// reached the subgroup.
    fn phase1(&mut self, twist: u16, flip: u16, slice: u16, depth: usize, last_face: usize) {
        if self.stop {
            return;
        }
        if depth == 0 {
            if twist == 0 && flip == 0 && slice == 0 {
                self.phase2_from_here(last_face);
            }
            return;
        }
        if (self.tables.phase1_distance(twist, flip, slice) as usize) > depth {
            return;
        }
        self.nodes += 1;
        if self.nodes > self.limits.max_nodes {
            self.stop = true;
            return;
        }
        for m in 0..N_MOVES {
            if !allowed(move_face(m), last_face) {
                continue;
            }
            self.stack1.push(m);
            self.phase1(
                self.tables.twist(twist, m),
                self.tables.flip(flip, m),
                self.tables.slice(slice, m),
                depth - 1,
                move_face(m),
            );
            self.stack1.pop();
            if self.stop {
                return;
            }
        }
    }

    /// Finish a cube that has reached the subgroup.
    fn phase2_from_here(&mut self, last_face: usize) {
        // A phase 1 ending on a phase 2 move is a longer way of writing a
        // shorter phase 1, so it has already been tried.
        if let Some(&last) = self.stack1.last()
            && PHASE2_MOVES.contains(&last)
        {
            return;
        }
        let mut cube = self.start;
        for &m in &self.stack1 {
            cube.apply(&self.tables.moves[m]);
        }
        let (corners, edges, slice) = (cube.corners(), cube.ud_edges(), cube.slice_sorted());
        let Some(budget) = self.best_len.checked_sub(self.stack1.len() + 1) else {
            return;
        };
        let floor = self.tables.phase2_distance(corners, edges, slice) as usize;
        for depth in floor..=budget {
            self.stack2.clear();
            if self.phase2(corners, edges, slice, depth, last_face) {
                self.best_len = self.stack1.len() + self.stack2.len();
                self.best = Some((self.stack1.clone(), self.stack2.clone()));
                if self.best_len <= self.limits.good_enough {
                    self.stop = true;
                }
                return;
            }
            if self.stop {
                return;
            }
        }
    }

    fn phase2(
        &mut self,
        corners: u16,
        edges: u16,
        slice: u16,
        depth: usize,
        last_face: usize,
    ) -> bool {
        if depth == 0 {
            return corners == 0 && edges == 0 && slice == 0;
        }
        if (self.tables.phase2_distance(corners, edges, slice) as usize) > depth {
            return false;
        }
        self.nodes += 1;
        if self.nodes > self.limits.max_nodes {
            self.stop = true;
            return false;
        }
        for (i, &face_move) in PHASE2_MOVES.iter().enumerate() {
            if !allowed(move_face(face_move), last_face) {
                continue;
            }
            self.stack2.push(i);
            let found = self.phase2(
                self.tables.corners(corners, i),
                self.tables.ud_edges(edges, i),
                self.tables.slice_perm(slice, i),
                depth - 1,
                move_face(face_move),
            );
            if found {
                return true;
            }
            self.stack2.pop();
            if self.stop {
                return false;
            }
        }
        false
    }
}

/// Never turn the same face twice in a row, and fix an order for the two faces
/// of an axis, which commute.
const fn allowed(face: usize, last_face: usize) -> bool {
    last_face == usize::MAX || (face != last_face && face + 3 != last_face)
}
