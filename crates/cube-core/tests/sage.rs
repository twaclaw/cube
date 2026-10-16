//! The move permutations, checked against SageMath's cube group.

#[path = "data/sage_data.rs"]
mod sage_data;

use cube_core::geom::{Face, facelet_label};
use cube_core::{Cube, parse};

/// Facelet colors in Singmaster order, centers left out, as Sage prints them.
fn singmaster(cube: &Cube) -> String {
    let n = cube.n();
    let mut out = Vec::new();
    for face in Face::ALL {
        for row in 0..n {
            for col in 0..n {
                if facelet_label(n, face, row, col).is_some() {
                    out.push((
                        facelet_label(n, face, row, col).unwrap(),
                        cube.get(face, row, col).letter().to_ascii_lowercase(),
                    ));
                }
            }
        }
    }
    out.sort_by_key(|(k, _)| *k);
    out.into_iter().map(|(_, c)| c).collect()
}

#[test]
fn matches_sage() {
    for (word, expected) in sage_data::CASES {
        let mut cube = Cube::solved(3);
        cube.apply_moves(&parse(3, word).expect("the test words parse"));
        assert_eq!(&singmaster(&cube), expected, "maneuver {word}");
    }
}

#[test]
fn numbering_matches_the_notebook() {
    // Spot checks of the layout printed by `rubik.display2d("")`.
    assert_eq!(facelet_label(3, Face::U, 0, 0), Some(1));
    assert_eq!(facelet_label(3, Face::U, 1, 1), None); // the center has no number
    assert_eq!(facelet_label(3, Face::U, 2, 2), Some(8));
    assert_eq!(facelet_label(3, Face::L, 0, 0), Some(9));
    assert_eq!(facelet_label(3, Face::F, 0, 0), Some(17));
    assert_eq!(facelet_label(3, Face::R, 0, 0), Some(25));
    assert_eq!(facelet_label(3, Face::B, 0, 0), Some(33));
    assert_eq!(facelet_label(3, Face::D, 2, 2), Some(48));
}

#[test]
fn notations_agree() {
    let variants = [
        "R U R' U'",
        "R*U*R^-1*U^-1",
        "RUru",
        "R U R' U'",
        "(R U R' U')",
        "R^1 U r u",
    ];
    let reference = {
        let mut c = Cube::solved(3);
        c.apply_moves(&parse(3, variants[0]).unwrap());
        c
    };
    for word in variants {
        let mut c = Cube::solved(3);
        c.apply_moves(&parse(3, word).unwrap());
        assert_eq!(c, reference, "{word} should mean the same maneuver");
    }
}

#[test]
fn slices_and_whole_cube_turns() {
    // M is L-wise, so y L' is M-like on the middle layer only when wide moves
    // are taken apart: Rw = R M'.
    let mut wide = Cube::solved(3);
    wide.apply_moves(&parse(3, "Rw").unwrap());
    let mut parts = Cube::solved(3);
    parts.apply_moves(&parse(3, "R M'").unwrap());
    assert_eq!(wide, parts);

    // A whole-cube turn leaves every face a single color, just re-oriented.
    let mut whole = Cube::solved(3);
    whole.apply_moves(&parse(3, "x y z >U >F").unwrap());
    assert!(whole.is_uniform() && !whole.is_solved());

    // The geometer slice prefix means the same as the WCA letter.
    let mut a = Cube::solved(3);
    a.apply_moves(&parse(3, "*F *U").unwrap());
    let mut b = Cube::solved(3);
    b.apply_moves(&parse(3, "S E'").unwrap());
    assert_eq!(a, b);
}

#[test]
fn geometer_macro_language() {
    // Lower case is the inverse turn, as in the Rubik user guide.
    let mut crammed = Cube::solved(3);
    crammed.apply_moves(&parse(3, "rfu").unwrap());
    let mut spelled = Cube::solved(3);
    spelled.apply_moves(&parse(3, "R' F' U'").unwrap());
    assert_eq!(crammed, spelled);

    // The guide's example: (F F L L) has order 6.
    let moves = parse(3, "(FFLL)6").unwrap();
    assert_eq!(moves.len(), 24);
    let mut cube = Cube::solved(3);
    cube.apply_moves(&moves);
    assert!(cube.is_solved());
}
