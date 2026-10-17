//! A cube state read as a permutation of its numbered facelets.
//!
//! The colors alone name a permutation only when every cubie carries a
//! different set of them, which is true up to a 3x3. A 4x4 has interchangeable
//! edge and center stickers, so there [`Permutation::of`] gives `None`.

use crate::cube::Cube;
use crate::geom::{Face, facelet_at, facelet_label, facelet_position, layer_index};
use std::collections::{BTreeSet, HashMap};

/// Where each numbered facelet went: `image[i - 1]` is the number of the place
/// the facelet numbered `i` now occupies, as SageMath's `CubeGroup` reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Permutation {
    image: Vec<u32>,
}

impl Permutation {
    /// The permutation a state names, if its stickers are all distinguishable.
    pub fn of(cube: &Cube) -> Option<Self> {
        let n = cube.n();
        let solved = Cube::solved(n);
        let numbered: Vec<(Face, u8, u8, u32)> = Face::ALL
            .into_iter()
            .flat_map(|f| (0..n).flat_map(move |r| (0..n).map(move |c| (f, r, c))))
            .filter_map(|(f, r, c)| facelet_label(n, f, r, c).map(|k| (f, r, c, k)))
            .collect();

        // A facelet is named by the color it shows and the colors of its cubie.
        let mut home = HashMap::with_capacity(numbered.len());
        for &(f, r, c, k) in &numbered {
            let key = (solved.get(f, r, c), cubie_colors(&solved, n, f, r, c));
            if home.insert(key, k).is_some() {
                return None;
            }
        }

        let mut image = vec![0; numbered.len()];
        for &(f, r, c, k) in &numbered {
            let key = (cube.get(f, r, c), cubie_colors(cube, n, f, r, c));
            let from = *home.get(&key)?;
            image[from as usize - 1] = k;
        }
        Some(Self { image })
    }

    /// How many facelets the permutation is on.
    pub fn len(&self) -> usize {
        self.image.len()
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty()
    }

    pub fn is_identity(&self) -> bool {
        self.image
            .iter()
            .enumerate()
            .all(|(i, k)| *k as usize == i + 1)
    }

    /// Facelets that do not stay where they are.
    pub fn moved(&self) -> usize {
        self.image
            .iter()
            .enumerate()
            .filter(|(i, k)| **k as usize != i + 1)
            .count()
    }

    /// The disjoint cycles, each starting at its smallest member, in order of
    /// those smallest members. Fixed points are left out.
    pub fn cycles(&self) -> Vec<Vec<u32>> {
        let mut seen = vec![false; self.image.len()];
        let mut out = Vec::new();
        for start in 0..self.image.len() {
            if seen[start] || self.image[start] as usize == start + 1 {
                continue;
            }
            let mut cycle = Vec::new();
            let mut at = start;
            while !seen[at] {
                seen[at] = true;
                cycle.push(at as u32 + 1);
                at = self.image[at] as usize - 1;
            }
            out.push(cycle);
        }
        out
    }

    /// Cycle lengths above one, longest first.
    pub fn cycle_type(&self) -> Vec<usize> {
        let mut lengths: Vec<usize> = self.cycles().iter().map(Vec::len).collect();
        lengths.sort_unstable_by(|a, b| b.cmp(a));
        lengths
    }

    /// Least `k > 0` with `self^k` the identity: the lcm of the cycle lengths.
    pub fn order(&self) -> u64 {
        self.cycle_type()
            .into_iter()
            .fold(1u64, |acc, len| lcm(acc, len as u64))
    }

    /// Disjoint cycle notation, written as SageMath writes it: `(1,3,8,6)(2,5,7,4)`,
    /// and `()` for the identity.
    pub fn to_cycle_notation(&self) -> String {
        let cycles = self.cycles();
        if cycles.is_empty() {
            return "()".to_string();
        }
        let mut out = String::new();
        for cycle in cycles {
            out.push('(');
            for (i, point) in cycle.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&point.to_string());
            }
            out.push(')');
        }
        out
    }
}

/// The colors showing on the cubie a facelet belongs to.
fn cubie_colors(cube: &Cube, n: u8, f: Face, r: u8, c: u8) -> BTreeSet<Face> {
    let cell = facelet_position(n, f, r, c);
    Face::ALL
        .into_iter()
        .filter(|dir| {
            let (axis, positive) = dir.axis();
            layer_index(n, axis, cell) == if positive { n - 1 } else { 0 }
        })
        .filter_map(|dir| facelet_at(n, cell, dir.normal()))
        .map(|(df, dr, dc)| cube.get(df, dr, dc))
        .collect()
}

fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }
    a / gcd(a, b) * b
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}
