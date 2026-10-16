//! SVG rendering. Both views are vector graphics, so what the browser shows is
//! exactly what gets exported.

pub mod cube3d;
pub mod net;
pub mod svg;

pub use cube3d::{CAMERA_PRESETS, Camera, Facet, Style3d};
pub use net::NetStyle;
pub use svg::P2;

use crate::geom::Face;
use std::collections::BTreeSet;

/// Facelets drawn with a ring round them, as `(face, row, col)`.
pub type Highlights = BTreeSet<(Face, u8, u8)>;
