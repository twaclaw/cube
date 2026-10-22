//! Typing a cube in by hand.

use cube_core::entry::Editor;
use cube_core::geom::Face;
use cube_core::render::Facelet;
use cube_core::scramble::{Rng, scrambled};
use cube_core::{Cube, solver};
use proptest::prelude::*;

/// Every facelet, in numbering order: the order someone reading a net would
/// work through them.
fn facelets() -> Vec<Facelet> {
    (0..54)
        .map(|i| (Face::ALL[i / 9], ((i % 9) / 3) as u8, (i % 3) as u8))
        .collect()
}

/// Type `cube` into a blank editor, letting it settle after every facelet,
/// and count the ones that actually had to be entered.
fn type_in(cube: &Cube) -> (usize, Editor) {
    let mut editor = Editor::new();
    let mut entered = 0;
    for facelet in facelets() {
        if editor.get(facelet).is_some() {
            continue; // already follows from what came before
        }
        editor.set(facelet, Some(cube.get(facelet.0, facelet.1, facelet.2)));
        entered += 1;
        editor.settle();
    }
    (entered, editor)
}

#[test]
fn a_blank_editor_knows_only_its_centers() {
    let editor = Editor::new();
    assert_eq!(editor.undecided(), 48);
    for face in Face::ALL {
        assert_eq!(editor.get((face, 1, 1)), Some(face));
        // A center names a color; it is not a sticker that can be moved.
        assert_eq!(editor.allowed((face, 1, 1)), vec![face]);
    }
}

#[test]
fn a_blank_editor_offers_every_color_off_centre() {
    // Nothing is ruled out before anything is entered.
    let editor = Editor::new();
    assert_eq!(editor.allowed((Face::U, 0, 0)).len(), 6);
    assert_eq!(editor.allowed((Face::U, 0, 1)).len(), 6);
}

#[test]
fn a_corner_pins_itself_from_two_stickers() {
    // URF read as white then red leaves only one corner it can be, so the
    // third sticker follows without being asked for.
    let mut editor = Editor::new();
    editor.set((Face::U, 2, 2), Some(Face::U));
    editor.set((Face::R, 0, 0), Some(Face::R));
    editor.settle();
    assert_eq!(editor.get((Face::F, 0, 2)), Some(Face::F));
}

#[test]
fn an_edge_needs_both_its_stickers() {
    // One sticker leaves four edges open, so nothing follows yet.
    let mut editor = Editor::new();
    editor.set((Face::U, 2, 1), Some(Face::U));
    editor.settle();
    assert_eq!(editor.get((Face::F, 0, 1)), None);
    let open = editor.allowed((Face::F, 0, 1));
    assert_eq!(open.len(), 4, "white pairs with four colors, not {open:?}");
    assert!(!open.contains(&Face::U) && !open.contains(&Face::D));
}

#[test]
fn typing_a_cube_takes_at_most_36_facelets() {
    // The count that matters: 54 stickers, 48 of them off-centre, but only
    // this many carry anything the rest does not already imply.
    let mut worst = 0;
    let mut total = 0;
    let runs = 200u64;
    for seed in 0..runs {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let (entered, editor) = type_in(&cube);
        assert_eq!(editor.cube().as_ref(), Some(&cube), "seed {seed}");
        assert!(entered <= 36, "seed {seed} needed {entered}");
        worst = worst.max(entered);
        total += entered;
    }
    println!(
        "entered: worst {worst}, mean {:.1}, of 48 off-centre facelets",
        total as f64 / runs as f64
    );
}

#[test]
fn the_solved_cube_types_in_too() {
    let (entered, editor) = type_in(&Cube::solved(3));
    assert_eq!(editor.cube(), Some(Cube::solved(3)));
    assert!(entered <= 36);
}

proptest! {
    /// Whatever the editor fills in by itself is what was really there.
    #[test]
    fn settling_never_invents_a_sticker(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let mut editor = Editor::new();
        for facelet in facelets() {
            if editor.get(facelet).is_none() {
                editor.set(facelet, Some(cube.get(facelet.0, facelet.1, facelet.2)));
                editor.settle();
            }
            // Everything decided so far, asked for or not, has to be right.
            for check in facelets() {
                if let Some(color) = editor.get(check) {
                    prop_assert_eq!(color, cube.get(check.0, check.1, check.2));
                }
            }
        }
    }

    /// The real color is never the one the menu leaves out.
    #[test]
    fn the_truth_is_always_on_offer(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let mut editor = Editor::new();
        for facelet in facelets() {
            if editor.get(facelet).is_some() {
                continue;
            }
            let truth = cube.get(facelet.0, facelet.1, facelet.2);
            prop_assert!(editor.allowed(facelet).contains(&truth));
            editor.set(facelet, Some(truth));
            editor.settle();
        }
    }

    /// Anything the menu offers can still be finished into a real cube.
    #[test]
    fn every_offer_leads_somewhere(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let mut editor = Editor::new();
        let mut rng = Rng::new(seed ^ 0xabc);
        // Part-fill, then check each offer on a facelet still open.
        for facelet in facelets().into_iter().take(20) {
            if editor.get(facelet).is_none() {
                editor.set(facelet, Some(cube.get(facelet.0, facelet.1, facelet.2)));
                editor.settle();
            }
        }
        let open = editor.open();
        if open.is_empty() {
            return Ok(());
        }
        let facelet = open[rng.below(open.len() as u32) as usize];
        for color in editor.allowed(facelet) {
            let mut trial = editor.clone();
            trial.set(facelet, Some(color));
            prop_assert!(trial.is_possible(), "{color:?} was offered but is a dead end");
        }
    }

    /// A finished editor is a state the solver accepts: the piece checks are
    /// built into what it would offer, so parity is all that is left.
    #[test]
    fn a_typed_cube_is_a_real_state(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let (_, editor) = type_in(&cube);
        let built = editor.cube().expect("every facelet decided");
        prop_assert!(solver::check(&built).is_ok());
    }

    /// Starting from a state on screen gives that state back.
    #[test]
    fn editing_starts_from_what_is_there(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let editor = Editor::from_cube(&cube);
        prop_assert_eq!(editor.undecided(), 0);
        prop_assert_eq!(editor.cube(), Some(cube));
    }
}

#[test]
fn a_sticker_can_be_taken_back() {
    // Clearing one sticker of a finished cube achieves nothing on its own:
    // its neighbours still name the piece, so it can only go back as it was.
    // Clearing the cubie frees the piece.
    let (cube, _) = scrambled(3, 25, &mut Rng::new(3));
    let mut editor = Editor::from_cube(&cube);
    let corner = (Face::U, 0, 0);

    editor.set(corner, None);
    assert_eq!(editor.undecided(), 1);
    editor.settle();
    assert_eq!(
        editor.get(corner),
        Some(cube.get(Face::U, 0, 0)),
        "one sticker on its own is pinned by its neighbours"
    );

    editor.clear_cubie(corner);
    assert_eq!(editor.undecided(), 3, "the whole corner goes");
}

#[test]
fn a_finished_cube_can_still_be_corrected() {
    // Every other piece placed settles which piece belongs here, but not the
    // way round it sits: a corner offers the three colors of that piece, an
    // edge the two of its own. Freeing a second piece is what lets a
    // different piece come instead.
    let (cube, _) = scrambled(3, 25, &mut Rng::new(5));
    let editor = Editor::from_cube(&cube);
    let corner = (Face::U, 0, 0);
    let edge = (Face::U, 0, 1);

    let twists = editor.allowed(corner);
    assert_eq!(
        twists.len(),
        3,
        "a corner can be turned in place: {twists:?}"
    );
    assert!(twists.contains(&cube.get(Face::U, 0, 0)));

    let flips = editor.allowed(edge);
    assert_eq!(flips.len(), 2, "an edge can be flipped in place: {flips:?}");
    assert!(flips.contains(&cube.get(Face::U, 0, 1)));

    let mut freed = editor.clone();
    freed.clear_cubie((Face::U, 0, 2));
    assert!(
        freed.allowed(corner).len() > 3,
        "with a second corner free, another piece can come here"
    );
}

#[test]
fn choosing_frees_the_cubie_it_lands_on() {
    let (cube, _) = scrambled(3, 25, &mut Rng::new(9));
    let mut editor = Editor::from_cube(&cube);
    let corner = (Face::U, 2, 2);
    let had = editor.get(corner).expect("a complete cube");
    let other = Face::ALL
        .into_iter()
        .find(|f| *f != had)
        .expect("another color");

    editor.choose(corner, other);
    assert_eq!(editor.get(corner), Some(other));
    // Its two neighbours went with it; nothing else did.
    assert_eq!(editor.undecided(), 2);
    assert_eq!(editor.cube(), None);
}

proptest! {
    /// Going back to a sticker offers only changes that can be carried out:
    /// each one, taken, leaves a cube that can still be finished.
    #[test]
    fn every_change_offered_can_be_made(seed in any::<u64>()) {
        let (cube, _) = scrambled(3, 25, &mut Rng::new(seed));
        let editor = Editor::from_cube(&cube);
        for facelet in [(Face::U, 0, 0), (Face::U, 0, 1), (Face::D, 2, 2), (Face::F, 1, 0)] {
            for color in editor.allowed(facelet) {
                let mut trial = editor.clone();
                trial.choose(facelet, color);
                prop_assert!(
                    trial.is_possible(),
                    "{color:?} offered at {facelet:?} but leaves nothing finishable"
                );
                prop_assert_eq!(trial.get(facelet), Some(color));
            }
        }
    }
}
