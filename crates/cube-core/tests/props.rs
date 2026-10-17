//! Properties that must hold for any cube size, any state and any camera.

use cube_core::export;
use cube_core::geom::{Axis, Face, facelet_at, facelet_index, facelet_label, facelet_position};
use cube_core::moves::Turn;
use cube_core::palette::STANDARD;
use cube_core::render::Highlights;
use cube_core::render::cube3d::{self, Anim, CAMERA_PRESETS, Camera, Paint, Style3d};
use cube_core::render::net::{self, NetStyle};
use cube_core::scramble::{Rng, scramble};
use cube_core::{Cube, Permutation, notation};
use proptest::prelude::*;

/// Cube sizes worth exercising: 2x2 up to 6x6.
fn sizes() -> impl Strategy<Value = u8> {
    2u8..=6
}

/// Any turn of a valid layer range of an `n`-cube.
fn turns(n: u8) -> impl Strategy<Value = Turn> {
    (0usize..3, 0u8..n, 0u8..n, -3i8..=3).prop_map(move |(axis, a, b, quarters)| Turn {
        axis: Axis::ALL[axis],
        lo: a.min(b),
        hi: a.max(b),
        quarters,
    })
}

fn scrambled(n: u8, seed: u64, len: usize) -> Cube {
    let mut cube = Cube::solved(n);
    cube.apply_moves(&scramble(n, len, &mut Rng::new(seed)));
    cube
}

/// A size paired with a turn that makes sense for it.
fn size_and_turn() -> impl Strategy<Value = (u8, Turn)> {
    sizes().prop_flat_map(|n| (Just(n), turns(n)))
}

/// Every `fill="..."` in a piece of markup, sorted: what the picture looks
/// like, independently of how the paths are written.
fn fills(markup: &str) -> Vec<String> {
    let mut out: Vec<String> = markup
        .split("fill=\"")
        .skip(1)
        .filter_map(|tail| tail.split('"').next().map(str::to_string))
        .collect();
    out.sort();
    out
}

proptest! {
    #[test]
    fn four_quarter_turns_do_nothing((n, turn) in size_and_turn(), seed: u64) {
        let mut cube = scrambled(n, seed, 12);
        let base = cube.clone();
        let t = Turn { quarters: 1, ..turn };
        for _ in 0..4 {
            cube.apply(&t);
        }
        prop_assert_eq!(cube, base);
    }

    #[test]
    fn a_turn_and_its_inverse_cancel((n, turn) in size_and_turn(), seed: u64) {
        let mut cube = scrambled(n, seed, 9);
        let base = cube.clone();
        cube.apply(&turn);
        cube.apply(&turn.inverse());
        prop_assert_eq!(cube, base);
    }

    #[test]
    fn a_sequence_is_undone_by_its_reverse(n in sizes(), seed: u64, len in 0usize..30) {
        let moves = scramble(n, len, &mut Rng::new(seed));
        let mut cube = Cube::solved(n);
        cube.apply_moves(&moves);
        for m in moves.iter().rev() {
            cube.apply(&m.inverse().turn);
        }
        prop_assert!(cube.is_solved());
    }

    #[test]
    fn turning_never_creates_or_destroys_a_color(n in sizes(), seed: u64, len in 0usize..40) {
        let cube = scrambled(n, seed, len);
        let per_face = n as usize * n as usize;
        for face in Face::ALL {
            let count = cube.stickers().iter().filter(|c| **c == face).count();
            prop_assert_eq!(count, per_face);
        }
    }

    #[test]
    fn facelets_and_lattice_points_agree(n in 1u8..8, face in 0usize..6, r in 0u8..8, c in 0u8..8) {
        prop_assume!(r < n && c < n);
        let face = Face::ALL[face];
        let p = facelet_position(n, face, r, c);
        prop_assert_eq!(facelet_at(n, p, face.normal()), Some((face, r, c)));
    }

    #[test]
    fn numbering_covers_every_facelet_once(n in 1u8..8) {
        let mut seen: Vec<u32> = Vec::new();
        for face in Face::ALL {
            for r in 0..n {
                for c in 0..n {
                    if let Some(k) = facelet_label(n, face, r, c) {
                        seen.push(k);
                    }
                }
            }
        }
        let centers = u32::from(n % 2 == 1) * 6;
        let total = 6 * n as u32 * n as u32 - centers;
        seen.sort_unstable();
        prop_assert_eq!(seen, (1..=total).collect::<Vec<_>>());
    }

    #[test]
    fn indices_are_distinct(n in 1u8..8) {
        let mut seen = std::collections::BTreeSet::new();
        for face in Face::ALL {
            for r in 0..n {
                for c in 0..n {
                    prop_assert!(seen.insert(facelet_index(n, face, r, c)));
                }
            }
        }
        prop_assert_eq!(seen.len(), 6 * n as usize * n as usize);
    }

    #[test]
    fn written_moves_survive_a_round_trip(n in sizes(), seed: u64, len in 1usize..25) {
        let moves = scramble(n, len, &mut Rng::new(seed));
        let text = notation::format(&moves);
        let again = notation::parse(n, &text).expect("what we wrote, we can read");
        prop_assert_eq!(
            moves.iter().map(|m| m.turn).collect::<Vec<_>>(),
            again.iter().map(|m| m.turn).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_parser_never_panics(src in ".{0,40}") {
        let _ = notation::parse(3, &src);
    }

    #[test]
    fn scrambles_never_repeat_an_axis(n in sizes(), seed: u64) {
        let moves = scramble(n, 25, &mut Rng::new(seed));
        for pair in moves.windows(2) {
            prop_assert_ne!(pair[0].turn.axis, pair[1].turn.axis);
        }
    }

    #[test]
    fn the_cube_always_fits_the_viewbox(
        n in sizes(), seed: u64, yaw in -720.0f64..720.0, pitch in -89.0f64..89.0,
        angle in -1.6f64..1.6, zoom in 0.4f64..1.0,
    ) {
        let cube = scrambled(n, seed, 7);
        let cam = Camera { yaw, pitch, zoom };
        let st = Style3d::default();
        let anim = Anim { turn: Turn::face(n, Face::U, 1), angle };
        let facets = cube3d::facets(&cube, &cam, Some(&anim), &st, &STANDARD, &Highlights::new());
        prop_assert!(!facets.is_empty());
        for f in &facets {
            for p in f.pts {
                prop_assert!(p[0].is_finite() && p[1].is_finite());
                prop_assert!(p[0] >= 0.0 && p[0] <= st.size, "x out of the box: {}", p[0]);
                prop_assert!(p[1] >= 0.0 && p[1] <= st.size, "y out of the box: {}", p[1]);
            }
        }
    }

    #[test]
    fn a_straight_on_view_shows_exactly_one_face(n in sizes(), seed: u64, preset in 4usize..10) {
        let (_, yaw, pitch) = CAMERA_PRESETS[preset];
        let cube = scrambled(n, seed, 11);
        let cam = Camera { yaw, pitch, zoom: 1.0 };
        let facets = cube3d::facets(&cube, &cam, None, &Style3d::default(), &STANDARD,
                                    &Highlights::new());
        let stickers: Vec<_> = facets.iter()
            .filter(|f| f.paint == Paint::Sticker)
            .filter_map(|f| f.facelet)
            .collect();
        prop_assert_eq!(stickers.len(), n as usize * n as usize);
        let face = stickers[0].0;
        prop_assert!(stickers.iter().all(|(f, _, _)| *f == face));
    }

    #[test]
    fn a_corner_view_shows_three_faces(n in sizes(), seed: u64) {
        let cube = scrambled(n, seed, 11);
        let cam = Camera::default();
        let facets = cube3d::facets(&cube, &cam, None, &Style3d::default(), &STANDARD,
                                    &Highlights::new());
        let stickers = facets.iter().filter(|f| f.paint == Paint::Sticker).count();
        prop_assert_eq!(stickers, 3 * n as usize * n as usize);
    }

    #[test]
    fn the_net_draws_every_facelet(n in sizes(), seed: u64, numbers: bool) {
        let cube = scrambled(n, seed, 5);
        let st = NetStyle { numbers, ..NetStyle::default() };
        let svg = net::markup(&cube, &st, &STANDARD, &Highlights::new());
        prop_assert_eq!(svg.matches("data-facelet").count(), 6 * n as usize * n as usize);
    }

    #[test]
    fn highlights_are_drawn_once_each(seed: u64, count in 0usize..48) {
        let cube = scrambled(3, seed, 5);
        let mut highlights = Highlights::new();
        for (i, face) in Face::ALL.into_iter().cycle().take(count).enumerate() {
            highlights.insert((face, (i % 3) as u8, (i / 3 % 3) as u8));
        }
        let svg = net::markup(&cube, &NetStyle::default(), &STANDARD, &highlights);
        prop_assert_eq!(svg.matches("stroke-width=\"3.74\"").count(), highlights.len());
    }

    #[test]
    fn exports_are_well_formed(n in sizes(), seed: u64, len in 0usize..6, steps in 1usize..4) {
        let cube = scrambled(n, seed, 4);
        let moves = scramble(n, len, &mut Rng::new(seed ^ 1));
        let (cam, st) = (Camera::default(), Style3d::default());
        let tl = export::timeline(&cube, &moves, &cam, &st, &STANDARD, &Highlights::new(),
                                  steps, 420.0);
        prop_assert_eq!(tl.frames.len(), 1 + len * steps);
        let svg = export::animated_svg(&tl, None);
        prop_assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        prop_assert_eq!(svg.matches("<animate ").count(), tl.frames.len());
        let html = export::player_html(&tl, "test", "R U");
        prop_assert!(html.contains("<svg id=\"view\""));
        let strip = export::strip_svg(&cube, &moves, &cam, &st, &STANDARD, &Highlights::new(),
                                      4, Some("#fff"));
        prop_assert!(strip.starts_with("<svg") && strip.ends_with("</svg>"));
    }

    #[test]
    fn a_state_permutes_its_facelets(n in 2u8..=3, seed: u64, len in 0usize..30) {
        let cube = scrambled(n, seed, len);
        let perm = Permutation::of(&cube).expect("small cubes name a permutation");
        let mut image: Vec<u32> = perm.cycles().concat();
        let fixed = perm.len() - perm.moved();
        prop_assert_eq!(image.len(), perm.moved());
        image.sort_unstable();
        image.dedup();
        prop_assert_eq!(image.len() + fixed, perm.len());
        for cycle in perm.cycles() {
            prop_assert!(cycle.len() > 1);
            prop_assert_eq!(cycle[0], *cycle.iter().min().unwrap());
        }
    }

    #[test]
    fn a_solved_cube_is_the_identity(n in 2u8..=3) {
        let perm = Permutation::of(&Cube::solved(n)).unwrap();
        prop_assert!(perm.is_identity());
        prop_assert_eq!(perm.to_cycle_notation(), "()");
        prop_assert_eq!(perm.order(), 1);
    }

    #[test]
    fn interchangeable_stickers_name_no_permutation(n in 4u8..=6) {
        prop_assert!(Permutation::of(&Cube::solved(n)).is_none());
    }

    #[test]
    fn the_order_is_how_often_a_maneuver_repeats(seed: u64, len in 1usize..8) {
        let moves = scramble(3, len, &mut Rng::new(seed));
        let mut cube = Cube::solved(3);
        cube.apply_moves(&moves);
        let order = Permutation::of(&cube).unwrap().order();
        prop_assume!(order < 400);
        let mut repeated = Cube::solved(3);
        for _ in 0..order {
            repeated.apply_moves(&moves);
        }
        prop_assert!(repeated.is_solved(), "{} repeats should solve it", order);
    }

    #[test]
    fn the_last_animation_frame_is_the_moved_state(n in sizes(), seed: u64) {
        let moves = scramble(n, 3, &mut Rng::new(seed));
        let cube = Cube::solved(n);
        let (cam, st) = (Camera::default(), Style3d::default());
        let tl = export::timeline(&cube, &moves, &cam, &st, &STANDARD, &Highlights::new(),
                                  5, 400.0);
        let mut moved = cube.clone();
        moved.apply_moves(&moves);
        let still = cube3d::inner(&moved, &cam, None, &st, &STANDARD, &Highlights::new());
        // A full quarter turn lands the layer exactly, so the picture is the
        // moved state: same quads, same colors, only the facelet names differ.
        prop_assert_eq!(fills(&tl.frames.last().unwrap().markup), fills(&still));
    }
}
