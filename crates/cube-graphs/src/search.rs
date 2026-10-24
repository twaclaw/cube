//! The tree an iterative deepening search walks.
//!
//! Both phases of the solver search depth first under a move budget, cutting
//! any branch where the moves already spent plus what the pruning tables
//! promise exceeds it. That is the whole algorithm, and it is a shape: a narrow
//! corridor of expanded nodes with cut branches hanging off it. This draws that
//! shape for the budget at which the search succeeds.
//!
//! Only a few of the cut branches are drawn, because eighteen dead ends per
//! node is a wall rather than a picture; the rest are counted in the parent's
//! panel.

use crate::tree::{Mark, Node, Tree};
use cube_core::Cube;
use cube_core::solver::cubie::CubieCube;
use cube_core::solver::tables::{N_MOVES, PHASE2_MOVES, Tables, all_moves, move_face, move_name};

/// Which search to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Reach the subgroup, with all eighteen moves.
    One,
    /// Finish inside it, with the subgroup's ten.
    Two,
}

impl Phase {
    fn moves(self) -> Vec<usize> {
        match self {
            Phase::One => (0..N_MOVES).collect(),
            Phase::Two => PHASE2_MOVES.to_vec(),
        }
    }

    /// The longest this phase ever is, so a search cannot run away.
    fn ceiling(self) -> usize {
        match self {
            Phase::One => 12,
            Phase::Two => 18,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Phase::One => "phase 1",
            Phase::Two => "phase 2",
        }
    }
}

/// The tree of the search for `cube`, at the first budget that succeeds.
///
/// `budget` forces one particular budget instead. `max_nodes` stops a figure
/// growing without end, and `pruned_shown` is how many cut branches to draw per
/// node before the rest are only counted.
///
/// `None` if the colours are not a state a cube can be in, or if phase 2 is
/// asked for a cube that has not reached the subgroup.
pub fn search_tree(
    tables: &Tables,
    cube: &Cube,
    phase: Phase,
    budget: Option<usize>,
    max_nodes: usize,
    pruned_shown: usize,
) -> Option<Tree> {
    let start = CubieCube::from_cube(cube)?;
    if !start.is_solvable() {
        return None;
    }
    if phase == Phase::Two && !in_subgroup(&start) {
        return None;
    }

    let turns = all_moves();
    let floor = bound(tables, phase, &start) as usize;
    let budgets: Vec<usize> = match budget {
        Some(b) => vec![b],
        None => (floor..=phase.ceiling()).collect(),
    };

    let mut last = None;
    for b in budgets {
        let mut walk = Walk {
            tables,
            turns: &turns,
            phase,
            moves: phase.moves(),
            budget: b,
            max_nodes,
            pruned_shown,
            nodes: 0,
            cut: 0,
            found: false,
        };
        let root = walk.step(start, "", 0, usize::MAX);
        let caption = format!(
            "{} under a budget of {} move{}: {} nodes drawn, {} branch{} cut, {}",
            phase.name(),
            b,
            if b == 1 { "" } else { "s" },
            root.count(),
            walk.cut,
            if walk.cut == 1 { "" } else { "es" },
            if walk.found {
                "and one way through"
            } else {
                "no way through"
            },
        );
        let tree = Tree::new(root, caption)
            .legend_as(Mark::Fresh, "searched")
            .legend_as(Mark::Pruned, "cut by the bound")
            .legend_as(Mark::Goal, "reached the goal");
        if walk.found {
            return Some(tree);
        }
        last = Some(tree);
    }
    last
}

/// What the pruning tables promise is still needed.
fn bound(tables: &Tables, phase: Phase, c: &CubieCube) -> u8 {
    match phase {
        Phase::One => tables.phase1_distance(c.twist(), c.flip(), c.slice_sorted() / 24),
        Phase::Two => tables.phase2_distance(c.corners(), c.ud_edges(), c.slice_sorted()),
    }
}

fn solved_for(phase: Phase, c: &CubieCube) -> bool {
    match phase {
        Phase::One => c.twist() == 0 && c.flip() == 0 && c.slice_sorted() / 24 == 0,
        Phase::Two => c.corners() == 0 && c.ud_edges() == 0 && c.slice_sorted() == 0,
    }
}

fn in_subgroup(c: &CubieCube) -> bool {
    c.twist() == 0 && c.flip() == 0 && c.slice_sorted() / 24 == 0
}

/// The coordinates a phase watches, as the search sees them.
fn coords(phase: Phase, c: &CubieCube) -> (String, Vec<(String, String)>) {
    match phase {
        Phase::One => (
            format!("{}, {}, {}", c.twist(), c.flip(), c.slice_sorted() / 24),
            vec![
                ("twist".into(), format!("{} of 2187", c.twist())),
                ("flip".into(), format!("{} of 2048", c.flip())),
                ("slice".into(), format!("{} of 495", c.slice_sorted() / 24)),
            ],
        ),
        Phase::Two => (
            format!("{}, {}, {}", c.corners(), c.ud_edges(), c.slice_sorted()),
            vec![
                ("corners".into(), format!("{} of 40320", c.corners())),
                ("ud_edges".into(), format!("{} of 40320", c.ud_edges())),
                ("slice_perm".into(), format!("{} of 24", c.slice_sorted())),
            ],
        ),
    }
}

struct Walk<'a> {
    tables: &'a Tables,
    turns: &'a [CubieCube; N_MOVES],
    phase: Phase,
    moves: Vec<usize>,
    budget: usize,
    max_nodes: usize,
    pruned_shown: usize,
    nodes: usize,
    cut: usize,
    found: bool,
}

impl Walk<'_> {
    fn step(&mut self, state: CubieCube, label: &str, g: usize, last_face: usize) -> Node {
        self.nodes += 1;
        let h = bound(self.tables, self.phase, &state) as usize;
        let (value, rows) = coords(self.phase, &state);
        let mut node = Node::new(label, state.to_cube(), value)
            .with("moves spent", format!("{g}"))
            .with("tables say at least", format!("{h}"))
            .with("so at best", format!("{} of {}", g + h, self.budget));
        for (key, value) in rows {
            node = node.with(key, value);
        }

        if solved_for(self.phase, &state) {
            self.found = true;
            return node
                .with(
                    "arrived",
                    if self.phase == Phase::One {
                        "in the subgroup"
                    } else {
                        "solved"
                    },
                )
                .marked(Mark::Goal);
        }
        if g + h > self.budget {
            return node.with("cut", "over the budget").marked(Mark::Pruned);
        }
        if g >= self.budget || self.nodes >= self.max_nodes {
            return node
                .with(
                    "stopped",
                    if g >= self.budget {
                        "out of budget"
                    } else {
                        "figure is full"
                    },
                )
                .marked(Mark::Pruned);
        }

        let mut shown = 0;
        let mut cut_here = 0;
        for &m in &self.moves.clone() {
            if !allowed(move_face(m), last_face) {
                continue;
            }
            let mut next = state;
            next.apply(&self.turns[m]);
            let ch = bound(self.tables, self.phase, &next) as usize;
            if g + 1 + ch > self.budget {
                // The bound cuts this branch. Draw a few, count the rest.
                self.cut += 1;
                cut_here += 1;
                if shown < self.pruned_shown {
                    shown += 1;
                    self.nodes += 1;
                    let (value, _) = coords(self.phase, &next);
                    node.children.push(
                        Node::new(move_name(m), next.to_cube(), value)
                            .with("moves spent", format!("{}", g + 1))
                            .with("tables say at least", format!("{ch}"))
                            .with("so at best", format!("{} of {}", g + 1 + ch, self.budget))
                            .with("cut", "over the budget")
                            .marked(Mark::Pruned),
                    );
                }
                continue;
            }
            let child = self.step(next, &move_name(m), g + 1, move_face(m));
            let reached = child.mark == Mark::Goal || has_goal(&child);
            node.children.push(child);
            if reached {
                break; // the first way through is the one the search keeps
            }
        }
        if cut_here > shown {
            node = node.with("branches cut", format!("{cut_here}, {shown} drawn"));
        }
        node
    }
}

fn has_goal(node: &Node) -> bool {
    node.mark == Mark::Goal || node.children.iter().any(has_goal)
}

/// The solver's own filter: never the same face twice in a row, and one fixed
/// order for the two faces of an axis, which commute.
const fn allowed(face: usize, last_face: usize) -> bool {
    last_face == usize::MAX || (face != last_face && face + 3 != last_face)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::notation::parse;

    fn cube_of(word: &str) -> Cube {
        let mut cube = Cube::solved(3);
        for m in parse(3, word).expect("a maneuver") {
            cube.apply(&m.turn);
        }
        cube
    }

    fn tables() -> Tables {
        Tables::new()
    }

    #[test]
    fn a_solved_cube_is_already_there() {
        let t = tables();
        let tree = search_tree(&t, &Cube::solved(3), Phase::One, None, 200, 2).expect("a tree");
        assert_eq!(tree.root.mark, Mark::Goal);
        assert_eq!(tree.len(), 1, "nothing to search");
    }

    #[test]
    fn phase_one_finds_its_way_and_marks_the_goal() {
        let t = tables();
        let tree =
            search_tree(&t, &cube_of("R2 U F' L"), Phase::One, None, 4000, 2).expect("a tree");
        assert!(has_goal(&tree.root), "a way into the subgroup");
        assert!(tree.len() > 1);
        // The path to the goal is as long as the budget the search settled on.
        let depth = goal_depth(&tree.root, 0).expect("the goal");
        assert!(depth <= 12);
        assert!(tree.caption.contains("one way through"));
    }

    #[test]
    fn the_drawn_cut_branches_are_capped_but_all_are_counted() {
        let t = tables();
        let tree = search_tree(&t, &cube_of("F R U2 B'"), Phase::One, None, 4000, 1).expect("tree");
        fn check(node: &Node) {
            let drawn = node
                .children
                .iter()
                .filter(|k| k.mark == Mark::Pruned)
                .count();
            assert!(drawn <= 1, "at most one cut branch drawn per node");
            if let Some(row) = node.detail.iter().find(|d| d.key == "branches cut") {
                assert!(row.value.contains("drawn"));
            }
            for kid in &node.children {
                check(kid);
            }
        }
        check(&tree.root);
    }

    #[test]
    fn phase_two_needs_a_cube_that_reached_the_subgroup() {
        let t = tables();
        assert!(search_tree(&t, &cube_of("R"), Phase::Two, None, 100, 1).is_none());
        let inside = cube_of("U R2 D' F2");
        let tree = search_tree(&t, &inside, Phase::Two, None, 4000, 1).expect("a tree");
        assert!(has_goal(&tree.root));
        for node in walk_all(&tree.root) {
            if node.label.is_empty() {
                continue;
            }
            assert!(
                PHASE2_MOVES.iter().any(|&m| move_name(m) == node.label),
                "phase 2 only turns the subgroup's moves, not {}",
                node.label
            );
        }
    }

    #[test]
    fn an_impossible_cube_has_no_tree() {
        let t = tables();
        // One corner twisted on its own: every piece is real, but the twists
        // no longer cancel, so no sequence of turns made it.
        let mut broken = Cube::solved(3);
        broken.set(cube_core::Face::U, 2, 2, cube_core::Face::R);
        broken.set(cube_core::Face::R, 0, 0, cube_core::Face::F);
        broken.set(cube_core::Face::F, 0, 2, cube_core::Face::U);
        assert!(search_tree(&t, &broken, Phase::One, None, 100, 1).is_none());
    }

    #[test]
    fn the_figure_draws_as_one_svg() {
        let t = tables();
        let tree = search_tree(&t, &cube_of("R2 U F' L"), Phase::One, None, 4000, 2).expect("tree");
        let svg = tree.svg(&crate::tree::Style::default());
        assert_eq!(svg.matches("<symbol ").count(), tree.len());
        assert!(
            svg.contains("reached the goal"),
            "the legend names the marks"
        );
    }

    fn goal_depth(node: &Node, depth: usize) -> Option<usize> {
        if node.mark == Mark::Goal {
            return Some(depth);
        }
        node.children
            .iter()
            .find_map(|kid| goal_depth(kid, depth + 1))
    }

    fn walk_all(node: &Node) -> Vec<&Node> {
        let mut out = vec![node];
        for kid in &node.children {
            out.extend(walk_all(kid));
        }
        out
    }
}
