//! A Rubik's cube, drawn twice: as a projected 3D cube and as a flat net, both
//! in SVG, both exportable.
//!
//! The model is written for any cube size; only the future solver will be
//! specific to the 3x3.

pub mod cube;
pub mod entry;
pub mod export;
pub mod geom;
pub mod moves;
pub mod notation;
pub mod palette;
pub mod permutation;
pub mod pieces;
pub mod render;
pub mod scan;
pub mod scramble;
pub mod solver;

pub use cube::Cube;
pub use entry::Editor;
pub use geom::{Axis, Face};
pub use moves::{Move, Turn};
pub use notation::{ParseError, parse};
pub use palette::{Palette, Rgb};
pub use permutation::Permutation;
pub use render::{Camera, Highlights, Marks, NetStyle, Style3d};
pub use scan::Reading;
pub use scramble::{Rng, scramble};
