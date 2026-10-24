//! A sequence animated into a grid: every state lands in a cell of its own,
//! and the turn that reaches it plays inside that cell as the cube arrives.
//!
//! This is the still grid of [`super::strip_svg`] put on the clock of
//! [`super::timeline`]. The first cell holds the state the sequence starts
//! from; the first move turns inside the second cell while the cube fades in
//! there; when the turn lands, that cell keeps the still picture and the next
//! one takes over. So the figure draws itself one cube per move, and a cube's
//! arrival *is* the move that produced it.
//!
//! Several tracks share one clock, which is what draws a search meeting in the
//! middle: a row walking forwards from the scramble, a row walking backwards
//! from the solved cube, and a third row that, once those two have met, carries
//! the whole solution across the grid. [`meet`] lays those three out.
//!
//! Every distinct cube drawing is written once into the timeline's backdrop and
//! placed by reference, so a grid of twenty cubes costs about what a single
//! animation of the same sequence costs.

use super::{Frame, Timeline, ease, escape};
use crate::cube::Cube;
use crate::moves::Move;
use crate::palette::Palette;
use crate::render::cube3d::Anim;
use crate::render::svg::num;
use crate::render::{Camera, Marks, Style3d, cube3d};
use std::collections::BTreeMap;

/// One lane of the grid: a cube, the moves that walk away from it, and where
/// along the grid the states they reach land.
#[derive(Clone, PartialEq, Debug)]
pub struct Track {
    /// The state the first cell holds.
    pub cube: Cube,
    /// Applied one per slot of the shared clock, in this order.
    pub moves: Vec<Move>,
    pub row: usize,
    /// Column the first cube lands in.
    pub col: usize,
    /// Fill leftwards from `col` rather than rightwards.
    pub back: bool,
    /// Slots of the shared clock to wait before the first cube appears.
    pub delay: usize,
    /// Carry on in the next row when the columns run out.
    pub wrap: bool,
    /// Caption under the first cube. Empty writes `start`.
    pub first: String,
    /// Caption under the last cube. Empty writes the move that reached it.
    pub last: String,
}

impl Track {
    /// Walked forwards from `cube`, filling rightwards from the first cell.
    pub fn new(cube: Cube, moves: Vec<Move>) -> Self {
        Self {
            cube,
            moves,
            row: 0,
            col: 0,
            back: false,
            delay: 0,
            wrap: false,
            first: String::new(),
            last: String::new(),
        }
    }

    /// Walked backwards from the far end: `cube` is the state `moves` arrive
    /// at, and the track undoes them one at a time, filling leftwards. This is
    /// the half of a two sided search that sets out from the solved cube.
    pub fn rewound(cube: Cube, moves: &[Move]) -> Self {
        Self {
            moves: moves.iter().rev().map(Move::inverse).collect(),
            back: true,
            ..Self::new(cube, Vec::new())
        }
    }

    pub fn at(mut self, row: usize, col: usize) -> Self {
        self.row = row;
        self.col = col;
        self
    }

    /// Hold still for this many slots before the first cube appears.
    pub fn after(mut self, slots: usize) -> Self {
        self.delay = slots;
        self
    }

    pub fn wrapping(mut self) -> Self {
        self.wrap = true;
        self
    }

    pub fn captioned(mut self, first: impl Into<String>, last: impl Into<String>) -> Self {
        self.first = first.into();
        self.last = last.into();
        self
    }

    /// Cells it fills: one for the state it starts from, one per move.
    pub fn cells(&self) -> usize {
        self.moves.len() + 1
    }

    /// The slot its last move lands in.
    fn span(&self) -> usize {
        self.delay + self.moves.len()
    }

    /// Where cell `i` sits, as `(row, column)`.
    fn place(&self, i: usize, columns: usize) -> (usize, usize) {
        if self.back {
            // A track that runs out of columns piles up in the first one
            // rather than walking off the figure.
            (self.row, self.col.saturating_sub(i))
        } else if self.wrap && columns > 0 {
            let col = self.col + i;
            (self.row + col / columns, col % columns)
        } else {
            (self.row, self.col + i)
        }
    }

    /// The caption cell `i` carries: the move that reached it, and the note
    /// the track puts on its first or last cell, which is what names the
    /// column two searches met in.
    fn caption(&self, i: usize) -> String {
        let note = if i == 0 && !self.first.is_empty() {
            self.first.as_str()
        } else if i + 1 == self.cells() && !self.last.is_empty() {
            self.last.as_str()
        } else {
            ""
        };
        match i {
            // The first cell is reached by no move, so the note is all of it.
            0 if note.is_empty() => "start".to_string(),
            0 => note.to_string(),
            _ if note.is_empty() => format!("{i}. {}", self.moves[i - 1].name),
            _ => format!("{i}. {} \u{00b7} {note}", self.moves[i - 1].name),
        }
    }
}

/// How the cells are arranged and how much room each one gets.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Columns a wrapping track fills before it moves to the next row.
    pub columns: usize,
    /// Space around and between cells, as a fraction of a cube.
    pub pad: f64,
    /// Room under each cube for its caption, as a fraction of a cube.
    pub caption: f64,
    /// Outline the cells no cube has reached yet, so the grid reads as a grid
    /// from the first frame.
    pub ghosts: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            columns: 8,
            pad: 0.04,
            caption: 0.12,
            ghosts: true,
        }
    }
}

/// The grid's measurements, in user units.
struct Grid {
    pad: f64,
    caption: f64,
    cell_w: f64,
    cell_h: f64,
}

impl Grid {
    fn of(layout: &Layout, st: &Style3d) -> Self {
        let pad = st.size * layout.pad;
        let caption = st.size * layout.caption;
        Self {
            pad,
            caption,
            cell_w: st.size + pad,
            cell_h: st.size + caption + pad,
        }
    }

    fn x(&self, col: usize) -> f64 {
        self.pad + col as f64 * self.cell_w
    }

    fn y(&self, row: usize) -> f64 {
        self.pad + row as f64 * self.cell_h
    }
}

/// One cell of one track, worked out: the state it holds and where it sits.
struct Cell {
    state: Cube,
    row: usize,
    col: usize,
    x: f64,
    y: f64,
    caption: String,
}

/// A track with its cells placed.
struct Lane<'a> {
    track: &'a Track,
    cells: Vec<Cell>,
}

impl<'a> Lane<'a> {
    fn of(track: &'a Track, layout: &Layout, grid: &Grid) -> Self {
        let mut state = track.cube.clone();
        let mut cells = Vec::with_capacity(track.cells());
        for i in 0..track.cells() {
            if i > 0 {
                state.apply(&track.moves[i - 1].turn);
            }
            let (row, col) = track.place(i, layout.columns);
            cells.push(Cell {
                state: state.clone(),
                row,
                col,
                x: grid.x(col),
                y: grid.y(row),
                caption: track.caption(i),
            });
        }
        Self { track, cells }
    }

    /// Moves landed by the start of `slot`, and `None` before the track opens.
    fn landed(&self, slot: usize) -> Option<usize> {
        let done = slot.checked_sub(self.track.delay)?;
        Some(done.min(self.track.moves.len()))
    }

    /// The move turning during `slot`, if any.
    fn turning(&self, slot: usize) -> Option<(&Move, &Cell, &Cell)> {
        let done = slot.checked_sub(self.track.delay)?;
        let m = self.track.moves.get(done)?;
        Some((m, &self.cells[done], &self.cells[done + 1]))
    }
}

/// Drawings written once and placed by reference, so a repeated state or a
/// repeated turn costs one `<use>` rather than another cube.
struct Gallery {
    prefix: String,
    ids: BTreeMap<String, usize>,
    items: Vec<String>,
}

impl Gallery {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            ids: BTreeMap::new(),
            items: Vec::new(),
        }
    }

    /// The id this markup is reachable at, writing it out the first time.
    fn intern(&mut self, body: String) -> String {
        let idx = match self.ids.get(&body) {
            Some(i) => *i,
            None => {
                let i = self.items.len();
                self.items.push(body.clone());
                self.ids.insert(body, i);
                i
            }
        };
        format!("{}-{idx}", self.prefix)
    }

    fn defs(&self) -> String {
        let mut s = String::from("<defs>");
        for (i, body) in self.items.iter().enumerate() {
            s.push_str(&format!("<g id=\"{}-{i}\">{body}</g>", self.prefix));
        }
        s.push_str("</defs>");
        s
    }
}

/// Everything the frames are drawn with, plus the gallery they fill.
struct Painter<'a> {
    cam: &'a Camera,
    st: &'a Style3d,
    pal: &'a Palette,
    marks: &'a Marks,
    grid: &'a Grid,
    gallery: Gallery,
}

impl Painter<'_> {
    /// A cell's drawing, placed at the cell's own origin: the cube, still or
    /// mid turn, with its caption underneath.
    fn cell(&mut self, cube: &Cube, anim: Option<&Anim>, caption: &str) -> String {
        let body = format!(
            "{}{}",
            cube3d::inner(cube, self.cam, anim, self.st, self.pal, self.marks),
            self.text(caption),
        );
        self.gallery.intern(body)
    }

    fn text(&self, caption: &str) -> String {
        if caption.is_empty() {
            return String::new();
        }
        format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" \
             font-family=\"ui-sans-serif,system-ui,sans-serif\" font-size=\"{}\" \
             fill=\"{}\">{}</text>",
            num(self.st.size * 0.5),
            num(self.st.size + self.grid.caption * 0.72),
            num(self.grid.caption * 0.62),
            self.pal.edge.hex(),
            escape(caption)
        )
    }

    /// The outlines of the cells no cube has reached by the start of `slot`.
    fn ghosts(&mut self, lanes: &[Lane], slot: usize) -> String {
        let mut taken = BTreeMap::new();
        for lane in lanes {
            let Some(landed) = lane.landed(slot) else {
                continue;
            };
            for cell in &lane.cells[..=landed] {
                taken.insert((cell.row, cell.col), ());
            }
        }
        let mut empty = BTreeMap::new();
        for lane in lanes {
            for cell in &lane.cells {
                if !taken.contains_key(&(cell.row, cell.col)) {
                    empty.insert((cell.row, cell.col), (cell.x, cell.y));
                }
            }
        }
        let inset = self.st.size * 0.05;
        let side = self.st.size - 2.0 * inset;
        let dash = self.st.size * 0.02;
        let body: String = empty
            .values()
            .map(|(x, y)| {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"none\" \
                     stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"{} {}\" \
                     opacity=\"0.3\"/>",
                    num(x + inset),
                    num(y + inset),
                    num(side),
                    num(side),
                    num(self.st.size * 0.06),
                    self.pal.edge.hex(),
                    num(self.st.stroke),
                    num(dash),
                    num(dash * 1.4),
                )
            })
            .collect();
        self.gallery.intern(body)
    }
}

/// How solid the arriving cube is, a fraction of the way through its turn. It
/// finishes materialising before the turn lands, so the end of the rotation is
/// seen at full strength.
fn fade(t: f64) -> f64 {
    0.15 + 0.85 * ease(t / 0.65)
}

/// One frame: every cube that has landed, plus the one turning its way in.
/// `t` is how far through the slot the frame sits; 0 draws the still the slot
/// starts from.
fn compose(lanes: &[Lane], slot: usize, t: f64, ghosts: Option<&str>, p: &mut Painter) -> String {
    let mut s = String::new();
    if let Some(id) = ghosts {
        s.push_str(&format!("<use href=\"#{id}\"/>"));
    }
    for lane in lanes {
        let Some(landed) = lane.landed(slot) else {
            continue;
        };
        for cell in &lane.cells[..=landed] {
            let id = p.cell(&cell.state, None, &cell.caption);
            s.push_str(&format!(
                "<use href=\"#{id}\" x=\"{}\" y=\"{}\"/>",
                num(cell.x),
                num(cell.y)
            ));
        }
        if t <= 0.0 {
            continue;
        }
        // The arriving cube is the one before the move, turning: the layer
        // swings and the whole thing fades up, in the cell it is on its way to.
        if let Some((m, from, to)) = lane.turning(slot) {
            let anim = Anim {
                turn: m.turn,
                angle: m.turn.angle() * ease(t),
            };
            let id = p.cell(&from.state, Some(&anim), &to.caption);
            s.push_str(&format!(
                "<use href=\"#{id}\" x=\"{}\" y=\"{}\" opacity=\"{}\"/>",
                num(to.x),
                num(to.y),
                num(fade(t))
            ));
        }
    }
    s
}

/// What is turning during `slot`, written out: one name per track, so a frame
/// of a two sided search is labelled with both halves.
fn label(lanes: &[Lane], slot: usize) -> String {
    let names: Vec<&str> = lanes
        .iter()
        .filter_map(|lane| lane.turning(slot).map(|(m, _, _)| m.name.as_str()))
        .collect();
    names.join(" ")
}

/// Draw the tracks into a grid, one cube per move. `steps` frames per move: 1
/// gives the states only, 6 to 10 give a smooth turn.
#[allow(clippy::too_many_arguments)]
pub fn timeline(
    tracks: &[Track],
    cam: &Camera,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
    layout: &Layout,
    steps: usize,
    ms_per_move: f64,
) -> Timeline {
    let steps = steps.max(1);
    let grid = Grid::of(layout, st);
    let lanes: Vec<Lane> = tracks
        .iter()
        .map(|track| Lane::of(track, layout, &grid))
        .collect();
    let slots = tracks.iter().map(Track::span).max().unwrap_or(0);
    let rows = lanes
        .iter()
        .flat_map(|l| l.cells.iter().map(|c| c.row + 1))
        .max()
        .unwrap_or(1);
    let cols = lanes
        .iter()
        .flat_map(|l| l.cells.iter().map(|c| c.col + 1))
        .max()
        .unwrap_or(1);

    let mut p = Painter {
        cam,
        st,
        pal,
        marks,
        grid: &grid,
        gallery: Gallery::new(format!("m{:x}", fingerprint(tracks))),
    };
    // One outline layer per slot: the set of waiting cells only changes when a
    // move lands, so every frame of a slot shares the same one.
    let ghosts: Vec<Option<String>> = (0..=slots)
        .map(|slot| layout.ghosts.then(|| p.ghosts(&lanes, slot)))
        .collect();
    let ghost_of = |slot: usize| ghosts[slot.min(slots)].as_deref();

    let mut frames = vec![Frame {
        markup: compose(&lanes, 0, 0.0, ghost_of(0), &mut p),
        label: "start".to_string(),
        ms: ms_per_move.max(1.0),
        is_state: true,
    }];
    for slot in 0..slots {
        let name = label(&lanes, slot);
        // A slot nothing turns in is one beat of stillness rather than a run
        // of identical frames: it is where a delayed row waits its turn.
        let waiting = name.is_empty();
        if !waiting {
            // The frames before the last are mid turn; the last one is every
            // move of the slot landed, which is the still the next slot
            // starts from.
            for s in 1..steps {
                frames.push(Frame {
                    markup: compose(
                        &lanes,
                        slot,
                        s as f64 / steps as f64,
                        ghost_of(slot),
                        &mut p,
                    ),
                    label: name.clone(),
                    ms: ms_per_move / steps as f64,
                    is_state: false,
                });
            }
        }
        frames.push(Frame {
            markup: compose(&lanes, slot + 1, 0.0, ghost_of(slot + 1), &mut p),
            label: if waiting { "wait".to_string() } else { name },
            ms: if waiting {
                ms_per_move
            } else {
                ms_per_move / steps as f64
            },
            is_state: true,
        });
    }
    if let Some(last) = frames.last_mut() {
        last.ms = ms_per_move.max(1.0);
    }
    Timeline {
        frames,
        width: cols as f64 * grid.cell_w + grid.pad,
        height: rows as f64 * grid.cell_h + grid.pad,
        backdrop: p.gallery.defs(),
    }
}

/// The three rows of a search that meets in the middle.
///
/// `forward` is the half found from the scramble and `backward` the half found
/// from the solved cube, both written the way they apply to the scramble: the
/// solution is the one followed by the other. The first row walks `forward`
/// rightwards from `cube`, the second walks `backward` leftwards from the state
/// the whole solution reaches, undoing one move at a time, and the two land on
/// the same state in the same column: that column is where the searches met.
/// Once they have, a third row carries the whole solution across the grid.
pub fn meet(cube: &Cube, forward: &[Move], backward: &[Move], pause: usize) -> Vec<Track> {
    let mut target = cube.clone();
    target.apply_moves(forward);
    target.apply_moves(backward);
    let end = if target.is_solved() {
        "solved"
    } else {
        "target"
    };
    let whole: Vec<Move> = forward.iter().chain(backward).cloned().collect();
    let columns = whole.len() + 1;
    let wait = forward.len().max(backward.len()) + pause;
    vec![
        Track::new(cube.clone(), forward.to_vec())
            .at(0, 0)
            .captioned("scrambled", "meet"),
        Track::rewound(target, backward)
            .at(1, columns - 1)
            .captioned(end, "meet"),
        Track::new(cube.clone(), whole)
            .at(2, 0)
            .after(wait)
            .captioned("scrambled", end),
    ]
}

/// A short tag for the ids this figure's `<defs>` use, so two mosaics on one
/// page do not reach into each other's drawings.
fn fingerprint(tracks: &[Track]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
        }
    };
    for t in tracks {
        eat(t.cube.to_letters().as_bytes());
        for m in &t.moves {
            eat(m.name.as_bytes());
        }
        eat(&[t.row as u8, t.col as u8, t.delay as u8, t.back as u8]);
    }
    h
}
