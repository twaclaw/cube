//! Disjoint cycle notation, drawn: every number is the facelet it stands for,
//! in the color of the face that facelet belongs to.

use super::svg::{num, open, rounded_path};
use crate::geom::facelet_from_label;
use crate::palette::Palette;
use crate::permutation::Permutation;

#[derive(Clone, Copy, Debug)]
pub struct CycleStyle {
    /// Height of a facelet chip, in user units.
    pub chip: f64,
    /// Space either side of the number inside a chip.
    pub pad: f64,
    /// Space between chips of one cycle.
    pub gap: f64,
    /// Space between cycles.
    pub cycle_gap: f64,
    pub round: f64,
    pub font: f64,
    pub stroke: f64,
    /// Wrap to a new line past this width.
    pub width: f64,
}

impl Default for CycleStyle {
    fn default() -> Self {
        Self {
            chip: 26.0,
            pad: 5.0,
            gap: 2.0,
            cycle_gap: 13.0,
            round: 6.0,
            font: 14.0,
            stroke: 1.0,
            width: 760.0,
        }
    }
}

impl CycleStyle {
    fn chip_width(&self, digits: usize) -> f64 {
        (self.font * 0.62 * digits as f64 + 2.0 * self.pad).max(self.chip * 0.8)
    }

    fn paren_width(&self) -> f64 {
        self.font * 0.5
    }
}

/// A drawn piece of cycle notation, and how tall it came out.
pub struct Drawing {
    pub markup: String,
    pub width: f64,
    pub height: f64,
}

/// Lay the cycles out, wrapping at `st.width`.
pub fn draw(perm: &Permutation, n: u8, pal: &Palette, st: &CycleStyle) -> Drawing {
    let cycles = perm.cycles();
    let line = st.chip * 1.45;
    let paren = st.paren_width();
    let ink = "#52525e";

    if cycles.is_empty() {
        let markup = format!(
            "<text x=\"0\" y=\"{}\" font-family=\"ui-monospace,SFMono-Regular,Menlo,monospace\" \
             font-size=\"{}\" fill=\"{ink}\">()</text>",
            num(st.chip * 0.72),
            num(st.font)
        );
        return Drawing {
            markup,
            width: paren * 2.0,
            height: line,
        };
    }

    let mut out = String::new();
    let (mut x, mut y, mut used) = (0.0f64, 0.0f64, 0.0f64);
    for cycle in &cycles {
        let widths: Vec<f64> = cycle
            .iter()
            .map(|k| st.chip_width(k.to_string().len()))
            .collect();
        let span: f64 = widths.iter().sum::<f64>()
            + st.gap * (cycle.len() - 1) as f64
            + 2.0 * paren
            + st.cycle_gap;
        if x > 0.0 && x + span > st.width {
            x = 0.0;
            y += line;
        }
        let text = |at: f64, value: &str, fill: &str| {
            format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" \
                 font-family=\"ui-monospace,SFMono-Regular,Menlo,monospace\" font-size=\"{}\" \
                 fill=\"{fill}\">{value}</text>",
                num(at),
                num(y + st.chip * 0.5),
                num(st.font)
            )
        };
        out.push_str(&text(x + paren * 0.5, "(", ink));
        let mut at = x + paren;
        for (k, width) in cycle.iter().zip(&widths) {
            let color = facelet_from_label(n, *k)
                .map(|(face, _, _)| pal.face(face))
                .unwrap_or(pal.plastic);
            let corners = [
                [at, y],
                [at + width, y],
                [at + width, y + st.chip],
                [at, y + st.chip],
            ];
            out.push_str(&format!(
                "<path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                rounded_path(&corners, st.round),
                color.hex(),
                pal.edge.hex(),
                num(st.stroke)
            ));
            let on_top = if color.luma() > 0.55 {
                "#1a1a1f"
            } else {
                "#ffffff"
            };
            out.push_str(&text(at + width * 0.5, &k.to_string(), on_top));
            at += width + st.gap;
        }
        out.push_str(&text(at - st.gap + paren * 0.5, ")", ink));
        x += span;
        used = used.max(x - st.cycle_gap);
    }
    // A little room so the first bracket is not flush with the edge.
    let margin = st.font * 0.35;
    Drawing {
        markup: format!(
            "<g transform=\"translate({} {})\">{out}</g>",
            num(margin),
            num(margin)
        ),
        width: used + 2.0 * margin,
        height: y + line + 2.0 * margin,
    }
}

/// A standalone SVG of the cycles.
pub fn document(
    perm: &Permutation,
    n: u8,
    pal: &Palette,
    st: &CycleStyle,
    background: Option<&str>,
) -> String {
    let drawn = draw(perm, n, pal, st);
    let mut s = open(drawn.width, drawn.height, background);
    s.push_str(&drawn.markup);
    s.push_str("</svg>");
    s
}
