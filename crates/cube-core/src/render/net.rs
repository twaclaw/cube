//! The 2D view: the cube unfolded, laid out as in the notebooks.
//!
//! ```text
//!        U
//!     L  F  R  B
//!        D
//! ```
//! Facelets can carry their Singmaster number and a highlight ring, the way the
//! notebooks ring the facelets that break an invariant.

use super::Marks;
use super::svg::{P2, num, open, rounded_path};
use crate::cube::Cube;
use crate::geom::{Face, facelet_label};
use crate::palette::Palette;

/// Position of each face in the net, as (row, column) blocks of `n` cells.
const BLOCKS: [(Face, u8, u8); 6] = [
    (Face::U, 0, 1),
    (Face::L, 1, 0),
    (Face::F, 1, 1),
    (Face::R, 1, 2),
    (Face::B, 1, 3),
    (Face::D, 2, 1),
];

#[derive(Clone, Copy, Debug)]
pub struct NetStyle {
    /// Side of one facelet cell, in user units.
    pub cell: f64,
    /// Gutter between stickers, as a fraction of a cell.
    pub gap: f64,
    /// Space between faces, in cells.
    pub face_gap: f64,
    /// Corner rounding, as a fraction of a sticker side.
    pub round: f64,
    pub stroke: f64,
    /// Draw the Singmaster number on every facelet and the letter on centers.
    pub numbers: bool,
    /// Plastic plate behind each face.
    pub plate: bool,
}

impl Default for NetStyle {
    fn default() -> Self {
        Self {
            cell: 34.0,
            gap: 0.1,
            face_gap: 0.3,
            round: 0.18,
            stroke: 0.9,
            numbers: false,
            plate: true,
        }
    }
}

impl NetStyle {
    /// Size of the whole net in user units.
    pub fn extent(&self, n: u8) -> (f64, f64) {
        let margin = self.cell * 0.35;
        let span = |faces: f64, gaps: f64| {
            2.0 * margin + faces * n as f64 * self.cell + gaps * self.face_gap * self.cell
        };
        (span(4.0, 3.0), span(3.0, 2.0))
    }

    fn origin(&self, n: u8, row_block: u8, col_block: u8) -> P2 {
        let margin = self.cell * 0.35;
        let step = n as f64 * self.cell + self.face_gap * self.cell;
        [
            margin + col_block as f64 * step,
            margin + row_block as f64 * step,
        ]
    }
}

fn cell_corners(x: f64, y: f64, side: f64) -> [P2; 4] {
    [[x, y], [x + side, y], [x + side, y + side], [x, y + side]]
}

/// SVG markup for the net, without the enclosing `<svg>` element.
pub fn markup(cube: &Cube, st: &NetStyle, pal: &Palette, marks: &Marks) -> String {
    let n = cube.n();
    let inset = st.gap * st.cell * 0.5;
    let side = st.cell - 2.0 * inset;
    let mut plates = String::new();
    let mut stickers = format!(
        "<g stroke-linejoin=\"round\" stroke=\"{}\" stroke-width=\"{}\">",
        pal.edge.hex(),
        num(st.stroke)
    );
    let mut labels = String::new();
    let mut rings = String::new();

    for (face, row_block, col_block) in BLOCKS {
        let [ox, oy] = st.origin(n, row_block, col_block);
        if st.plate {
            let span = n as f64 * st.cell;
            let pad = inset * 0.6;
            let d = rounded_path(
                &cell_corners(ox - pad, oy - pad, span + 2.0 * pad),
                st.cell * 0.3,
            );
            plates.push_str(&format!("<path d=\"{d}\" fill=\"{}\"/>", pal.plastic.hex()));
        }
        for row in 0..n {
            for col in 0..n {
                let x = ox + col as f64 * st.cell + inset;
                let y = oy + row as f64 * st.cell + inset;
                let pts = cell_corners(x, y, side);
                let d = rounded_path(&pts, st.round * side);
                let color = marks
                    .fill
                    .get(&(face, row, col))
                    .copied()
                    .unwrap_or_else(|| pal.face(cube.get(face, row, col)));
                stickers.push_str(&format!(
                    "<path d=\"{d}\" fill=\"{}\" data-facelet=\"{}-{row}-{col}\"/>",
                    color.hex(),
                    face.letter()
                ));
                let written = marks.label.get(&(face, row, col));
                if st.numbers || written.is_some() {
                    let text = match (written, facelet_label(n, face, row, col)) {
                        (Some(text), _) => text.clone(),
                        (None, Some(k)) => k.to_string(),
                        (None, None) => face.letter().to_string(),
                    };
                    let ink = if color.luma() > 0.55 {
                        "#1a1a1f"
                    } else {
                        "#ffffff"
                    };
                    labels.push_str(&format!(
                        "<text x=\"{}\" y=\"{}\" fill=\"{ink}\" font-size=\"{}\">{text}</text>",
                        num(x + side * 0.5),
                        num(y + side * 0.5),
                        num(side * 0.42)
                    ));
                }
                if let Some(ring) = marks.ring.get(&(face, row, col)) {
                    rings.push_str(&format!(
                        "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                        ring.hex(),
                        num(st.cell * 0.11)
                    ));
                }
            }
        }
    }
    stickers.push_str("</g>");
    let labels = if labels.is_empty() {
        String::new()
    } else {
        format!(
            "<g text-anchor=\"middle\" dominant-baseline=\"central\" \
             font-family=\"ui-sans-serif,system-ui,sans-serif\" font-weight=\"600\">{labels}</g>"
        )
    };
    let rings = if rings.is_empty() {
        String::new()
    } else {
        format!("<g stroke-linejoin=\"round\">{rings}</g>")
    };
    format!("{plates}{stickers}{labels}{rings}")
}

/// A standalone SVG document.
pub fn document(
    cube: &Cube,
    st: &NetStyle,
    pal: &Palette,
    marks: &Marks,
    background: Option<&str>,
) -> String {
    let (w, h) = st.extent(cube.n());
    let mut s = open(w, h, background);
    s.push_str(&markup(cube, st, pal, marks));
    s.push_str("</svg>");
    s
}
