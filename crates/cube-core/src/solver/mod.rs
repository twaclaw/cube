//! Solving a 3x3 with Kociemba's two-phase algorithm.

pub mod cubie;
pub mod lehmer;
pub mod search;
pub mod tables;

use crate::cube::Cube;
use crate::moves::{Move, Turn};
use core::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SolveError {
    /// The state cannot be reached by turning faces.
    Unreachable,
    /// The solver gave up (search limits).
    GaveUp,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            SolveError::Unreachable => "this state cannot be reached by turning faces",
            SolveError::GaveUp => "the solver gave up",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for SolveError {}

/// Why a set of colors is not a state the cube can be in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flaw {
    /// Some piece shows a pair of colors no cubie carries: the same color
    /// twice, or two faces that never meet.
    Pieces,
    /// Every piece is a real one, but no sequence of turns arranges them so:
    /// one is swapped, flipped or twisted on its own.
    Parity,
}

/// What stops these colors being a state, if anything does: the checks a
/// solve would make anyway, without building the tables a solve needs.
///
/// For a 3x3, which is the only size the cubie level is written for.
pub fn check(cube: &Cube) -> Result<(), Flaw> {
    match cubie::CubieCube::from_cube(cube) {
        None => Err(Flaw::Pieces),
        Some(c) if !c.is_solvable() => Err(Flaw::Parity),
        Some(_) => Ok(()),
    }
}

pub trait Solver {
    fn name(&self) -> &str;

    /// A maneuver that takes `cube` to the solved state.
    fn solve(&self, cube: &Cube) -> Result<Vec<Move>, SolveError>;
}

/// A two-phase solution, kept in its two halves: the first drives the cube
/// into the subgroup `<U, D, R2, L2, F2, B2>`, the second solves it there.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Solution {
    pub phase1: Vec<Move>,
    pub phase2: Vec<Move>,
    /// Search nodes visited, for the record.
    pub nodes: u64,
}

impl Solution {
    pub fn len(&self) -> usize {
        self.phase1.len() + self.phase2.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Both phases, one after the other.
    pub fn moves(&self) -> Vec<Move> {
        self.phase1.iter().chain(&self.phase2).cloned().collect()
    }
}

/// Kociemba's two-phase solver, holding the tables it searches with.
///
/// Building the tables takes a moment and a few megabytes, so make one and
/// keep it.
pub struct Kociemba {
    tables: tables::Tables,
}

impl Kociemba {
    pub fn new() -> Self {
        Self {
            tables: tables::Tables::new(),
        }
    }

    /// Size of the tables in bytes.
    pub fn table_bytes(&self) -> usize {
        self.tables.bytes()
    }

    /// Solve, looking as hard as `limits` allows.
    pub fn solve_with(&self, cube: &Cube, limits: search::Limits) -> Result<Solution, SolveError> {
        let start = cubie::CubieCube::from_cube(cube).ok_or(SolveError::Unreachable)?;
        if !start.is_solvable() {
            return Err(SolveError::Unreachable);
        }
        let mut search = search::Search::new(&self.tables, start, limits);
        let (phase1, phase2, nodes) = search.run_counting().ok_or(SolveError::GaveUp)?;
        Ok(Solution {
            phase1: phase1.into_iter().map(named).collect(),
            phase2: phase2
                .into_iter()
                .map(|i| named(tables::PHASE2_MOVES[i]))
                .collect(),
            nodes,
        })
    }
}

impl Default for Kociemba {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver for Kociemba {
    fn name(&self) -> &str {
        "kociemba"
    }

    fn solve(&self, cube: &Cube) -> Result<Vec<Move>, SolveError> {
        Ok(self.solve_with(cube, search::Limits::default())?.moves())
    }
}

/// A solver move index as a written move.
fn named(m: usize) -> Move {
    Move::new(
        Turn::face(3, tables::FACES[m / 3], (m % 3) as i8 + 1),
        tables::move_name(m),
    )
}
