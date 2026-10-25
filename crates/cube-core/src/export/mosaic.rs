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
//! Rows share one clock but need not run at one speed: each carries its own
//! milliseconds per turn, its own pause on a landed state, and its own start.
//! That is what draws a search meeting in the middle: a row walking forwards
//! from the scramble, a row walking backwards from the solved cube, and a
//! third row that, once those two have met, carries the whole solution across
//! the grid. [`meet`] lays those three out.
//!
//! Every distinct cube drawing is written once into the timeline's backdrop and
//! placed by reference, so a grid of twenty cubes costs about what a single
//! animation of the same sequence costs.

use super::{Frame, Timeline, ease, escape};
use crate::cube::Cube;
use crate::moves::Move;
use crate::palette::{Palette, Rgb};
use crate::render::cube3d::Anim;
use crate::render::svg::num;
use crate::render::{Camera, Marks, Style3d, cube3d};
use std::collections::BTreeMap;

/// How long a cube rests on a landed state, as a fraction of a turn, when
/// neither the figure nor the track says.
const HOLD: f64 = 0.4;

/// The writing is lifted off the sticker outline's near black into gray, so
/// that it sits under the cubes rather than competing with them. The row
/// names are the darker of the two, being what the figure is divided by.
const CAPTION_GRAY: f64 = 1.45;
const TITLE_GRAY: f64 = 1.3;

/// Times closer together than this are the same moment, in milliseconds.
const TOGETHER: f64 = 0.5;

/// One lane of the grid: a cube, the moves that walk away from it, where
/// along the grid the states they reach land, and how fast it all happens.
#[derive(Clone, PartialEq, Debug)]
pub struct Track {
    /// The state the first cell holds.
    pub cube: Cube,
    /// Applied one at a time, in this order.
    pub moves: Vec<Move>,
    pub row: usize,
    /// Column the first cube lands in.
    pub col: usize,
    /// Fill leftwards from `col` rather than rightwards.
    pub back: bool,
    /// Turns of the figure's own pace to wait before the first cube appears.
    pub delay: f64,
    /// Milliseconds this row takes over a turn. `None` runs at the figure's
    /// pace, and a row of its own can crawl while another races.
    pub ms: Option<f64>,
    /// Milliseconds this row rests on a state once its turn has landed.
    pub hold: Option<f64>,
    /// Carry on in the next row when the columns run out.
    pub wrap: bool,
    /// What to write under the cube each move reaches, in place of the move's
    /// own name. Short of a name per move, the rest keep theirs.
    pub labels: Vec<String>,
    /// A name for the whole row, written in the margin beside it.
    pub title: String,
    /// Cells to mark, by index. This is how the two cubes a search meets on
    /// are pointed at; [`Layout::highlight`] says how and when.
    pub highlight: Vec<usize>,
    /// Caption under the first cube. Empty writes `start`.
    pub first: String,
    /// Caption added under the last cube, beside the move that reached it.
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
            delay: 0.0,
            ms: None,
            hold: None,
            wrap: false,
            labels: Vec::new(),
            title: String::new(),
            highlight: Vec::new(),
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

    /// Hold still for this many turns of the figure's pace before starting.
    pub fn after(mut self, turns: f64) -> Self {
        self.delay = turns;
        self
    }

    /// Run this row at its own speed: `ms` over a turn, `hold` resting on the
    /// state it lands.
    pub fn paced(mut self, ms: Option<f64>, hold: Option<f64>) -> Self {
        self.ms = ms;
        self.hold = hold;
        self
    }

    pub fn wrapping(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// Name the row, which is written in the margin beside it.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Write these under the cubes the moves reach, one per move, in place of
    /// the move names.
    pub fn labelled(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Mark these cells, by index.
    pub fn marking(mut self, cells: Vec<usize>) -> Self {
        self.highlight = cells;
        self
    }

    /// Mark the cube the last move reaches.
    pub fn marking_last(self) -> Self {
        let last = self.cells() - 1;
        self.marking(vec![last])
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

    /// The caption cell `i` carries: what the move that reached it is called,
    /// and the note the track puts on its first or last cell, which is what
    /// names the column two searches met in.
    fn caption(&self, i: usize, numbered: bool) -> String {
        let note = if i == 0 && !self.first.is_empty() {
            self.first.as_str()
        } else if i + 1 == self.cells() && !self.last.is_empty() {
            self.last.as_str()
        } else {
            ""
        };
        // The first cell is reached by no move, so the note is all of it.
        if i == 0 {
            return if note.is_empty() {
                "start".to_string()
            } else {
                note.to_string()
            };
        }
        let name = match self.labels.get(i - 1) {
            Some(given) if !given.is_empty() => given.clone(),
            _ => self.moves[i - 1].name.clone(),
        };
        let base = if numbered {
            format!("{i}. {name}")
        } else {
            name
        };
        if note.is_empty() {
            base
        } else {
            format!("{base} \u{b7} {note}")
        }
    }
}

/// How fast the figure runs, and how finely it is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Pace {
    /// Milliseconds a turn takes.
    pub ms: f64,
    /// Milliseconds a cube rests on a state once its turn has landed, which is
    /// what makes a state readable before the next one starts arriving.
    /// `None` rests for a little under half a turn.
    pub hold: Option<f64>,
    /// Frames per turn: 1 gives the states only, 6 to 10 a smooth turn.
    pub steps: usize,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            ms: 420.0,
            hold: None,
            steps: 6,
        }
    }
}

impl Pace {
    fn rest(&self) -> f64 {
        self.hold.unwrap_or(self.ms * HOLD)
    }

    /// A turn and the rest after it: the unit a track's `delay` counts.
    fn beat(&self) -> f64 {
        self.ms + self.rest()
    }
}

/// When a track turns, in milliseconds from the start of the figure.
///
/// A track appears, rests, turns, rests, turns, and so on, so the move it is
/// on is read straight off the time: cell 0 is there from `start`, and move
/// `i` swings between `turn_start` and `turn_end`.
#[derive(Clone, Copy, Debug)]
struct Clock {
    start: f64,
    ms: f64,
    hold: f64,
}

impl Clock {
    fn of(track: &Track, pace: &Pace) -> Self {
        let ms = track.ms.unwrap_or(pace.ms).max(1.0);
        Self {
            start: track.delay.max(0.0) * pace.beat(),
            ms,
            hold: track.hold.or(pace.hold).unwrap_or(ms * HOLD).max(0.0),
        }
    }

    fn beat(&self) -> f64 {
        self.ms + self.hold
    }

    fn turn_start(&self, i: usize) -> f64 {
        self.start + self.hold + i as f64 * self.beat()
    }

    fn turn_end(&self, i: usize) -> f64 {
        self.start + (i + 1) as f64 * self.beat()
    }

    /// When the track is done, the rest on its last state included.
    fn end(&self, moves: usize) -> f64 {
        self.start + self.hold + moves as f64 * self.beat()
    }
}

/// What a marked cell gets drawn on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mark {
    /// A ring round the cell, where its outline was before a cube landed.
    #[default]
    Frame,
    /// A dot under the caption.
    Dot,
}

/// When a marked cell is marked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum When {
    /// The moment every marked cube has arrived, which for a two sided search
    /// is the moment its halves meet in the middle.
    #[default]
    Met,
    /// Each one as its own cube lands.
    Landed,
    /// Once the whole figure has played out.
    End,
}

/// What the cells a track points at get, and when they get it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Highlight {
    pub mark: Mark,
    pub when: When,
    /// `None` takes the palette's own highlight.
    pub color: Option<Rgb>,
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
    /// Number the captions, as `3. R`, rather than writing the move alone.
    pub numbered: bool,
    /// Outline the cells no cube has reached yet, so the grid reads as a grid
    /// from the first frame.
    pub ghosts: bool,
    /// What the cells a track points at get, and when.
    pub highlight: Highlight,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            columns: 8,
            pad: 0.04,
            caption: 0.12,
            numbered: true,
            ghosts: true,
            highlight: Highlight::default(),
        }
    }
}

/// The grid's measurements, in user units.
struct Grid {
    pad: f64,
    caption: f64,
    /// Margin down the left for the row names, 0 when no row has one.
    gutter: f64,
    cell_w: f64,
    cell_h: f64,
}

impl Grid {
    fn of(layout: &Layout, st: &Style3d, tracks: &[Track]) -> Self {
        let pad = st.size * layout.pad;
        let caption = st.size * layout.caption;
        let font = caption * 0.62;
        let widest = tracks
            .iter()
            .map(|t| text_width(&t.title, font))
            .fold(0.0, f64::max);
        Self {
            pad,
            caption,
            gutter: if widest > 0.0 {
                widest + 2.0 * pad
            } else {
                0.0
            },
            cell_w: st.size + pad,
            cell_h: st.size + caption + pad,
        }
    }

    fn x(&self, col: usize) -> f64 {
        self.gutter + self.pad + col as f64 * self.cell_w
    }

    fn y(&self, row: usize) -> f64 {
        self.pad + row as f64 * self.cell_h
    }
}

/// Room a run of text needs. The figure only has to leave enough, so this errs
/// generous.
fn text_width(s: &str, font: f64) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    0.58 * font * s.chars().count() as f64
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

/// A track with its cells placed and its clock set.
struct Lane<'a> {
    track: &'a Track,
    cells: Vec<Cell>,
    clock: Clock,
}

impl<'a> Lane<'a> {
    fn of(track: &'a Track, layout: &Layout, grid: &Grid, pace: &Pace) -> Self {
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
                caption: track.caption(i, layout.numbered),
            });
        }
        Self {
            track,
            cells,
            clock: Clock::of(track, pace),
        }
    }

    /// Where the track has got to at `t`: the moves landed, and the one
    /// turning with how far round it is. `None` before the track opens.
    fn at(&self, t: f64) -> Option<(usize, Option<(usize, f64)>)> {
        let c = &self.clock;
        if t + TOGETHER < c.start {
            return None;
        }
        let k = self.track.moves.len();
        let i = ((t - c.start) / c.beat()).floor().max(0.0) as usize;
        if i >= k {
            return Some((k, None));
        }
        let into = t - c.start - i as f64 * c.beat();
        let turned = ((into - c.hold) / c.ms).clamp(0.0, 1.0);
        match turned {
            u if u <= 0.0 => Some((i, None)),
            u if u >= 1.0 => Some((i + 1, None)),
            u => Some((i, Some((i, u)))),
        }
    }

    /// The move turning at `t`, if any.
    fn turning(&self, t: f64) -> Option<&Move> {
        match self.at(t) {
            Some((_, Some((i, _)))) => self.track.moves.get(i),
            _ => None,
        }
    }

    /// When cell `i` gets its cube.
    fn arrival(&self, i: usize) -> f64 {
        match i.checked_sub(1) {
            None => self.clock.start,
            Some(move_) => self.clock.turn_end(move_),
        }
    }

    /// The move that lands exactly at `t`, if any.
    fn landing(&self, t: f64) -> Option<&Move> {
        (0..self.track.moves.len())
            .find(|i| (self.clock.turn_end(*i) - t).abs() < TOGETHER)
            .and_then(|i| self.track.moves.get(i))
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
    fn font(&self) -> f64 {
        self.grid.caption * 0.62
    }

    /// A cell's drawing, placed at the cell's own origin: the cube, still or
    /// mid turn, with its caption underneath.
    fn cell(&mut self, cube: &Cube, anim: Option<&Anim>, caption: &str) -> String {
        let body = format!(
            "{}{}",
            cube3d::inner(cube, self.cam, anim, self.st, self.pal, self.marks),
            self.text(
                caption,
                self.st.size * 0.5,
                self.st.size + self.grid.caption * 0.72,
                "middle",
                self.pal.edge.shade(CAPTION_GRAY),
            ),
        );
        self.gallery.intern(body)
    }

    fn text(&self, s: &str, x: f64, y: f64, anchor: &str, fill: Rgb) -> String {
        if s.is_empty() {
            return String::new();
        }
        format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\" \
             font-family=\"ui-sans-serif,system-ui,sans-serif\" font-size=\"{}\" \
             fill=\"{}\">{}</text>",
            num(x),
            num(y),
            num(self.font()),
            fill.hex(),
            escape(s)
        )
    }

    /// The row names, down the margin. They do not move, so they are drawn
    /// once behind the frames rather than in every one of them.
    fn titles(&self, lanes: &[Lane]) -> String {
        lanes
            .iter()
            .filter(|lane| !lane.track.title.is_empty())
            .map(|lane| {
                self.text(
                    &lane.track.title,
                    self.grid.gutter - self.grid.pad,
                    self.grid.y(lane.track.row) + self.st.size * 0.5 + self.font() * 0.35,
                    "end",
                    self.pal.edge.shade(TITLE_GRAY),
                )
            })
            .collect()
    }

    /// The outlines of the cells no cube has reached at `t`.
    fn ghosts(&mut self, lanes: &[Lane], t: f64) -> String {
        let mut taken = BTreeMap::new();
        for lane in lanes {
            let Some((landed, _)) = lane.at(t) else {
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
                    self.pal.edge.shade(CAPTION_GRAY).hex(),
                    num(self.st.stroke),
                    num(dash),
                    num(dash * 1.4),
                )
            })
            .collect();
        self.gallery.intern(body)
    }
}

impl Painter<'_> {
    /// What a marked cell gets: a ring where its outline was, or a dot under
    /// what is written beneath it.
    fn mark(&self, cell: &Cell, how: Mark, color: Rgb) -> String {
        match how {
            Mark::Frame => {
                let inset = self.st.size * 0.05;
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{side}\" height=\"{side}\" \
                     rx=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    num(cell.x + inset),
                    num(cell.y + inset),
                    num(self.st.size * 0.06),
                    color.hex(),
                    num(self.st.stroke * 2.4),
                    side = num(self.st.size - 2.0 * inset),
                )
            }
            Mark::Dot => format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                num(cell.x + self.st.size * 0.5),
                num(cell.y + self.st.size + self.grid.caption * 1.06),
                num(self.grid.caption * 0.19),
                color.hex(),
            ),
        }
    }

    /// The marks, grouped by the moment they appear and written once each, so
    /// that the frames only have to fade them up.
    fn marks_in_time(
        &mut self,
        lanes: &[Lane],
        how: &Highlight,
        played: f64,
    ) -> Vec<(f64, String)> {
        let color = how.color.unwrap_or(self.pal.highlight);
        let mut pending: Vec<(f64, String)> = Vec::new();
        for lane in lanes {
            for cell in lane
                .track
                .highlight
                .iter()
                .filter_map(|i| lane.cells.get(*i).map(|c| (i, c)))
            {
                let (i, cell) = cell;
                pending.push((lane.arrival(*i), self.mark(cell, how.mark, color)));
            }
        }
        if pending.is_empty() {
            return Vec::new();
        }
        // Everything together at one moment, or each as its own cube lands.
        let at = |all: f64| -> Vec<(f64, String)> {
            pending.iter().map(|(_, m)| (all, m.clone())).collect()
        };
        let timed = match how.when {
            When::Landed => pending.clone(),
            When::Met => at(pending.iter().map(|(t, _)| *t).fold(0.0, f64::max)),
            When::End => at(played),
        };
        let mut layers: BTreeMap<i64, String> = BTreeMap::new();
        for (t, markup) in timed {
            *layers.entry((t / TOGETHER).round() as i64).or_default() += &markup;
        }
        layers
            .into_iter()
            .map(|(key, body)| (key as f64 * TOGETHER, self.gallery.intern(body)))
            .collect()
    }
}

/// How solid the arriving cube is, a fraction of the way through its turn. It
/// finishes materialising before the turn lands, so the end of the rotation is
/// seen at full strength.
fn fade(t: f64) -> f64 {
    0.15 + 0.85 * ease(t / 0.65)
}

/// One frame: the grid as it stands at `t`, with every cube that has landed
/// and the ones turning their way in.
fn compose(
    lanes: &[Lane],
    t: f64,
    ghosts: Option<&str>,
    marks: &[(&str, f64)],
    p: &mut Painter,
) -> String {
    let mut s = String::new();
    if let Some(id) = ghosts {
        s.push_str(&format!("<use href=\"#{id}\"/>"));
    }
    for lane in lanes {
        let Some((landed, turning)) = lane.at(t) else {
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
        // The arriving cube is the one before the move, turning: the layer
        // swings and the whole thing fades up, in the cell it is on its way to.
        if let Some((i, u)) = turning {
            let m = &lane.track.moves[i];
            let (from, to) = (&lane.cells[i], &lane.cells[i + 1]);
            let anim = Anim {
                turn: m.turn,
                angle: m.turn.angle() * ease(u),
            };
            let id = p.cell(&from.state, Some(&anim), &to.caption);
            s.push_str(&format!(
                "<use href=\"#{id}\" x=\"{}\" y=\"{}\" opacity=\"{}\"/>",
                num(to.x),
                num(to.y),
                num(fade(u))
            ));
        }
    }
    // Over the cubes, so what is pointed at is not half hidden behind one.
    for (id, up) in marks {
        s.push_str(&format!("<use href=\"#{id}\" opacity=\"{}\"/>", num(*up)));
    }
    s
}

/// What a frame is of: whatever is turning while it is up, or, when nothing
/// is, whatever has just landed. One name per row, so a frame of a two sided
/// search is labelled with both halves.
fn label(lanes: &[Lane], start: f64, middle: f64) -> String {
    let turning: Vec<&str> = lanes
        .iter()
        .filter_map(|lane| lane.turning(middle).map(|m| m.name.as_str()))
        .collect();
    if !turning.is_empty() {
        return turning.join(" ");
    }
    let landed: Vec<&str> = lanes
        .iter()
        .filter_map(|lane| lane.landing(start).map(|m| m.name.as_str()))
        .collect();
    if !landed.is_empty() {
        return landed.join(" ");
    }
    "wait".to_string()
}

/// The moments the figure has to draw: every turn sampled `steps` times,
/// every arrival and landing, which are what the player steps between, and
/// each marking coming in over `over`.
fn moments(lanes: &[Lane], steps: usize, marked: &[(f64, String)], over: f64) -> Vec<(f64, bool)> {
    let mut marks = vec![(0.0, true)];
    for (from, _) in marked {
        for s in 0..=steps {
            marks.push((from + over * s as f64 / steps as f64, s == steps));
        }
    }
    for lane in lanes {
        marks.push((lane.clock.start, true));
        for i in 0..lane.track.moves.len() {
            let (from, to) = (lane.clock.turn_start(i), lane.clock.turn_end(i));
            for s in 0..steps {
                marks.push((from + (to - from) * s as f64 / steps as f64, false));
            }
            marks.push((to, true));
        }
    }
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, bool)> = Vec::with_capacity(marks.len());
    for (t, is_state) in marks {
        match out.last_mut() {
            Some(last) if t - last.0 < TOGETHER => last.1 |= is_state,
            _ => out.push((t, is_state)),
        }
    }
    out
}

/// Draw the tracks into a grid, one cube per move.
pub fn timeline(
    tracks: &[Track],
    cam: &Camera,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
    layout: &Layout,
    pace: &Pace,
) -> Timeline {
    let pace = Pace {
        steps: pace.steps.max(1),
        ..*pace
    };
    let grid = Grid::of(layout, st, tracks);
    let lanes: Vec<Lane> = tracks
        .iter()
        .map(|track| Lane::of(track, layout, &grid, &pace))
        .collect();
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
    let played = lanes
        .iter()
        .map(|l| l.clock.end(l.track.moves.len()))
        .fold(0.0, f64::max);

    let mut p = Painter {
        cam,
        st,
        pal,
        marks,
        grid: &grid,
        gallery: Gallery::new(format!("m{:x}", fingerprint(tracks))),
    };
    // A mark comes in over a turn's worth of time, and the figure waits for
    // the last of them before it starts over.
    let marking = pace.ms;
    let marked = p.marks_in_time(&lanes, &layout.highlight, played);
    let over = marked
        .iter()
        .map(|(t, _)| t + marking)
        .fold(played, f64::max);
    let moments = moments(&lanes, pace.steps, &marked, marking);
    let mut frames: Vec<Frame> = Vec::with_capacity(moments.len());
    for (i, &(t, is_state)) in moments.iter().enumerate() {
        // A frame is up until the next moment, and the last one rests on the
        // finished figure for a beat before it all starts again.
        let ms = match moments.get(i + 1) {
            Some(&(next, _)) => (next - t).max(1.0),
            None => (over - t).max(pace.beat()),
        };
        let ghosts = layout.ghosts.then(|| p.ghosts(&lanes, t));
        let showing: Vec<(&str, f64)> = marked
            .iter()
            .filter_map(|(from, id)| {
                let up = ease((t - from) / marking);
                (up > 0.0).then_some((id.as_str(), up))
            })
            .collect();
        let markup = compose(&lanes, t, ghosts.as_deref(), &showing, &mut p);
        // Nothing on screen changed, so this is the same frame held longer:
        // which is what a rest on a landed state is, and what a row waiting
        // for another to finish looks like.
        if let Some(last) = frames.last_mut()
            && last.markup == markup
        {
            last.ms += ms;
            last.is_state |= is_state;
            continue;
        }
        let name = match (i, frames.last()) {
            (0, _) => "start".to_string(),
            // Past the last landing nothing is turning, so rather than say so
            // the figure goes on saying what it last did.
            (_, Some(prev)) if t >= played => prev.label.clone(),
            _ => label(&lanes, t, t + ms * 0.5),
        };
        frames.push(Frame {
            markup,
            label: name,
            ms,
            is_state,
        });
    }
    let backdrop = format!("{}{}", p.gallery.defs(), p.titles(&lanes));
    Timeline {
        frames,
        width: grid.gutter + cols as f64 * grid.cell_w + grid.pad,
        height: rows as f64 * grid.cell_h + grid.pad,
        backdrop,
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
/// Once they have, a third row carries the whole solution across.
///
/// The rows are named forwards, backwards and together; rename them by setting
/// `title` on what comes back.
pub fn meet(cube: &Cube, forward: &[Move], backward: &[Move], pause: f64) -> Vec<Track> {
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
    let wait = forward.len().max(backward.len()) as f64 + pause;
    vec![
        Track::new(cube.clone(), forward.to_vec())
            .at(0, 0)
            .titled("forwards")
            .captioned("scrambled", "meet")
            .marking_last(),
        Track::rewound(target, backward)
            .at(1, columns - 1)
            .titled("backwards")
            .captioned(end, "meet")
            .marking_last(),
        Track::new(cube.clone(), whole)
            .at(2, 0)
            .after(wait)
            .titled("together")
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
        eat(t.title.as_bytes());
        eat(&[t.row as u8, t.col as u8, t.delay as u8, t.back as u8]);
        eat(&t.highlight.iter().map(|i| *i as u8).collect::<Vec<u8>>());
    }
    h
}
