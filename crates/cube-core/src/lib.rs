//! A Rubik's cube, drawn twice: as a projected 3D cube and as a flat net, both
//! in SVG, both exportable.
//!
//! The model is written for any cube size; only the future solver will be
//! specific to the 3x3.

pub mod cube;
pub mod export;
pub mod geom;
pub mod moves;
pub mod notation;
pub mod palette;
pub mod render;
pub mod scramble;
pub mod solver;

pub use cube::Cube;
pub use geom::{Axis, Face};
pub use moves::{Move, Turn};
pub use notation::{ParseError, parse};
pub use palette::{Palette, Rgb};
pub use render::{Camera, Highlights, NetStyle, Style3d};
pub use scramble::{Rng, scramble};
