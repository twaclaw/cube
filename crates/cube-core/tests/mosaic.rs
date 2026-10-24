//! The grid animation: a sequence that draws itself one cube per move.

use cube_core::export::mosaic::{self, Layout, Track};
use cube_core::export::{animated_svg, player_html};
use cube_core::palette::STANDARD;
use cube_core::render::{Camera, Marks, Style3d};
use cube_core::{Cube, notation};
use std::collections::BTreeSet;

const STEPS: usize = 6;

fn moves(word: &str) -> Vec<cube_core::Move> {
    notation::parse(3, word).expect("a maneuver the parser knows")
}

fn draw(tracks: &[Track], steps: usize) -> cube_core::export::Timeline {
    mosaic::timeline(
        tracks,
        &Camera::default(),
        &Style3d {
            size: 120.0,
            ..Style3d::default()
        },
        &STANDARD,
        &Marks::new(),
        &Layout::default(),
        steps,
        300.0,
    )
}

/// Every id the frames point at.
fn referenced(tl: &cube_core::export::Timeline) -> BTreeSet<String> {
    tl.frames
        .iter()
        .flat_map(|f| f.markup.split("<use href=\"#").skip(1))
        .map(|rest| rest.split('"').next().unwrap_or_default().to_string())
        .collect()
}

/// Every id the backdrop defines.
fn defined(tl: &cube_core::export::Timeline) -> BTreeSet<String> {
    tl.backdrop
        .split("<g id=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn one_slot_per_move_and_one_still_per_state() {
    let word = "R U R' U'";
    let tl = draw(&[Track::new(Cube::solved(3), moves(word))], STEPS);
    let states = tl.frames.iter().filter(|f| f.is_state).count();
    assert_eq!(tl.frames.len(), 1 + 4 * STEPS);
    assert_eq!(states, 5, "the state it starts from, then one per move");
    assert_eq!(tl.frames[0].label, "start");
    assert_eq!(tl.frames.last().unwrap().label, "U'");
}

#[test]
fn a_cube_lands_in_every_cell_as_its_move_goes_by() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U F"))], STEPS);
    // The still at the head of each slot holds one more cube than the one
    // before it: the grid fills itself as the sequence plays.
    let landed: Vec<usize> = tl
        .frames
        .iter()
        .filter(|f| f.is_state)
        .map(|f| f.markup.matches("<use href=").count())
        .collect();
    // One outline layer per frame, plus the cubes that have landed.
    assert_eq!(landed, vec![2, 3, 4, 5]);
    // Mid turn there is one more: the cube on its way into the next cell.
    let mid = &tl.frames[1];
    assert!(!mid.is_state);
    assert_eq!(mid.markup.matches("<use href=").count(), 3);
    assert!(
        mid.markup.contains("opacity="),
        "the arriving cube fades in"
    );
}

#[test]
fn states_only_when_a_move_gets_one_frame() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U F"))], 1);
    assert_eq!(tl.frames.len(), 4);
    assert!(tl.frames.iter().all(|f| f.is_state));
    assert!(
        tl.frames.iter().all(|f| !f.markup.contains("opacity=")),
        "nothing is mid turn, so nothing is fading in"
    );
}

#[test]
fn the_grid_is_as_wide_as_the_cells_it_holds() {
    let st = Style3d {
        size: 120.0,
        ..Style3d::default()
    };
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U F"))], STEPS);
    // Four cells across, one row down, with the caption strip under the cube.
    assert!(tl.width > 4.0 * st.size && tl.width < 4.4 * st.size);
    assert!(tl.height > st.size && tl.height < 1.3 * st.size);
}

#[test]
fn a_long_sequence_wraps_into_rows() {
    let word = "R U R' U' R U R' U' R U R' U'";
    let track = Track::new(Cube::solved(3), moves(word)).wrapping();
    let wide = mosaic::timeline(
        std::slice::from_ref(&track),
        &Camera::default(),
        &Style3d::default(),
        &STANDARD,
        &Marks::new(),
        &Layout {
            columns: 13,
            ..Layout::default()
        },
        1,
        300.0,
    );
    let narrow = mosaic::timeline(
        &[track],
        &Camera::default(),
        &Style3d::default(),
        &STANDARD,
        &Marks::new(),
        &Layout {
            columns: 5,
            ..Layout::default()
        },
        1,
        300.0,
    );
    assert!(narrow.width < wide.width / 2.0);
    assert!(narrow.height > wide.height * 2.0);
}

#[test]
fn every_drawing_is_written_once_and_pointed_at() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U R' U'"))], STEPS);
    // The frames are references only: no geometry is repeated in them.
    assert!(tl.frames.iter().all(|f| !f.markup.contains("<path")));
    assert!(tl.backdrop.contains("<path"));
    let referenced = referenced(&tl);
    let defined = defined(&tl);
    assert!(!referenced.is_empty());
    assert!(
        referenced.is_subset(&defined),
        "every reference resolves inside the figure"
    );
    assert_eq!(referenced, defined, "nothing is written out unused");
}

#[test]
fn two_mosaics_do_not_share_ids() {
    let one = draw(&[Track::new(Cube::solved(3), moves("R U"))], STEPS);
    let two = draw(&[Track::new(Cube::solved(3), moves("F B"))], STEPS);
    assert!(defined(&one).is_disjoint(&defined(&two)));
}

#[test]
fn a_rewound_track_undoes_its_moves() {
    let word = "R U F";
    let mut target = Cube::solved(3);
    target.apply_moves(&moves(word));
    let track = Track::rewound(target, &moves(word));
    assert!(track.back, "it fills the row leftwards");
    // Walking it reaches the state the moves set out from.
    let mut cube = track.cube.clone();
    cube.apply_moves(&track.moves);
    assert!(cube.is_solved());
    assert_eq!(
        notation::format(&track.moves),
        "F' U' R'",
        "the moves are undone last first"
    );
}

#[test]
fn the_two_halves_of_a_meet_land_on_the_same_state() {
    let (cube, _) = cube_core::scramble::scrambled(3, 8, &mut cube_core::Rng::new(7));
    let solution = moves("R U R' U' F2 L D");
    let (forward, backward) = solution.split_at(3);
    // The figure is drawn for a solution; any pair of halves exercises the
    // layout, so what is checked here is where the two walks end up.
    let tracks = mosaic::meet(&cube, forward, backward, 1);
    assert_eq!(tracks.len(), 3);

    let mut ahead = cube.clone();
    ahead.apply_moves(forward);
    let mut behind = tracks[1].cube.clone();
    behind.apply_moves(&tracks[1].moves);
    assert_eq!(ahead.to_letters(), behind.to_letters());

    // Both walks stop in the same column, one row under the other: the
    // column they meet in is as far along as the forward half is long.
    let meet = forward.len();
    assert_eq!(
        (tracks[0].row, tracks[0].col + tracks[0].moves.len()),
        (0, meet)
    );
    assert_eq!(
        (tracks[1].row, tracks[1].col - tracks[1].moves.len()),
        (1, meet)
    );
    // The third row holds the whole solution and waits for the other two.
    assert_eq!(tracks[2].moves.len(), solution.len());
    assert_eq!(tracks[2].delay, forward.len().max(backward.len()) + 1);
}

#[test]
fn a_meet_runs_until_the_combined_row_is_done() {
    let cube = Cube::solved(3);
    let solution = moves("R U R' U' F2 L D");
    let (forward, backward) = solution.split_at(3);
    let tracks = mosaic::meet(&cube, forward, backward, 1);
    let tl = draw(&tracks, STEPS);
    // The clock runs as long as the slowest track: the combined row, after
    // its wait. Every slot but the one nothing turns in is drawn in full.
    let slots = tracks[2].delay + solution.len();
    assert_eq!(tl.frames.len(), 1 + (slots - 1) * STEPS + 1);
    let waits: Vec<&str> = tl
        .frames
        .iter()
        .filter(|f| f.label == "wait")
        .map(|f| f.label.as_str())
        .collect();
    assert_eq!(waits, vec!["wait"], "one beat of stillness, drawn once");
    // The first frame shows both ends and nothing of the third row.
    let first = &tl.frames[0].markup;
    assert_eq!(first.matches("<use href=").count(), 3);
    // By the end every cell of all three rows holds a cube.
    let last = &tl.frames.last().unwrap().markup;
    let cells: usize = tracks.iter().map(Track::cells).sum();
    assert_eq!(last.matches("<use href=").count(), cells + 1);
}

#[test]
fn both_outputs_carry_the_backdrop() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U"))], STEPS);
    let svg = animated_svg(&tl, None);
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("<defs>"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains(&format!("viewBox=\"0 0 {} ", num(tl.width))));

    let html = player_html(&tl, "meet", "R U");
    assert!(html.contains("<defs>"));
    assert!(html.contains("<g id=\"view\"></g>"));
    assert!(html.contains(&format!(
        "viewBox=\"0 0 {} {}\"",
        num(tl.width),
        num(tl.height)
    )));
}

/// The number formatting the SVG writer uses.
fn num(v: f64) -> String {
    let mut s = format!("{v:.2}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}
