//! Seat for the solver that comes later.
//!
//! The UI already has its button; implementing this trait (a two-phase
//! Kociemba search for the 3x3) is all that is missing.

use crate::cube::Cube;
use crate::moves::Move;
use core::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SolveError {
    /// No solver is wired up yet.
    NotImplemented,
    /// The state cannot be reached by turning faces.
    Unreachable,
    /// The solver gave up (search limits).
    GaveUp,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            SolveError::NotImplemented => "the solver is not implemented yet",
            SolveError::Unreachable => "this state cannot be reached by turning faces",
            SolveError::GaveUp => "the solver gave up",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for SolveError {}

pub trait Solver {
    fn name(&self) -> &str;

    /// A maneuver that takes `cube` to the solved state.
    fn solve(&self, cube: &Cube) -> Result<Vec<Move>, SolveError>;
}

/// Placeholder so the rest of the app can already talk to a solver.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pending;

impl Solver for Pending {
    fn name(&self) -> &str {
        "pending"
    }

    fn solve(&self, _cube: &Cube) -> Result<Vec<Move>, SolveError> {
        Err(SolveError::NotImplemented)
    }
}
