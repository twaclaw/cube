//! Reading a cube off a camera.
//!
//! Six photographs give 54 sampled colors; this turns them into a state. The
//! hard part is the color, not the geometry: a red sticker under warm light
//! and an orange one under cold light arrive as much the same pixel. So no
//! sticker is decided on its own. The centers name the six colors, every
//! color is carried by exactly nine stickers, and the reading is the cheapest
//! labelling that obeys both. That second constraint is what tells red from
//! orange.

use crate::cube::Cube;
use crate::geom::Face;
use crate::palette::Rgb;
use crate::render::Facelet;

/// Facelets of a 3x3.
const N: usize = 54;
/// Stickers of each color, which is also the stickers of each face.
const PER_FACE: usize = 9;

/// A sampled color for every facelet, in numbering order (U, L, F, R, B, D,
/// row-major), centers included: what six photographs of a 3x3 give.
pub type Samples = [Rgb; N];

/// What the camera made of the cube.
#[derive(Clone, Debug)]
pub struct Reading {
    pub cube: Cube,
    /// Facelets whose color was a close call, worth a second look.
    pub unsure: Vec<Facelet>,
}

/// How many times the anchors are re-centered on what they caught.
const REFINEMENTS: usize = 2;

/// Lightness counts for less than hue in the distance between two samples. A
/// sticker in shadow is still its own color, and a face shot against a window
/// is not a different face; what separates the six colors is where they sit
/// in the a-b plane.
const LIGHTNESS_WEIGHT: f64 = 0.4;

/// A sticker is flagged when the runner-up color came this close, measured as
/// the distance below. Stickers the nine-of-each count had to pull away from
/// their nearest color come out negative here, so they are flagged too.
const CLOSE_CALL: f64 = 10.0;

/// Distances as whole numbers, which is what the assignment works in.
const SCALE: f64 = 256.0;

/// Cost of giving a center any color but its own: more than a whole reading
/// of honest distances can add up to, so it never happens.
const PINNED: i64 = 1_000_000_000;

/// Turn 54 sampled colors into the state they came from.
///
/// The samples arrive in numbering order, so the center of each face is the
/// fifth sample of its block; that center names the color of its face, which
/// is what makes the six anchors.
pub fn read(samples: &Samples) -> Reading {
    let labs: Vec<[f64; 3]> = samples.iter().map(|c| lab(*c)).collect();
    let mut anchors: [[f64; 3]; 6] = core::array::from_fn(|f| labs[center(f)]);
    let mut choice = assign(&labs, &anchors);
    // One sampled pixel is a thin description of a color. Average what each
    // color actually caught and assign again: after a round or two an anchor
    // stands for the color as all six photographs saw it, rather than as one
    // sticker in one light did.
    for _ in 0..REFINEMENTS {
        anchors = core::array::from_fn(|f| mean(&labs, &choice, f));
        choice = assign(&labs, &anchors);
    }

    let mut cube = Cube::solved(3);
    let mut unsure = Vec::new();
    for (i, &color) in choice.iter().enumerate() {
        let (face, row, col) = facelet_of(i);
        cube.set(face, row, col, Face::ALL[color]);
        let own = distance(labs[i], anchors[color]);
        let rival = (0..6)
            .filter(|f| *f != color)
            .map(|f| distance(labs[i], anchors[f]))
            .fold(f64::INFINITY, f64::min);
        if rival - own < CLOSE_CALL {
            unsure.push((face, row, col));
        }
    }
    Reading { cube, unsure }
}

/// Which sample names the color of face `f`.
const fn center(f: usize) -> usize {
    f * PER_FACE + 4
}

/// Where the `i`th sample belongs on the cube.
fn facelet_of(i: usize) -> Facelet {
    (
        Face::ALL[i / PER_FACE],
        ((i % PER_FACE) / 3) as u8,
        (i % 3) as u8,
    )
}

/// Give every sticker a color, nine of each, as cheaply as possible.
///
/// Nearest anchor on its own is not enough: it has no reason to hand out nine
/// of each, and under warm light it will happily read six reds and twelve
/// oranges. Written as an assignment -- 54 stickers into 54 places, nine per
/// color, the centers nailed to their own -- the counts come out right by
/// construction, and the stickers that are sure of themselves push the
/// doubtful ones into the places left over.
fn assign(labs: &[[f64; 3]], anchors: &[[f64; 3]; 6]) -> Vec<usize> {
    let mut cost = vec![vec![0i64; N]; N];
    for (i, row) in cost.iter_mut().enumerate() {
        let pinned = (0..6).find(|f| center(*f) == i);
        for (j, slot) in row.iter_mut().enumerate() {
            let color = j / PER_FACE;
            *slot = match pinned {
                Some(own) => {
                    if own == color {
                        0
                    } else {
                        PINNED
                    }
                }
                None => (distance(labs[i], anchors[color]) * SCALE) as i64,
            };
        }
    }
    hungarian(&cost)
        .into_iter()
        .map(|place| place / PER_FACE)
        .collect()
}

/// Where the stickers given to one color sit, on average. The assignment
/// hands every color exactly nine, so that is what this divides by.
fn mean(labs: &[[f64; 3]], choice: &[usize], color: usize) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for (lab, c) in labs.iter().zip(choice) {
        if *c == color {
            for (s, v) in sum.iter_mut().zip(lab) {
                *s += v;
            }
        }
    }
    sum.map(|v| v / PER_FACE as f64)
}

/// How far apart two samples are, in the Lab space below and with lightness
/// discounted by [`LIGHTNESS_WEIGHT`].
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (LIGHTNESS_WEIGHT * d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// sRGB to CIE Lab, against the D65 white point sRGB is defined in.
///
/// Distances in RGB do not mean much -- the gap from red to orange is a
/// smaller number there than the gap between one white sticker and another in
/// a different light. Lab is near enough uniform that a distance in it stands
/// for how different two colors look.
fn lab(c: Rgb) -> [f64; 3] {
    let linear = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(c.0), linear(c.1), linear(c.2));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            t * 841.0 / 108.0 + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Minimum-cost perfect matching on a square matrix: the Hungarian algorithm
/// in its O(n^3) shortest-augmenting-path form, keeping a potential on each
/// row and column so every search runs on non-negative reduced costs.
///
/// Returns the column chosen for each row.
fn hungarian(cost: &[Vec<i64>]) -> Vec<usize> {
    let n = cost.len();
    const INF: i64 = i64::MAX / 4;
    // One extra entry at the front: column 0 is the free column an augmenting
    // path starts from, so the real columns are 1..=n.
    let mut row_pot = vec![0i64; n + 1];
    let mut col_pot = vec![0i64; n + 1];
    // Row matched to each column, 0 while the column is unmatched.
    let mut matched = vec![0usize; n + 1];
    let mut parent = vec![0usize; n + 1];
    for i in 1..=n {
        matched[0] = i;
        let mut col = 0usize;
        let mut best = vec![INF; n + 1];
        let mut done = vec![false; n + 1];
        // Grow the path one column at a time until it reaches a free one.
        loop {
            done[col] = true;
            let row = matched[col];
            let mut step = INF;
            let mut next = 0usize;
            for j in 1..=n {
                if done[j] {
                    continue;
                }
                let reduced = cost[row - 1][j - 1] - row_pot[row] - col_pot[j];
                if reduced < best[j] {
                    best[j] = reduced;
                    parent[j] = col;
                }
                if best[j] < step {
                    step = best[j];
                    next = j;
                }
            }
            for j in 0..=n {
                if done[j] {
                    row_pot[matched[j]] += step;
                    col_pot[j] -= step;
                } else {
                    best[j] -= step;
                }
            }
            col = next;
            if matched[col] == 0 {
                break;
            }
        }
        // Walk back along the path, moving every match on by one column.
        while col != 0 {
            let previous = parent[col];
            matched[col] = matched[previous];
            col = previous;
        }
    }
    let mut chosen = vec![0usize; n];
    for j in 1..=n {
        chosen[matched[j] - 1] = j - 1;
    }
    chosen
}
