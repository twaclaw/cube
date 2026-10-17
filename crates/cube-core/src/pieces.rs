//! Cubies: the little cubes, the facelets they show, and how a maneuver moves
//! them around.
//!
//! A cubie is named by the slot it sits in, which is a lattice cell, so the
//! same code works for any cube size. Face letters name slots on a 3x3, where
//! `UFR` and `FR` are unambiguous.

use crate::geom::{Face, V3, facelet_at, facelet_position, layer_index};
use crate::moves::{Move, Turn};
use crate::render::Facelet;
use std::collections::{BTreeMap, BTreeSet};

/// Where a cubie sits: its cell on the lattice facelets live on.
pub type Slot = V3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Kind {
    /// One facelet.
    Center,
    /// Two.
    Edge,
    /// Three.
    Corner,
}

/// Which face a cubie is named after first: the U or D one if it has one,
/// then F or B. The rest of a corner's letters follow clockwise, so the names
/// come out as `URF`, `DLF`, `FR`, `UB`, as everyone writes them.
const NAME_FIRST: [Face; 6] = [Face::U, Face::D, Face::F, Face::B, Face::L, Face::R];

/// Every cubie that shows at least one facelet.
pub fn slots(n: u8) -> Vec<Slot> {
    let m = n as i32 - 1;
    let mut out = Vec::new();
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let outer = |v: u8| v == 0 || v == n - 1;
                if outer(i) || outer(j) || outer(k) {
                    out.push(V3::new(
                        2 * i as i32 - m,
                        2 * j as i32 - m,
                        2 * k as i32 - m,
                    ));
                }
            }
        }
    }
    out
}

/// The faces a cubie shows, in the order its name is written.
pub fn faces(n: u8, slot: Slot) -> Vec<Face> {
    let mut showing: Vec<Face> = NAME_FIRST
        .into_iter()
        .filter(|face| {
            let (axis, positive) = face.axis();
            layer_index(n, axis, slot) == if positive { n - 1 } else { 0 }
        })
        .collect();
    if showing.len() == 3 {
        // After the first, go round the corner clockwise seen from outside,
        // which is where `cross(a, b) == -c` holds.
        let (first, a, b) = (showing[0], showing[1], showing[2]);
        let clockwise = first.normal().cross(a.normal()) == b.normal().scale(-1);
        showing = if clockwise {
            vec![first, a, b]
        } else {
            vec![first, b, a]
        };
    }
    showing
}

/// The facelets a cubie shows, in name order.
pub fn facelets(n: u8, slot: Slot) -> Vec<Facelet> {
    faces(n, slot)
        .into_iter()
        .filter_map(|face| facelet_at(n, slot, face.normal()))
        .collect()
}

pub fn kind(n: u8, slot: Slot) -> Kind {
    match facelets(n, slot).len() {
        3 => Kind::Corner,
        2 => Kind::Edge,
        _ => Kind::Center,
    }
}

/// Every cubie of one kind.
pub fn of_kind(n: u8, want: Kind) -> Vec<Slot> {
    slots(n)
        .into_iter()
        .filter(|slot| kind(n, *slot) == want)
        .collect()
}

/// The faces a cubie shows, as letters: `UFR`, `FR`, `U`. On cubes bigger than
/// a 3x3 several cubies share a name.
pub fn name(n: u8, slot: Slot) -> String {
    faces(n, slot).into_iter().map(Face::letter).collect()
}

/// The cubie a name points at, in any letter order.
pub fn named(n: u8, name: &str) -> Option<Slot> {
    let want: BTreeSet<char> = name.trim().to_uppercase().chars().collect();
    slots(n)
        .into_iter()
        .find(|slot| self::name(n, *slot).chars().collect::<BTreeSet<_>>() == want)
}

/// Which cubie a facelet belongs to.
pub fn slot_of(n: u8, facelet: Facelet) -> Slot {
    facelet_position(n, facelet.0, facelet.1, facelet.2)
}

/// Where one turn sends a cubie.
pub fn turned(n: u8, turn: &Turn, slot: Slot) -> Slot {
    if turn.covers(layer_index(n, turn.axis, slot)) {
        turn.axis.rotate(slot, turn.quarters as i32)
    } else {
        slot
    }
}

/// Where a whole maneuver sends a cubie.
pub fn moved(n: u8, moves: &[Move], slot: Slot) -> Slot {
    moves.iter().fold(slot, |at, m| turned(n, &m.turn, at))
}

/// The cubies a maneuver leaves in place. They may still be turned around.
pub fn fixed(n: u8, moves: &[Move]) -> Vec<Slot> {
    slots(n)
        .into_iter()
        .filter(|slot| moved(n, moves, *slot) == *slot)
        .collect()
}

/// The cubies a maneuver carries somewhere else.
pub fn disturbed(n: u8, moves: &[Move]) -> Vec<Slot> {
    slots(n)
        .into_iter()
        .filter(|slot| moved(n, moves, *slot) != *slot)
        .collect()
}

/// The orbits of the cubies under a set of maneuvers: the groups of slots that
/// can reach each other, and so cannot be told apart by those moves.
///
/// Colouring the orbits of `U, D, R2, L2, F2, B2` is what shows at a glance
/// which cubies phase 2 of Kociemba's algorithm can and cannot move.
pub fn orbits(n: u8, generators: &[Vec<Move>]) -> Vec<Vec<Slot>> {
    let all = slots(n);
    let index: BTreeMap<Slot, usize> = all.iter().enumerate().map(|(i, s)| (*s, i)).collect();
    let mut leader: Vec<usize> = (0..all.len()).collect();

    fn find(leader: &mut [usize], mut i: usize) -> usize {
        while leader[i] != i {
            leader[i] = leader[leader[i]];
            i = leader[i];
        }
        i
    }

    for (i, slot) in all.iter().enumerate() {
        for moves in generators {
            let j = index[&moved(n, moves, *slot)];
            let (a, b) = (find(&mut leader, i), find(&mut leader, j));
            if a != b {
                leader[a] = b;
            }
        }
    }

    let mut groups: BTreeMap<usize, Vec<Slot>> = BTreeMap::new();
    for (i, slot) in all.iter().enumerate() {
        groups.entry(find(&mut leader, i)).or_default().push(*slot);
    }
    // Biggest first, so colours go to the orbits that matter.
    let mut out: Vec<Vec<Slot>> = groups.into_values().collect();
    out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
    out
}
