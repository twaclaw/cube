//! The cube at the cubie level: which cubie sits in each slot and how it is
//! turned. This is the representation Kociemba's coordinates are read from.

use super::lehmer;
use crate::cube::Cube;
use crate::geom::Face;
use crate::moves::Turn;

pub const N_TWIST: usize = 2187; // 3^7 corner orientations
pub const N_FLIP: usize = 2048; // 2^11 edge orientations
pub const N_PERM4: usize = 24; // orders of the four slice edges
pub const N_SLICE: usize = 495; // places the slice edges can occupy
pub const N_SLICE_SORTED: usize = N_SLICE * N_PERM4;
pub const N_CORNERS: usize = 40320; // 8! corner permutations
pub const N_UD_EDGES: usize = 40320; // 8! permutations of the U and D edges

/// Corner slots, each as its three facelets read clockwise from outside,
/// starting at the U or D facelet. Order: URF UFL ULB UBR DFR DLF DBL DRB.
const CORNERS: [[(Face, u8, u8); 3]; 8] = [
    [(Face::U, 2, 2), (Face::R, 0, 0), (Face::F, 0, 2)],
    [(Face::U, 2, 0), (Face::F, 0, 0), (Face::L, 0, 2)],
    [(Face::U, 0, 0), (Face::L, 0, 0), (Face::B, 0, 2)],
    [(Face::U, 0, 2), (Face::B, 0, 0), (Face::R, 0, 2)],
    [(Face::D, 0, 2), (Face::F, 2, 2), (Face::R, 2, 0)],
    [(Face::D, 0, 0), (Face::L, 2, 2), (Face::F, 2, 0)],
    [(Face::D, 2, 0), (Face::B, 2, 2), (Face::L, 2, 0)],
    [(Face::D, 2, 2), (Face::R, 2, 2), (Face::B, 2, 0)],
];

/// Edge slots, reference facelet first. Order: UR UF UL UB DR DF DL DB, then
/// the four slice edges FR FL BL BR.
const EDGES: [[(Face, u8, u8); 2]; 12] = [
    [(Face::U, 1, 2), (Face::R, 0, 1)],
    [(Face::U, 2, 1), (Face::F, 0, 1)],
    [(Face::U, 1, 0), (Face::L, 0, 1)],
    [(Face::U, 0, 1), (Face::B, 0, 1)],
    [(Face::D, 1, 2), (Face::R, 2, 1)],
    [(Face::D, 0, 1), (Face::F, 2, 1)],
    [(Face::D, 1, 0), (Face::L, 2, 1)],
    [(Face::D, 2, 1), (Face::B, 2, 1)],
    [(Face::F, 1, 2), (Face::R, 1, 0)],
    [(Face::F, 1, 0), (Face::L, 1, 2)],
    [(Face::B, 1, 2), (Face::L, 1, 0)],
    [(Face::B, 1, 0), (Face::R, 1, 2)],
];

/// The first slice edge in the edge order above.
const FR: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CubieCube {
    /// Which corner sits in each slot.
    pub cp: [u8; 8],
    /// How far it is twisted, clockwise.
    pub co: [u8; 8],
    /// Which edge sits in each slot.
    pub ep: [u8; 12],
    /// Whether it is flipped.
    pub eo: [u8; 12],
}

impl Default for CubieCube {
    fn default() -> Self {
        Self::solved()
    }
}

impl CubieCube {
    pub const fn solved() -> Self {
        Self {
            cp: [0, 1, 2, 3, 4, 5, 6, 7],
            co: [0; 8],
            ep: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            eo: [0; 12],
        }
    }

    /// Read a facelet cube. `None` if the colors do not make up a cube.
    pub fn from_cube(cube: &Cube) -> Option<Self> {
        if cube.n() != 3 {
            return None;
        }
        let mut out = Self::solved();
        for (slot, facelets) in CORNERS.iter().enumerate() {
            let colors: [Face; 3] = core::array::from_fn(|k| {
                let (f, r, c) = facelets[k];
                cube.get(f, r, c)
            });
            let twist = colors.iter().position(|c| *c == Face::U || *c == Face::D)?;
            let (next, after) = (colors[(twist + 1) % 3], colors[(twist + 2) % 3]);
            let cubie = CORNERS
                .iter()
                .position(|home| home[1].0 == next && home[2].0 == after)?;
            out.cp[slot] = cubie as u8;
            out.co[slot] = twist as u8;
        }
        for (slot, facelets) in EDGES.iter().enumerate() {
            let colors: [Face; 2] = core::array::from_fn(|k| {
                let (f, r, c) = facelets[k];
                cube.get(f, r, c)
            });
            let (cubie, flip) =
                EDGES
                    .iter()
                    .enumerate()
                    .find_map(|(j, home)| match (home[0].0, home[1].0) {
                        (a, b) if (a, b) == (colors[0], colors[1]) => Some((j, 0)),
                        (a, b) if (b, a) == (colors[0], colors[1]) => Some((j, 1)),
                        _ => None,
                    })?;
            out.ep[slot] = cubie as u8;
            out.eo[slot] = flip;
        }
        out.is_a_cube().then_some(out)
    }

    /// Paint the cubies back onto a facelet cube.
    pub fn to_cube(&self) -> Cube {
        let mut cube = Cube::solved(3);
        for (slot, facelets) in CORNERS.iter().enumerate() {
            let cubie = self.cp[slot] as usize;
            let twist = self.co[slot] as usize;
            for k in 0..3 {
                let (f, r, c) = facelets[(k + twist) % 3];
                cube.set(f, r, c, CORNERS[cubie][k].0);
            }
        }
        for (slot, facelets) in EDGES.iter().enumerate() {
            let cubie = self.ep[slot] as usize;
            let flip = self.eo[slot] as usize;
            for k in 0..2 {
                let (f, r, c) = facelets[(k + flip) % 2];
                cube.set(f, r, c, EDGES[cubie][k].0);
            }
        }
        cube
    }

    /// Both arrays are permutations, which the colors alone do not guarantee.
    fn is_a_cube(&self) -> bool {
        let mut corners = [false; 8];
        let mut edges = [false; 12];
        self.cp.iter().for_each(|&c| corners[c as usize] = true);
        self.ep.iter().for_each(|&e| edges[e as usize] = true);
        corners.iter().all(|&seen| seen) && edges.iter().all(|&seen| seen)
    }

    /// Whether turning faces can reach this state: the twists must cancel, the
    /// flips must cancel, and the two permutations must have equal parity.
    pub fn is_solvable(&self) -> bool {
        let twist: u8 = self.co.iter().sum::<u8>() % 3;
        let flip: u8 = self.eo.iter().sum::<u8>() % 2;
        twist == 0 && flip == 0 && parity(&self.cp) == parity(&self.ep)
    }

    /// Apply `other`, as a move applied to this state.
    pub fn apply(&mut self, other: &CubieCube) {
        let mut cp = [0u8; 8];
        let mut co = [0u8; 8];
        for i in 0..8 {
            let from = other.cp[i] as usize;
            cp[i] = self.cp[from];
            co[i] = (self.co[from] + other.co[i]) % 3;
        }
        let mut ep = [0u8; 12];
        let mut eo = [0u8; 12];
        for i in 0..12 {
            let from = other.ep[i] as usize;
            ep[i] = self.ep[from];
            eo[i] = (self.eo[from] + other.eo[i]) % 2;
        }
        *self = Self { cp, co, ep, eo };
    }

    // ----- Kociemba's coordinates ---------------------------------------

    /// Corner orientations, as a number in base three.
    pub fn twist(&self) -> u16 {
        self.co[..7]
            .iter()
            .fold(0u16, |acc, &o| acc * 3 + u16::from(o))
    }

    pub fn set_twist(&mut self, twist: u16) {
        let mut rest = twist;
        let mut total = 0;
        for i in (0..7).rev() {
            self.co[i] = (rest % 3) as u8;
            total += self.co[i];
            rest /= 3;
        }
        self.co[7] = (3 - total % 3) % 3;
    }

    /// Edge orientations, as a number in base two.
    pub fn flip(&self) -> u16 {
        self.eo[..11]
            .iter()
            .fold(0u16, |acc, &o| acc * 2 + u16::from(o))
    }

    pub fn set_flip(&mut self, flip: u16) {
        let mut rest = flip;
        let mut total = 0;
        for i in (0..11).rev() {
            self.eo[i] = (rest % 2) as u8;
            total += self.eo[i];
            rest /= 2;
        }
        self.eo[11] = total % 2;
    }

    /// Where the four slice edges are and in what order: the places they take
    /// (495 of them) times 24, plus the Lehmer index of their order.
    pub fn slice_sorted(&self) -> u16 {
        let mut places = 0u32;
        let mut found = 0u32;
        let mut order = [0u8; 4];
        // Count down so that the solved state, FR FL BL BR at the back, is 0.
        for slot in (0..12).rev() {
            if self.ep[slot] >= FR {
                places += choose(11 - slot as u32, found + 1);
                order[3 - found as usize] = self.ep[slot] - FR;
                found += 1;
            }
        }
        (places * N_PERM4 as u32 + lehmer::encode(&order)) as u16
    }

    pub fn set_slice_sorted(&mut self, coord: u16) {
        let mut places = u32::from(coord) / N_PERM4 as u32;
        let mut order = [0u8; 4];
        lehmer::decode(u32::from(coord) % N_PERM4 as u32, &mut order);
        // Everything not a slice edge is filled in afterwards.
        self.ep = [u8::MAX; 12];
        let mut left = 4u32;
        for slot in 0..12 {
            let weight = choose(11 - slot as u32, left);
            if left > 0 && places >= weight {
                places -= weight;
                self.ep[slot] = FR + order[(4 - left) as usize];
                left -= 1;
            }
        }
        let mut spare = [0u8; 8];
        let mut count = 0;
        for edge in 0..FR {
            if !self.ep.contains(&edge) {
                spare[count] = edge;
                count += 1;
            }
        }
        let mut next = 0;
        for slot in 0..12 {
            if self.ep[slot] == u8::MAX {
                self.ep[slot] = spare[next];
                next += 1;
            }
        }
    }

    /// The corner permutation, as a Lehmer index.
    pub fn corners(&self) -> u16 {
        lehmer::encode(&self.cp) as u16
    }

    pub fn set_corners(&mut self, coord: u16) {
        lehmer::decode(u32::from(coord), &mut self.cp);
    }

    /// The permutation of the eight U and D edges, as a Lehmer index. Only
    /// meaningful once phase 1 has put those edges in their own faces.
    pub fn ud_edges(&self) -> u16 {
        lehmer::encode(&self.ep[..8]) as u16
    }

    pub fn set_ud_edges(&mut self, coord: u16) {
        let mut ud = [0u8; 8];
        lehmer::decode(u32::from(coord), &mut ud);
        self.ep[..8].copy_from_slice(&ud);
    }
}

/// `n` choose `k`, for the slice coordinate.
fn choose(n: u32, k: u32) -> u32 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (0..k).fold(1, |acc, i| acc * (n - i) / (i + 1))
}

fn parity(perm: &[u8]) -> u8 {
    let mut swaps = 0;
    for i in 0..perm.len() {
        for j in 0..i {
            if perm[j] > perm[i] {
                swaps += 1;
            }
        }
    }
    swaps % 2
}

/// The six face turns at the cubie level, read off the facelet model so that
/// the solver and the renderer can never disagree.
pub fn basic_moves() -> [CubieCube; 6] {
    core::array::from_fn(|i| {
        let mut cube = Cube::solved(3);
        cube.apply(&Turn::face(3, Face::ALL[i], 1));
        CubieCube::from_cube(&cube).expect("a face turn leaves a valid cube")
    })
}
