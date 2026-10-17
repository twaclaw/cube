//! The solver's cubie level, coordinates and search.

use cube_core::Cube;
use cube_core::geom::Face;
use cube_core::notation::{self, parse};
use cube_core::scramble::{Rng, scramble};
use cube_core::solver::cubie::{CubieCube, N_CORNERS, N_FLIP, N_SLICE_SORTED, N_TWIST, N_UD_EDGES};
use cube_core::solver::lehmer;
use cube_core::solver::search::Limits;
use cube_core::solver::{Kociemba, Solution};
use proptest::prelude::*;
use std::sync::OnceLock;

/// The tables cost a moment to build, so every test shares one solver.
fn solver() -> &'static Kociemba {
    static SOLVER: OnceLock<Kociemba> = OnceLock::new();
    SOLVER.get_or_init(Kociemba::new)
}

fn scrambled(seed: u64, len: usize) -> Cube {
    let mut cube = Cube::solved(3);
    cube.apply_moves(&scramble(3, len, &mut Rng::new(seed)));
    cube
}

/// Phase 2 may only turn U and D, or turn another face twice.
fn is_a_subgroup_move(name: &str) -> bool {
    matches!(name, "U" | "U'" | "U2" | "D" | "D'" | "D2")
        || matches!(name, "R2" | "L2" | "F2" | "B2")
}

#[test]
fn lehmer_matches_the_reference() {
    // The example from the lehmer library: [2, 0, 3, 1] is index 13.
    assert_eq!(lehmer::encode(&[2, 0, 3, 1]), 13);
    let mut perm = [0u8; 4];
    lehmer::decode(13, &mut perm);
    assert_eq!(perm, [2, 0, 3, 1]);

    assert_eq!(lehmer::encode(&[0, 1, 2, 3]), 0);
    assert_eq!(lehmer::encode(&[3, 2, 1, 0]), 23);
}

#[test]
fn lehmer_is_a_bijection() {
    for n in 1..=8usize {
        let total = lehmer::factorial(n);
        let mut seen = vec![false; total as usize];
        let mut perm = vec![0u8; n];
        for index in 0..total {
            lehmer::decode(index, &mut perm);
            let mut sorted = perm.clone();
            sorted.sort_unstable();
            assert_eq!(
                sorted,
                (0..n as u8).collect::<Vec<_>>(),
                "not a permutation"
            );
            assert_eq!(lehmer::encode(&perm), index, "round trip at {index}");
            seen[index as usize] = true;
        }
        assert!(seen.into_iter().all(|hit| hit));
    }
}

#[test]
fn the_cubie_level_mirrors_the_facelets() {
    // Slices and whole-cube turns leave the centres moved, which the cubie
    // level does not model, so those are left out on purpose.
    for word in ["U", "R'", "F2", "D", "L", "B'", "R U R' U'", "F L D"] {
        let moves = parse(3, word).unwrap();
        let mut cube = Cube::solved(3);
        cube.apply_moves(&moves);
        let cubies = CubieCube::from_cube(&cube).expect("a real state");
        assert!(cubies.is_solvable(), "{word} should be solvable");
        assert_eq!(cubies.to_cube(), cube, "{word} round trip");
    }
}

#[test]
fn coordinates_round_trip() {
    let mut cube = CubieCube::solved();
    for value in 0..N_TWIST as u16 {
        cube.set_twist(value);
        assert_eq!(cube.twist(), value);
    }
    for value in 0..N_FLIP as u16 {
        cube.set_flip(value);
        assert_eq!(cube.flip(), value);
    }
    for value in 0..N_SLICE_SORTED as u16 {
        cube.set_slice_sorted(value);
        assert_eq!(cube.slice_sorted(), value, "slice {value}");
    }
    for value in 0..N_CORNERS as u16 {
        cube.set_corners(value);
        assert_eq!(cube.corners(), value);
    }
    for value in 0..N_UD_EDGES as u16 {
        cube.set_ud_edges(value);
        assert_eq!(cube.ud_edges(), value);
    }
}

#[test]
fn a_solved_cube_sits_at_the_origin() {
    let solved = CubieCube::solved();
    assert_eq!(solved.twist(), 0);
    assert_eq!(solved.flip(), 0);
    assert_eq!(solved.slice_sorted(), 0);
    assert_eq!(solved.corners(), 0);
    assert_eq!(solved.ud_edges(), 0);
}

#[test]
fn scrambles_are_solvable_states() {
    for seed in 0..40u64 {
        let moves = scramble(3, 25, &mut Rng::new(seed));
        let mut cube = Cube::solved(3);
        cube.apply_moves(&moves);
        let cc = CubieCube::from_cube(&cube).expect("a real state");
        assert!(cc.is_solvable(), "seed {seed}");
        assert_eq!(cc.to_cube(), cube);
    }
}

#[test]
fn it_solves_a_scrambled_cube() {
    use cube_core::solver::{Kociemba, Solver};
    let start = std::time::Instant::now();
    let solver = Kociemba::new();
    let built = start.elapsed();
    println!(
        "tables: {:.1} MB in {:?}",
        solver.table_bytes() as f64 / 1e6,
        built
    );
    let mut lengths = Vec::new();
    for seed in 0..20u64 {
        let moves = scramble(3, 25, &mut Rng::new(seed));
        let mut cube = Cube::solved(3);
        cube.apply_moves(&moves);
        let at = std::time::Instant::now();
        let solution = solver
            .solve_with(&cube, Default::default())
            .expect("a solution");
        let took = at.elapsed();
        let mut check = cube.clone();
        check.apply_moves(&solution.moves());
        assert!(
            check.is_solved(),
            "seed {seed} is not solved by the solution"
        );
        lengths.push(solution.len());
        if seed < 3 {
            println!(
                "seed {seed}: {} = {} + {} moves, {} nodes, {took:?}",
                solution.len(),
                solution.phase1.len(),
                solution.phase2.len(),
                solution.nodes
            );
        }
    }
    let worst = lengths.iter().max().unwrap();
    let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
    println!("lengths: mean {mean:.1}, worst {worst}");
    assert!(*worst <= 24);
    let _ = solver.name();
}

proptest! {
    /// Exhaustive testing stops at 8! = 40320; past that only sampling reaches.
    #[test]
    fn lehmer_round_trips_past_exhaustive_reach(n in 9usize..=12, pick: u32) {
        let total = lehmer::factorial(n);
        let index = pick % total;
        let mut perm = vec![0u8; n];
        lehmer::decode(index, &mut perm);
        let mut sorted = perm.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, (0..n as u8).collect::<Vec<_>>());
        prop_assert_eq!(lehmer::encode(&perm), index);
    }

    #[test]
    fn the_cubie_level_follows_any_maneuver(seed: u64, len in 0usize..30) {
        let cube = scrambled(seed, len);
        let cubies = CubieCube::from_cube(&cube).expect("a real state");
        prop_assert!(cubies.is_solvable());
        prop_assert_eq!(cubies.to_cube(), cube);
    }

    #[test]
    fn a_state_the_colors_do_not_allow_is_refused(seed: u64) {
        // Swapping two stickers of one face leaves colors that no cube wears.
        let mut cube = scrambled(seed, 12);
        let (a, b) = (cube.get(Face::U, 0, 0), cube.get(Face::U, 0, 1));
        prop_assume!(a != b);
        cube.set(Face::U, 0, 0, b);
        cube.set(Face::U, 0, 1, a);
        let refused = match CubieCube::from_cube(&cube) {
            None => true,
            Some(cubies) => !cubies.is_solvable(),
        };
        prop_assert!(refused);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn any_scramble_is_solved(seed: u64, len in 1usize..40) {
        let cube = scrambled(seed, len);
        let solution = solver().solve_with(&cube, Limits::default()).expect("a solution");
        let mut check = cube.clone();
        check.apply_moves(&solution.moves());
        prop_assert!(check.is_solved(), "{} did not solve it", notation::format(&solution.moves()));
        prop_assert!(solution.len() <= Limits::default().max_length);
    }

    #[test]
    fn phase_one_reaches_the_subgroup(seed: u64, len in 1usize..40) {
        let cube = scrambled(seed, len);
        let solution = solver().solve_with(&cube, Limits::default()).expect("a solution");
        let mut after = cube.clone();
        after.apply_moves(&solution.phase1);
        let cubies = CubieCube::from_cube(&after).expect("a real state");
        // In the subgroup nothing is twisted or flipped and the slice edges
        // are home, which is what phase 2 is allowed to assume.
        prop_assert_eq!(cubies.twist(), 0);
        prop_assert_eq!(cubies.flip(), 0);
        prop_assert_eq!(cubies.slice_sorted() / 24, 0);
    }

    #[test]
    fn phase_two_stays_in_the_subgroup(seed: u64, len in 1usize..40) {
        let cube = scrambled(seed, len);
        let solution = solver().solve_with(&cube, Limits::default()).expect("a solution");
        for m in &solution.phase2 {
            prop_assert!(is_a_subgroup_move(&m.name), "{} is not a phase 2 move", m.name);
        }
    }
}

#[test]
fn a_solved_cube_needs_no_moves() {
    let solution = solver()
        .solve_with(&Cube::solved(3), Limits::default())
        .expect("nothing to do");
    assert_eq!(solution, Solution::default());
    assert!(solution.is_empty());
}

#[test]
fn a_cube_with_one_corner_twisted_is_refused() {
    use cube_core::solver::SolveError;
    let mut cube = Cube::solved(3);
    // Rotate the colors of one corner in place: a cube no sequence can reach.
    cube.set(Face::U, 2, 2, Face::R);
    cube.set(Face::R, 0, 0, Face::F);
    cube.set(Face::F, 0, 2, Face::U);
    assert_eq!(
        solver().solve_with(&cube, Limits::default()),
        Err(SolveError::Unreachable)
    );
}
