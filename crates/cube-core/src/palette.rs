//! Colors. The defaults are the standard (Western/BOY) scheme the notebooks
//! use: U white, L orange, F green, R red, B blue, D yellow.

use crate::geom::Face;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn from_hex(hex: u32) -> Self {
        Self(
            ((hex >> 16) & 0xff) as u8,
            ((hex >> 8) & 0xff) as u8,
            (hex & 0xff) as u8,
        )
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Darken (`k < 1`) or lift toward white (`k > 1`).
    pub fn shade(self, k: f64) -> Self {
        let f = |c: u8| {
            let c = c as f64;
            let v = if k <= 1.0 {
                c * k
            } else {
                c + (255.0 - c) * (k - 1.0).min(1.0)
            };
            v.round().clamp(0.0, 255.0) as u8
        };
        Self(f(self.0), f(self.1), f(self.2))
    }

    /// Perceived brightness, for picking readable label colors.
    pub fn luma(self) -> f64 {
        (0.299 * self.0 as f64 + 0.587 * self.1 as f64 + 0.114 * self.2 as f64) / 255.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// One color per face, in `Face` order (U, L, F, R, B, D).
    pub faces: [Rgb; 6],
    /// The cubie body seen between the stickers.
    pub plastic: Rgb,
    /// Outline of every sticker.
    pub edge: Rgb,
    /// Ring drawn around highlighted facelets.
    pub highlight: Rgb,
}

impl Palette {
    pub const fn face(&self, f: Face) -> Rgb {
        self.faces[f as usize]
    }
}

pub const STANDARD: Palette = Palette {
    faces: [
        Rgb::from_hex(0xfdfdfd), // U white
        Rgb::from_hex(0xff5800), // L orange
        Rgb::from_hex(0x009e60), // F green
        Rgb::from_hex(0xc41e3a), // R red
        Rgb::from_hex(0x0051ba), // B blue
        Rgb::from_hex(0xffd500), // D yellow
    ],
    plastic: Rgb::from_hex(0x1a1a1f),
    edge: Rgb::from_hex(0x101014),
    highlight: Rgb::from_hex(0xd10a6e),
};
