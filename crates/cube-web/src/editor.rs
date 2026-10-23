//! The cube editor: typing a state in rather than turning one into place.
//!
//! Two ways in, both writing the same state. The letters are the format the
//! rest of the program already speaks, the one `?state=` carries and
//! [`cube_core::Cube::to_letters`] writes. The grid is the cube unfolded,
//! with a menu on every facelet.
//!
//! The menus are the point. A facelet offers only the colors that still
//! leave a cube that can be finished, so the grid cannot be painted into a
//! corner, and everything that follows from what has been entered is filled
//! in rather than asked for.

use crate::dom;
use cube_core::entry::Editor as Entry;
use cube_core::geom::Face;
use cube_core::palette::Palette;
use cube_core::render::Facelet;

/// Where each face sits in the unfolded grid, as blocks of three cells.
/// The same layout the net is drawn in, so the two read alike.
const BLOCKS: [(Face, usize, usize); 6] = [
    (Face::U, 0, 1),
    (Face::L, 1, 0),
    (Face::F, 1, 1),
    (Face::R, 1, 2),
    (Face::B, 1, 3),
    (Face::D, 2, 1),
];

/// The unfolded cube, every facelet a button that picks it.
///
/// The colors live in a strip under the grid rather than in a menu hung on
/// the cell. A menu over the grid covers the cells beneath it, and the net
/// leaves no free space under most of them, so a click meant for a covered
/// cell lands on the menu instead and paints the wrong sticker. One strip,
/// always in the same place, cannot do that -- and is easier to hit on a
/// phone. The cell being worked on is ringed, so it is clear which one the
/// strip belongs to.
pub fn grid(entry: &Entry, pal: &Palette, pick: Option<Facelet>) -> String {
    let mut html = String::new();
    for (face, _, _) in BLOCKS {
        for row in 0..3u8 {
            for col in 0..3u8 {
                let facelet = (face, row, col);
                html.push_str(&cell(entry, pal, facelet, pick == Some(facelet)));
            }
        }
    }
    html
}

const fn is_center((_, row, col): Facelet) -> bool {
    row == 1 && col == 1
}

/// Where a facelet sits in the grid, counting rows and columns from one.
fn seat_of((face, row, col): Facelet) -> (usize, usize) {
    let (_, block_row, block_col) = BLOCKS
        .into_iter()
        .find(|(f, _, _)| *f == face)
        .expect("every face has a block");
    (
        block_row * 3 + row as usize + 1,
        block_col * 3 + col as usize + 1,
    )
}

fn cell(entry: &Entry, pal: &Palette, facelet: Facelet, picked: bool) -> String {
    let (face, r, c) = facelet;
    let (row, col) = seat_of(facelet);
    let chosen = entry.get(facelet);
    // A center names a color rather than showing a sticker, so it is read
    // only: changing one would rename the face, not move anything.
    let center = is_center(facelet);
    let (fill, ink, label) = match chosen {
        Some(color) => {
            let rgb = pal.face(color);
            (rgb.hex(), ink_for(rgb.luma()), color.letter().to_string())
        }
        None => (
            "transparent".to_string(),
            "currentColor".to_string(),
            "\u{b7}".to_string(),
        ),
    };
    // The last cell of a block carries the gap that keeps one face from
    // running into the next, the way the net leaves room between faces.
    let mut class = String::from("cell");
    if center {
        class.push_str(" center");
    }
    if chosen.is_none() {
        class.push_str(" empty");
    }
    if c == 2 {
        class.push_str(" face-right");
    }
    if r == 2 {
        class.push_str(" face-foot");
    }
    if picked {
        class.push_str(" picked");
    }
    format!(
        "<button type=\"button\" class=\"{class}\" data-facelet=\"{}-{r}-{c}\"{} \
         style=\"grid-area:{row}/{col};background:{fill};color:{ink}\" \
         aria-label=\"{} row {} column {}\">{label}</button>",
        face.letter(),
        if center { " disabled" } else { "" },
        face.letter(),
        r + 1,
        c + 1,
    )
}

fn ink_for(luma: f64) -> String {
    if luma > 0.55 { "#1a1a1f" } else { "#ffffff" }.to_string()
}

/// How much is left to do, and whether what is there is a cube.
pub fn report(entry: &Entry) -> String {
    let left = entry.undecided();
    if left > 0 {
        return match left {
            1 => "One facelet left.".to_string(),
            n => format!("{n} facelets left."),
        };
    }
    match entry.cube() {
        Some(cube) => match cube_core::solver::check(&cube) {
            Ok(()) => "A cube, and the solver can take it.".to_string(),
            Err(cube_core::solver::Flaw::Parity) => {
                "Every piece is real, but no sequence of turns reaches this: \
                 one is swapped, flipped or twisted on its own."
                    .to_string()
            }
            Err(cube_core::solver::Flaw::Pieces) => {
                "Some piece shows colors no cubie carries.".to_string()
            }
        },
        None => "Still being entered.".to_string(),
    }
}

/// Put the editor's panel on the page.
pub fn render(entry: &Entry, pal: &Palette, pick: Option<Facelet>) {
    dom::set_html("editor-grid", &grid(entry, pal, pick));
    render_pick(entry, pal, pick);
    dom::set_text("editor-count", &report(entry));
    let apply: web_sys::HtmlButtonElement =
        wasm_bindgen::JsCast::unchecked_into(dom::by_id("editor-apply"));
    apply.set_disabled(entry.undecided() > 0);
}

/// The strip under the grid: the colors the picked sticker can take.
///
/// Picking is in two beats everywhere -- click the sticker, then the color --
/// whether the sticker was clicked in the grid, in the net or on the cube,
/// so all three ways in end at the same strip.
fn render_pick(entry: &Entry, pal: &Palette, pick: Option<Facelet>) {
    let strip = dom::by_id("editor-menu");
    let Some(facelet) = pick else {
        dom::set_html(
            "editor-menu",
            "<span class=\"pick-hint\">Click a sticker on the grid, the net or the cube.</span>",
        );
        let _ = strip.remove_attribute("hidden");
        return;
    };
    let (face, row, col) = facelet;
    let mut html = format!(
        "<span class=\"pick-at\">{}{}{}</span>",
        face.letter(),
        row + 1,
        col + 1
    );
    if is_center(facelet) {
        html.push_str("<span class=\"pick-hint\">a centre: it names the color</span>");
    } else {
        for color in entry.allowed(facelet) {
            let rgb = pal.face(color);
            html.push_str(&format!(
                "<button type=\"button\" class=\"pick-swatch\" data-color=\"{letter}\" \
                 style=\"background:{};color:{}\" title=\"{letter}\">{letter}</button>",
                rgb.hex(),
                ink_for(rgb.luma()),
                letter = color.letter(),
            ));
        }
        if entry.get(facelet).is_some() {
            html.push_str(
                "<button type=\"button\" class=\"pick-swatch clear\" data-color=\"\" \
                 title=\"take this sticker back, and the rest of its piece with it\">\
                 take back</button>",
            );
        }
    }
    dom::set_html("editor-menu", &html);
    let _ = strip.remove_attribute("hidden");
}
