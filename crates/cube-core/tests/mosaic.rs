//! The grid animation: a sequence that draws itself one cube per move.

use cube_core::export::mosaic::{self, Highlight, Layout, Mark, Pace, Track, When};
use cube_core::export::{Timeline, animated_svg, player_html};
use cube_core::palette::STANDARD;
use cube_core::render::{Camera, Marks, Style3d};
use cube_core::{Cube, notation};
use std::collections::BTreeSet;

const STEPS: usize = 6;

fn moves(word: &str) -> Vec<cube_core::Move> {
    notation::parse(3, word).expect("a maneuver the parser knows")
}

fn drawn(tracks: &[Track], layout: &Layout, pace: &Pace) -> Timeline {
    mosaic::timeline(
        tracks,
        &Camera::default(),
        &Style3d {
            size: 120.0,
            ..Style3d::default()
        },
        &STANDARD,
        &Marks::new(),
        layout,
        pace,
    )
}

fn draw(tracks: &[Track], steps: usize) -> Timeline {
    drawn(
        tracks,
        &Layout::default(),
        &Pace {
            ms: 300.0,
            steps,
            ..Pace::default()
        },
    )
}

/// Every id the frames point at.
fn referenced(tl: &Timeline) -> BTreeSet<String> {
    tl.frames
        .iter()
        .flat_map(|f| f.markup.split("<use href=\"#").skip(1))
        .map(|rest| rest.split('"').next().unwrap_or_default().to_string())
        .collect()
}

/// Every id the backdrop defines.
fn defined(tl: &Timeline) -> BTreeSet<String> {
    tl.backdrop
        .split("<g id=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn one_turn_per_move_and_one_still_per_state() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U R' U'"))], STEPS);
    let states = tl.frames.iter().filter(|f| f.is_state).count();
    assert_eq!(tl.frames.len(), 1 + 4 * STEPS);
    assert_eq!(states, 5, "the state it starts from, then one per move");
    assert_eq!(tl.frames[0].label, "start");
    assert_eq!(tl.frames.last().unwrap().label, "U'");
}

#[test]
fn a_cube_lands_in_every_cell_as_its_move_goes_by() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U F"))], STEPS);
    // The still at each landing holds one more cube than the one before it:
    // the grid fills itself as the sequence plays.
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
fn a_hold_rests_on_the_state_it_lands_without_drawing_it_twice() {
    let track = Track::new(Cube::solved(3), moves("R U F"));
    let brisk = drawn(
        std::slice::from_ref(&track),
        &Layout::default(),
        &Pace {
            ms: 300.0,
            hold: Some(0.0),
            steps: STEPS,
        },
    );
    let resting = drawn(
        &[track],
        &Layout::default(),
        &Pace {
            ms: 300.0,
            hold: Some(500.0),
            steps: STEPS,
        },
    );
    // The rest is time, not frames: the still a move lands on simply stays up.
    assert_eq!(brisk.frames.len(), resting.frames.len());
    assert_eq!(resting.total_ms(), brisk.total_ms() + 4.0 * 500.0);
    let landing = resting
        .frames
        .iter()
        .find(|f| f.is_state && f.label == "R")
        .expect("the frame R lands on");
    assert!(landing.ms >= 500.0);
}

#[test]
fn a_row_can_run_at_its_own_speed() {
    let cube = Cube::solved(3);
    let quick = Track::new(cube.clone(), moves("R U F")).paced(Some(100.0), Some(0.0));
    let slow = Track::new(cube, moves("R U F"))
        .at(1, 0)
        .paced(Some(400.0), Some(0.0));
    let tl = draw(&[quick, slow], STEPS);
    // Both rows finish their three moves, and the figure lasts as long as the
    // slower one takes, plus the beat it rests on the finished grid.
    assert!(tl.total_ms() > 3.0 * 400.0);
    assert!(tl.total_ms() < 3.0 * 400.0 + 600.0);
    let last = tl.frames.last().unwrap();
    assert_eq!(
        last.markup.matches("<use href=").count(),
        9,
        "4 + 4 + ghosts"
    );
    // The quick row is done while the slow one is still turning: a third of
    // the way in it has landed everything and the other has landed one.
    let third = tl
        .frames
        .iter()
        .scan(0.0, |run, f| {
            *run += f.ms;
            Some((*run, f))
        })
        .find(|(run, _)| *run > 410.0)
        .expect("a frame after the quick row is done")
        .1;
    assert_eq!(
        third.markup.matches("<use href=").count(),
        7,
        "4 + 2 + ghosts"
    );
}

#[test]
fn moves_can_be_labelled_and_the_numbering_dropped() {
    let track = Track::new(Cube::solved(3), moves("R U F")).labelled(vec![
        "first".to_string(),
        String::new(),
        "last".to_string(),
    ]);
    let tl = draw(std::slice::from_ref(&track), 1);
    let drawn = tl.backdrop.clone();
    assert!(drawn.contains(">1. first<"));
    assert!(
        drawn.contains(">2. U<"),
        "a blank label keeps the move's name"
    );
    assert!(drawn.contains(">3. last<"));

    let plain = drawn_without_numbers(&track);
    assert!(plain.contains(">first<") && !plain.contains(">1. first<"));
    assert!(plain.contains(">U<"));
}

fn drawn_without_numbers(track: &Track) -> String {
    drawn(
        std::slice::from_ref(track),
        &Layout {
            numbered: false,
            ..Layout::default()
        },
        &Pace {
            steps: 1,
            ..Pace::default()
        },
    )
    .backdrop
}

#[test]
fn a_row_can_be_named_in_the_margin() {
    let plain = draw(&[Track::new(Cube::solved(3), moves("R U"))], 1);
    let titled = draw(
        &[Track::new(Cube::solved(3), moves("R U")).titled("forwards")],
        1,
    );
    assert!(titled.backdrop.contains(">forwards<"));
    assert!(
        titled.width > plain.width,
        "the name gets a margin of its own"
    );
    // The name is drawn once, behind the frames, not in every one of them.
    assert!(titled.frames.iter().all(|f| !f.markup.contains("forwards")));
}

#[test]
fn the_grid_is_as_wide_as_the_cells_it_holds() {
    let size = 120.0;
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U F"))], STEPS);
    // Four cells across, one row down, with the caption strip under the cube.
    assert!(tl.width > 4.0 * size && tl.width < 4.4 * size);
    assert!(tl.height > size && tl.height < 1.3 * size);
}

#[test]
fn a_long_sequence_wraps_into_rows() {
    let word = "R U R' U' R U R' U' R U R' U'";
    let track = Track::new(Cube::solved(3), moves(word)).wrapping();
    let pace = Pace {
        steps: 1,
        ..Pace::default()
    };
    let wide = drawn(
        std::slice::from_ref(&track),
        &Layout {
            columns: 13,
            ..Layout::default()
        },
        &pace,
    );
    let narrow = drawn(
        &[track],
        &Layout {
            columns: 5,
            ..Layout::default()
        },
        &pace,
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
    let tracks = mosaic::meet(&cube, forward, backward, 1.0);
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
    assert_eq!(
        tracks[2].delay,
        forward.len().max(backward.len()) as f64 + 1.0
    );
    // And every row says what it is.
    let titles: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["forwards", "backwards", "together"]);
}

#[test]
fn a_meet_runs_until_the_combined_row_is_done() {
    let cube = Cube::solved(3);
    let solution = moves("R U R' U' F2 L D");
    let (forward, backward) = solution.split_at(3);
    let tracks = mosaic::meet(&cube, forward, backward, 1.0);
    let tl = draw(&tracks, STEPS);
    // The first frame shows both ends and nothing of the third row.
    let first = &tl.frames[0].markup;
    assert_eq!(first.matches("<use href=").count(), 3);
    // By the end every cell of all three rows holds a cube, over the outline
    // layer and under the mark on the pair they met on.
    let last = &tl.frames.last().unwrap().markup;
    let cells: usize = tracks.iter().map(Track::cells).sum();
    assert_eq!(last.matches("<use href=").count(), cells + 2);
    // The two halves turn together, so a mid turn frame names both.
    assert!(
        tl.frames.iter().any(|f| f.label.split(' ').count() == 2),
        "a frame of two rows turning at once is labelled with both"
    );
}

/// The layers a frame places where they were drawn: the outline of the cells
/// still waiting, and the marks. A cube is placed with an `x`, these are not.
fn layers(markup: &str) -> Vec<&str> {
    markup
        .split("<use href=\"#")
        .skip(1)
        .map(|rest| rest.split("/>").next().unwrap_or_default())
        .filter(|tag| !tag.contains(" x=\""))
        .collect()
}

/// The mark layers of a frame: the ones that fade, the outline never does.
fn mark_layers(markup: &str) -> usize {
    layers(markup)
        .iter()
        .filter(|tag| tag.contains("opacity="))
        .count()
}

/// How far up the marks are in a frame, 0 when it has none.
fn marked(markup: &str) -> f64 {
    layers(markup)
        .iter()
        .filter_map(|tag| tag.split("opacity=\"").nth(1))
        .filter_map(|rest| rest.split('"').next())
        .filter_map(|v| v.parse::<f64>().ok())
        .fold(0.0, f64::max)
}

/// Each frame with the time it comes up at.
fn clocked(tl: &Timeline) -> Vec<(f64, &cube_core::export::Frame)> {
    let mut at = 0.0;
    tl.frames
        .iter()
        .map(|f| {
            let was = at;
            at += f.ms;
            (was, f)
        })
        .collect()
}

#[test]
fn nothing_is_marked_unless_a_track_asks() {
    let tl = draw(&[Track::new(Cube::solved(3), moves("R U"))], STEPS);
    assert!(tl.frames.iter().all(|f| marked(&f.markup) == 0.0));
}

#[test]
fn the_pair_a_search_meets_on_is_marked_the_moment_it_meets() {
    let cube = Cube::solved(3);
    let solution = moves("R U R' U' F2 L");
    // Halves of different lengths, so the two walks arrive at different
    // times and only the later of them is the meeting.
    let (forward, backward) = solution.split_at(4);
    let tracks = mosaic::meet(&cube, forward, backward, 1.0);
    assert_eq!(tracks[0].highlight, vec![4]);
    assert_eq!(tracks[1].highlight, vec![2]);
    assert!(tracks[2].highlight.is_empty());

    // A turn every 300ms and no resting, so the forward row lands its fourth
    // move at 1200ms and the backward row its second at 600ms.
    let tl = drawn(
        &tracks,
        &Layout::default(),
        &Pace {
            ms: 300.0,
            hold: Some(0.0),
            steps: STEPS,
        },
    );
    let clocked = clocked(&tl);
    let (first, _) = clocked
        .iter()
        .find(|(_, f)| marked(&f.markup) > 0.0)
        .expect("a frame with the mark on it");
    assert!(
        (1200.0..1500.0).contains(first),
        "marked from {first}ms, not from the later of the two arrivals"
    );
    // Once up it stays up, and it is full up by the end.
    let on = clocked
        .iter()
        .position(|(_, f)| marked(&f.markup) > 0.0)
        .expect("a frame with the mark on it");
    assert!(clocked[on..].iter().all(|(_, f)| marked(&f.markup) > 0.0));
    assert_eq!(marked(&tl.frames.last().unwrap().markup), 1.0);
    // Both rows are marked at one moment, so they share one layer.
    assert_eq!(mark_layers(&tl.frames.last().unwrap().markup), 1);
}

#[test]
fn a_mark_can_wait_for_the_whole_figure() {
    let track = Track::new(Cube::solved(3), moves("R U F")).marking_last();
    let pace = Pace {
        ms: 300.0,
        hold: Some(0.0),
        steps: STEPS,
    };
    let at_end = drawn(
        std::slice::from_ref(&track),
        &Layout {
            highlight: Highlight {
                when: When::End,
                ..Highlight::default()
            },
            ..Layout::default()
        },
        &pace,
    );
    let plain = drawn(
        &[Track::new(Cube::solved(3), moves("R U F"))],
        &Layout::default(),
        &pace,
    );
    // The figure runs on past the last cube, to bring the mark in.
    assert!(at_end.total_ms() > plain.total_ms());
    assert_eq!(at_end.frames.len(), plain.frames.len() + STEPS);
    assert_eq!(marked(&at_end.frames.last().unwrap().markup), 1.0);
}

#[test]
fn each_mark_can_wait_for_its_own_cube() {
    let cube = Cube::solved(3);
    let track = Track::new(cube, moves("R U F")).marking(vec![1, 3]);
    let tl = drawn(
        &[track],
        &Layout {
            highlight: Highlight {
                when: When::Landed,
                ..Highlight::default()
            },
            ..Layout::default()
        },
        &Pace {
            ms: 300.0,
            hold: Some(0.0),
            steps: STEPS,
        },
    );
    // Two moments, so two layers, and the first is up long before the second.
    let last = tl.frames.last().unwrap();
    assert_eq!(mark_layers(&last.markup), 2, "one layer per moment");
    let marked_at: Vec<f64> = clocked(&tl)
        .iter()
        .filter(|(_, f)| marked(&f.markup) > 0.0)
        .map(|(t, _)| *t)
        .collect();
    assert!(
        marked_at[0] < 600.0,
        "the first cube is pointed at as it lands"
    );
}

#[test]
fn a_dot_can_stand_in_for_the_frame() {
    let track = Track::new(Cube::solved(3), moves("R U")).marking_last();
    let dotted = drawn(
        std::slice::from_ref(&track),
        &Layout {
            highlight: Highlight {
                mark: Mark::Dot,
                ..Highlight::default()
            },
            ..Layout::default()
        },
        &Pace::default(),
    );
    let framed = drawn(&[track], &Layout::default(), &Pace::default());
    assert!(dotted.backdrop.contains("<circle"));
    assert!(!framed.backdrop.contains("<circle"));
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
