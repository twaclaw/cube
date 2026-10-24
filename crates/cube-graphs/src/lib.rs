//! Figures about the cube that the app does not need.
//!
//! The web app draws states: a cube in 3D, a cube unfolded, a maneuver as a
//! strip or an animation. This crate draws the things *around* a state, which
//! only a notebook or a book has room for: so far, the trees that show a table
//! being filled and a search being walked.
//!
//! Everything here renders through `cube_core::render`, so a figure is one SVG
//! document with no script and no dependency, and the same figure can be
//! wrapped in a little HTML when a page can offer more (see
//! [`tree::Tree::html`]).

pub mod search;
pub mod tables;
pub mod tree;

pub use search::{Phase, search_tree};
pub use tables::{Coord, table_tree};
pub use tree::{Detail, Mark, Node, Style, Tree};
