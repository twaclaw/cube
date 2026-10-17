//! SVG rendering. Both views are vector graphics, so what the browser shows is
//! exactly what gets exported.

pub mod cube3d;
pub mod cycles;
pub mod net;
pub mod svg;

pub use cube3d::{CAMERA_PRESETS, Camera, Facet, Style3d};
pub use cycles::CycleStyle;
pub use net::NetStyle;
pub use svg::P2;

use crate::geom::Face;
use crate::palette::Rgb;
use std::collections::{BTreeMap, BTreeSet};

/// A facelet, as `(face, row, col)`.
pub type Facelet = (Face, u8, u8);

/// A plain set of facelets, which is what clicking the net collects.
pub type Highlights = BTreeSet<Facelet>;

/// What to draw on top of the colors: repainted stickers, rings, and text.
///
/// This is how a blank cube is marked up to show which cubies are corners,
/// which stay put under a set of moves, and so on.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Marks {
    /// Paint the sticker this color instead of its own.
    pub fill: BTreeMap<Facelet, Rgb>,
    /// Draw a ring round the sticker.
    pub ring: BTreeMap<Facelet, Rgb>,
    /// Write this on the sticker, in place of its number.
    pub label: BTreeMap<Facelet, String>,
}

impl Marks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ring every facelet in a set, as the highlight field does.
    pub fn rings(facelets: &Highlights, color: Rgb) -> Self {
        Self {
            ring: facelets.iter().map(|f| (*f, color)).collect(),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fill.is_empty() && self.ring.is_empty() && self.label.is_empty()
    }

    pub fn paint(&mut self, facelet: Facelet, color: Rgb) -> &mut Self {
        self.fill.insert(facelet, color);
        self
    }

    pub fn ring_at(&mut self, facelet: Facelet, color: Rgb) -> &mut Self {
        self.ring.insert(facelet, color);
        self
    }

    pub fn write(&mut self, facelet: Facelet, text: impl Into<String>) -> &mut Self {
        self.label.insert(facelet, text.into());
        self
    }

    /// Everything from `other` as well, with `other` winning.
    pub fn merge(&mut self, other: &Marks) -> &mut Self {
        self.fill.extend(other.fill.iter().map(|(k, v)| (*k, *v)));
        self.ring.extend(other.ring.iter().map(|(k, v)| (*k, *v)));
        self.label
            .extend(other.label.iter().map(|(k, v)| (*k, v.clone())));
        self
    }
}
