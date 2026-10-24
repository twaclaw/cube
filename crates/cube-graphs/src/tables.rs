//! The tree of a table being filled.
//!
//! A pruning table is written by breadth-first search outward from the solved
//! cube: turn every face of everything found last time, and keep the results
//! whose coordinate has not been seen. This draws that, with one node per
//! arrival. A node whose value was already in the table is marked
//! [`Mark::Seen`] and not expanded, which is exactly what the sweep does and
//! the reason a table of a few thousand numbers can stand in for a tree that
//! branches eighteen ways forever.

use crate::tree::{Mark, Node, Tree};
use cube_core::solver::cubie::CubieCube;
use cube_core::solver::tables::{N_MOVES, PHASE2_MOVES, all_moves, move_name};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

/// Corner slots, in the order the solver keeps them.
const CORNERS: [&str; 8] = ["URF", "UFL", "ULB", "UBR", "DFR", "DLF", "DBL", "DRB"];
/// Edge slots, the eight U/D ones and then the four in the middle layer.
const EDGES: [&str; 12] = [
    "UR", "UF", "UL", "UB", "DR", "DF", "DL", "DB", "FR", "FL", "BL", "BR",
];

/// Which of the solver's six coordinates a tree is about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Coord {
    /// Corner orientations, `3^7` of them.
    Twist,
    /// Edge orientations, `2^11`.
    Flip,
    /// Which four slots hold the slice edges, `C(12,4)`.
    Slice,
    /// Corner permutation, `8!`.
    Corners,
    /// Permutation of the eight U/D edges, `8!`. Phase 2 only.
    UdEdges,
    /// The order of the four slice edges, `4!`. Phase 2 only.
    SlicePerm,
}

impl Coord {
    /// The name the solver's table goes by.
    pub fn name(self) -> &'static str {
        match self {
            Coord::Twist => "twist",
            Coord::Flip => "flip",
            Coord::Slice => "slice",
            Coord::Corners => "corners",
            Coord::UdEdges => "ud_edges",
            Coord::SlicePerm => "slice_perm",
        }
    }

    /// How many values it has.
    pub fn size(self) -> u32 {
        match self {
            Coord::Twist => 2187,
            Coord::Flip => 2048,
            Coord::Slice => 495,
            Coord::Corners | Coord::UdEdges => 40320,
            Coord::SlicePerm => 24,
        }
    }

    /// The value this state has, which is the table slot it indexes.
    pub fn of(self, c: &CubieCube) -> u32 {
        match self {
            Coord::Twist => u32::from(c.twist()),
            Coord::Flip => u32::from(c.flip()),
            Coord::Slice => u32::from(c.slice_sorted() / 24),
            Coord::Corners => u32::from(c.corners()),
            Coord::UdEdges => u32::from(c.ud_edges()),
            Coord::SlicePerm => u32::from(c.slice_sorted() % 24),
        }
    }

    /// The moves the table is built with: all eighteen, or the ten of the
    /// subgroup for the two coordinates that only mean anything inside it.
    pub fn moves(self) -> Vec<usize> {
        match self {
            Coord::UdEdges | Coord::SlicePerm => PHASE2_MOVES.to_vec(),
            _ => (0..N_MOVES).collect(),
        }
    }

    /// Whether a face turn can take this coordinate out of its own world.
    fn phase2_only(self) -> bool {
        matches!(self, Coord::UdEdges | Coord::SlicePerm)
    }

    /// What the value is made of, written out for the node's panel.
    fn detail(self, c: &CubieCube) -> Vec<(String, String)> {
        match self {
            Coord::Twist => vec![
                ("twists, clockwise".into(), digits(&c.co, "")),
                ("in base three".into(), base_three(&c.co[..7])),
            ],
            Coord::Flip => vec![
                ("flips".into(), digits(&c.eo, "")),
                ("flipped edges".into(), named(&c.eo, &EDGES)),
            ],
            Coord::Slice => vec![
                ("slice edges in".into(), slots_holding(c)),
                ("of the twelve slots".into(), "4 chosen".into()),
            ],
            Coord::Corners => vec![
                (
                    "corners, in slot order".into(),
                    named_order(&c.cp, &CORNERS),
                ),
                ("lehmer digits".into(), digits(&lehmer(&c.cp), " ")),
            ],
            Coord::UdEdges => vec![
                (
                    "U/D edges, in slot order".into(),
                    named_order(&c.ep[..8], &EDGES),
                ),
                ("lehmer digits".into(), digits(&lehmer(&c.ep[..8]), " ")),
            ],
            Coord::SlicePerm => vec![
                (
                    "belt, in slot order".into(),
                    named_order(&slice_order(c), &EDGES[8..]),
                ),
                (
                    "lehmer digits".into(),
                    digits(&lehmer(&slice_order(c)), " "),
                ),
            ],
        }
    }
}

/// The tree a sweep of `coord` walks, to `depth` levels, turning `moves`.
///
/// Pass an empty `moves` for the coordinate's own move set, which is all
/// eighteen face turns except for the two phase 2 coordinates. Keep the move
/// list short: a figure with three moves and four levels already has forty
/// nodes.
pub fn table_tree(coord: Coord, moves: &[usize], depth: usize) -> Tree {
    let mut turns: Vec<usize> = if moves.is_empty() {
        coord.moves()
    } else {
        moves.to_vec()
    };
    if coord.phase2_only() {
        turns.retain(|m| PHASE2_MOVES.contains(m));
    }
    let table = all_moves();

    // How the sweep fills the table: breadth first, by value, so the depth a
    // value is first reached at is the distance the table stores for it.
    let start = CubieCube::solved();
    let mut first: BTreeMap<u32, usize> = BTreeMap::new();
    first.insert(coord.of(&start), 0);
    let mut frontier = vec![start];
    for level in 1..=depth {
        let mut next = Vec::new();
        for state in &frontier {
            for &m in &turns {
                let mut moved = *state;
                moved.apply(&table[m]);
                // or_insert, never insert: the distance a value is first
                // reached at is the one the table keeps.
                if let Entry::Vacant(slot) = first.entry(coord.of(&moved)) {
                    slot.insert(level);
                    next.push(moved);
                }
            }
        }
        frontier = next;
    }

    let sweep = Sweep {
        coord,
        table: &table,
        turns: &turns,
        first: &first,
        depth,
    };
    let root = sweep.grow(start, "", 0);
    let moved = turns
        .iter()
        .map(|&m| move_name(m))
        .collect::<Vec<_>>()
        .join(", ");
    Tree::new(
        root,
        format!(
            "the {} table filling up: {} levels of {}, {} of {} values reached",
            coord.name(),
            depth,
            moved,
            first.len(),
            coord.size(),
        ),
    )
}

/// What one sweep of a coordinate needs to know while it grows the tree.
struct Sweep<'a> {
    coord: Coord,
    table: &'a [CubieCube; N_MOVES],
    turns: &'a [usize],
    /// The distance each value is first reached at: what the table stores.
    first: &'a BTreeMap<u32, usize>,
    depth: usize,
}

impl Sweep<'_> {
    /// One node and, unless its value is already in the table, its children.
    fn grow(&self, state: CubieCube, label: &str, level: usize) -> Node {
        let value = self.coord.of(&state);
        let born = self.first.get(&value).copied().unwrap_or(level);
        let seen = born < level;
        let mut node = Node::new(label, state.to_cube(), format!("{value}"))
            .with(
                format!("{} value", self.coord.name()),
                format!("{value} of {}", self.coord.size()),
            )
            .with("moves from solved", format!("{level}"));
        node = if seen {
            node.with("the table has it", format!("at distance {born}"))
                .marked(Mark::Seen)
        } else {
            node.with("the table writes", format!("distance {level}"))
        };
        for (key, value) in self.coord.detail(&state) {
            node = node.with(key, value);
        }
        if level < self.depth && !seen {
            node.children = self
                .turns
                .iter()
                .map(|&m| {
                    let mut moved = state;
                    moved.apply(&self.table[m]);
                    self.grow(moved, &move_name(m), level + 1)
                })
                .collect();
        }
        node
    }
}

fn digits(v: &[u8], sep: &str) -> String {
    v.iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

fn base_three(v: &[u8]) -> String {
    let n = v.iter().fold(0u32, |acc, &d| acc * 3 + u32::from(d));
    format!("{n}")
}

/// The pieces that are flipped or twisted, by slot name.
fn named(v: &[u8], names: &[&str]) -> String {
    let out: Vec<&str> = v
        .iter()
        .enumerate()
        .filter(|&(_, &d)| d != 0)
        .map(|(i, _)| names[i])
        .collect();
    if out.is_empty() {
        "none".into()
    } else {
        out.join(" ")
    }
}

/// Which piece sits in each slot, by name.
fn named_order(v: &[u8], names: &[&str]) -> String {
    v.iter()
        .map(|&p| names[p as usize % names.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

/// The slots holding the four slice edges.
fn slots_holding(c: &CubieCube) -> String {
    let out: Vec<&str> = (0..12)
        .filter(|&i| c.ep[i] >= 8)
        .map(|i| EDGES[i])
        .collect();
    out.join(" ")
}

/// The four slice edges in the order they sit in the belt.
fn slice_order(c: &CubieCube) -> Vec<u8> {
    let mut out = Vec::with_capacity(4);
    for slot in 0..12 {
        if c.ep[slot] >= 8 {
            out.push(c.ep[slot] - 8);
        }
    }
    while out.len() < 4 {
        out.push(0);
    }
    out
}

/// The Lehmer code of a permutation: for each place, how many smaller values
/// come after it.
fn lehmer(perm: &[u8]) -> Vec<u8> {
    (0..perm.len())
        .map(|i| {
            perm[i + 1..]
                .iter()
                .filter(|&&later| later < perm[i])
                .count() as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Mark;

    fn marks(node: &Node, into: &mut Vec<Mark>) {
        into.push(node.mark);
        for kid in &node.children {
            marks(kid, into);
        }
    }

    #[test]
    fn the_root_is_the_solved_cube_at_value_zero() {
        let tree = table_tree(Coord::Corners, &[0, 4], 1);
        assert_eq!(tree.root.value, "0");
        assert!(tree.root.cube.is_solved());
        assert_eq!(tree.root.children.len(), 2);
    }

    #[test]
    fn a_half_turn_twice_comes_back_to_the_table_slot_it_left() {
        // U is move 0, R2 is move 4. R2 R2 is the identity, so the second R2
        // arrives at a value the table already holds.
        let tree = table_tree(Coord::Corners, &[0, 4], 2);
        let r2 = &tree.root.children[1];
        assert_eq!(r2.label, "R2");
        let back = &r2.children[1];
        assert_eq!(back.value, "0");
        assert_eq!(back.mark, Mark::Seen);
        assert!(back.children.is_empty(), "a seen value is not expanded");
    }

    #[test]
    fn node_values_match_the_coordinate_of_the_state_drawn() {
        let tree = table_tree(Coord::Flip, &[6, 0], 3);
        fn check(node: &Node) {
            let cubie = CubieCube::from_cube(&node.cube).expect("a real state");
            assert_eq!(node.value, format!("{}", Coord::Flip.of(&cubie)));
            for kid in &node.children {
                check(kid);
            }
        }
        check(&tree.root);
    }

    #[test]
    fn the_phase_two_coordinates_refuse_moves_that_leave_the_subgroup() {
        // 3 is R, a quarter turn, which is not a move of the subgroup.
        let tree = table_tree(Coord::UdEdges, &[3, 0], 1);
        for kid in &tree.root.children {
            assert_ne!(kid.label, "R");
        }
    }

    #[test]
    fn every_node_carries_its_numbers() {
        let tree = table_tree(Coord::Twist, &[0, 6], 2);
        fn check(node: &Node) {
            assert!(node.detail.len() >= 4, "value, distance and the digits");
            assert!(node.detail.iter().any(|d| d.key.contains("twist")));
            for kid in &node.children {
                check(kid);
            }
        }
        check(&tree.root);
    }

    #[test]
    fn a_deeper_tree_still_draws_one_svg() {
        let tree = table_tree(Coord::Corners, &[0, 4], 4);
        let svg = tree.svg(&crate::tree::Style::default());
        assert_eq!(svg.matches("<symbol ").count(), tree.len());
        assert!(
            tree.len() > 15,
            "four levels is more than the old layout fit"
        );
        let mut found = Vec::new();
        marks(&tree.root, &mut found);
        assert!(found.contains(&Mark::Seen), "the sweep meets itself");
    }

    #[test]
    fn lehmer_counts_inversions_to_the_right() {
        assert_eq!(lehmer(&[0, 1, 2, 3]), vec![0, 0, 0, 0]);
        assert_eq!(lehmer(&[3, 2, 1, 0]), vec![3, 2, 1, 0]);
        assert_eq!(
            lehmer(&[0, 5, 3, 1, 7, 2, 6, 4]),
            vec![0, 4, 2, 0, 3, 0, 1, 0]
        );
    }
}
