//! Entering a cube by hand, one facelet at a time.
//!
//! A cube is not 54 independent stickers. The six centers name the colors,
//! and the other 48 come in the 20 cubies a 3x3 is built from: eight corners
//! of three stickers, twelve edges of two, each cubie used exactly once. So
//! most of what the user would otherwise type is already decided by what they
//! have typed.
//!
//! That is what this module is for. [`Editor::allowed`] answers, for one
//! facelet, which colors still leave a cube that can be finished -- not which
//! colors look plausible locally, but which ones survive matching every
//! unfinished slot against every unused piece. [`Editor::settle`] then fills
//! in everything that has only one answer left. Between them the user enters
//! 36 facelets at worst, and usually fewer, rather than 48.

use crate::cube::Cube;
use crate::geom::{Face, facelet_index};
use crate::render::Facelet;
use crate::solver::cubie::{CORNERS, EDGES};

/// Facelets of a 3x3.
const N: usize = 54;

/// Where a facelet sits: which cubie it belongs to, and which sticker of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Seat {
    /// A center, which names its face's color and is never edited.
    Center,
    /// Corner slot, and which of its three stickers, read clockwise.
    Corner(usize, usize),
    /// Edge slot, and which of its two stickers.
    Edge(usize, usize),
}

/// Which cubie each facelet belongs to, by numbering index.
const SEATS: [Seat; N] = seats();

const fn seats() -> [Seat; N] {
    let mut out = [Seat::Center; N];
    let mut slot = 0;
    while slot < 8 {
        let mut k = 0;
        while k < 3 {
            let (f, row, col) = CORNERS[slot][k];
            out[facelet_index(3, f, row, col)] = Seat::Corner(slot, k);
            k += 1;
        }
        slot += 1;
    }
    let mut slot = 0;
    while slot < 12 {
        let mut k = 0;
        while k < 2 {
            let (f, row, col) = EDGES[slot][k];
            out[facelet_index(3, f, row, col)] = Seat::Edge(slot, k);
            k += 1;
        }
        slot += 1;
    }
    out
}

/// A cube being typed in: every facelet either decided or still open.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Editor {
    colors: [Option<Face>; N],
}

impl Editor {
    /// A blank cube with only its centers set, since those are what the six
    /// colors are named by.
    pub fn new() -> Self {
        let mut colors = [None; N];
        for face in Face::ALL {
            colors[facelet_index(3, face, 1, 1)] = Some(face);
        }
        Self { colors }
    }

    /// Start from a state already on screen, so editing is a change to it
    /// rather than typing it again.
    pub fn from_cube(cube: &Cube) -> Self {
        let mut colors = [None; N];
        for (i, slot) in colors.iter_mut().enumerate() {
            let (face, row, col) = facelet_of(i);
            *slot = Some(cube.get(face, row, col));
        }
        Self { colors }
    }

    pub fn get(&self, facelet: Facelet) -> Option<Face> {
        self.colors[index(facelet)]
    }

    /// Set or clear one facelet. Centers are left alone: changing one would
    /// rename a color rather than move a sticker.
    pub fn set(&mut self, facelet: Facelet, color: Option<Face>) {
        let i = index(facelet);
        if SEATS[i] != Seat::Center {
            self.colors[i] = color;
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Clear every facelet of the cubie this one belongs to.
    ///
    /// Going back to a sticker means going back to its piece. Freeing one
    /// sticker on its own achieves nothing: its neighbours still name the
    /// cubie between them, so the only color it could be given back is the
    /// one it had. Centers are left alone, as ever.
    pub fn clear_cubie(&mut self, facelet: Facelet) {
        let (kind, slot) = match SEATS[index(facelet)] {
            Seat::Center => return,
            Seat::Corner(slot, _) => (Kind::Corner, slot),
            Seat::Edge(slot, _) => (Kind::Edge, slot),
        };
        for k in 0..kind.sides() {
            self.colors[kind.facelet(slot, k)] = None;
        }
    }

    /// Enter a color, freeing the cubie first if the facelet already had a
    /// different one: changing a sticker is re-entering its piece.
    pub fn choose(&mut self, facelet: Facelet, color: Face) {
        if self.get(facelet).is_some_and(|had| had != color) {
            self.clear_cubie(facelet);
        }
        self.set(facelet, Some(color));
    }

    /// Facelets still waiting for a color.
    pub fn undecided(&self) -> usize {
        self.colors.iter().filter(|c| c.is_none()).count()
    }

    /// Every facelet still open, in numbering order.
    pub fn open(&self) -> Vec<Facelet> {
        (0..N)
            .filter(|i| self.colors[*i].is_none())
            .map(facelet_of)
            .collect()
    }

    /// The colors this facelet can still take: the ones that leave a cube
    /// that can be finished.
    ///
    /// Local sense is not enough. A corner whose two stickers read white and
    /// green is a real corner, but if both white-green corners are already
    /// placed elsewhere it is not a possible one, and only matching the
    /// unfinished slots against the unused pieces catches that. A center
    /// offers only the color it names.
    pub fn allowed(&self, facelet: Facelet) -> Vec<Face> {
        let i = index(facelet);
        if let Seat::Center = SEATS[i] {
            return self.colors[i].into_iter().collect();
        }
        let mut trial = self.clone();
        // A facelet that already has a color is being re-entered, and that
        // means its piece is: judged with its neighbours still in place it
        // could only ever be put back the way it was, which is no choice at
        // all. Judged with the cubie freed, the answer is the one worth
        // showing -- every color some unused piece could still bring here.
        if trial.colors[i].is_some() {
            trial.clear_cubie(facelet);
        }
        Face::ALL
            .into_iter()
            .filter(|color| {
                trial.colors[i] = Some(*color);
                trial.is_possible()
            })
            .collect()
    }

    /// Whether what has been entered so far can still be finished.
    pub fn is_possible(&self) -> bool {
        self.placements(Kind::Corner).is_some() && self.placements(Kind::Edge).is_some()
    }

    /// Fill in every facelet that has only one color left, over and over
    /// until nothing more follows. Returns how many were filled.
    ///
    /// This is what keeps the typing down to the 36 facelets that carry
    /// information: the third sticker of a corner follows from the other two,
    /// and the last cubie of each kind follows from the ones before it.
    pub fn settle(&mut self) -> usize {
        let before = self.undecided();
        loop {
            let mut moved = false;
            for facelet in self.open() {
                let allowed = self.allowed(facelet);
                if allowed.len() == 1 {
                    self.set(facelet, Some(allowed[0]));
                    moved = true;
                }
            }
            // Orientation is not something matching can settle: with every
            // piece placed, a cubie nobody has looked at could still be put
            // in either way round. What rules that out is the cube's own
            // arithmetic, that the twists add to nothing mod 3 and the flips
            // to nothing mod 2, which decides the last one of each kind.
            if self.settle_last_orientation() {
                moved = true;
            }
            if !moved {
                return before - self.undecided();
            }
        }
    }

    /// Take as much of a proposed cube as will fit, in the order given, and
    /// settle whatever follows from it. Facelets the proposal cannot justify
    /// are left open.
    ///
    /// This is how a reading off a camera becomes something to correct
    /// rather than something to accept or throw away. A camera gets a
    /// sticker wrong now and then, and one wrong sticker makes the whole
    /// reading impossible; offered here in the order the camera was sure of
    /// them, the stickers it could see go in, the ones that cannot be true
    /// beside them are dropped, and what is left is a part-entered cube with
    /// a menu on every hole.
    ///
    /// Returns how many of the proposed facelets were taken.
    pub fn absorb(&mut self, cube: &Cube, order: &[Facelet]) -> usize {
        let mut taken = 0;
        for facelet in order {
            if self.get(*facelet).is_some() {
                continue;
            }
            let (face, row, col) = *facelet;
            let color = cube.get(face, row, col);
            if self.allowed(*facelet).contains(&color) {
                self.set(*facelet, Some(color));
                taken += 1;
            }
        }
        self.settle();
        taken
    }

    /// The state, once every facelet has a color.
    pub fn cube(&self) -> Option<Cube> {
        let mut cube = Cube::solved(3);
        for (i, color) in self.colors.iter().enumerate() {
            let (face, row, col) = facelet_of(i);
            cube.set(face, row, col, (*color)?);
        }
        Some(cube)
    }

    // ----- the pieces ----------------------------------------------------

    /// Which piece sits in each slot of one kind, if the unfinished slots can
    /// still be filled from the unused pieces at all.
    fn placements(&self, kind: Kind) -> Option<Vec<usize>> {
        let count = kind.count();
        let mut options = Vec::with_capacity(count);
        for slot in 0..count {
            let fits: Vec<usize> = (0..count)
                .filter(|piece| self.fit(kind, slot, *piece).is_some())
                .collect();
            if fits.is_empty() {
                return None;
            }
            options.push(fits);
        }
        matching(&options)
    }

    /// How `piece` would have to be turned to sit in `slot` and show the
    /// colors entered so far, if it can sit there at all.
    fn fit(&self, kind: Kind, slot: usize, piece: usize) -> Option<usize> {
        let sides = kind.sides();
        (0..sides).find(|turn| {
            (0..sides).all(|k| match self.colors[kind.facelet(slot, k)] {
                Some(color) => color == kind.color(piece, (k + turn) % sides),
                None => true,
            })
        })
    }

    /// Settle the one cubie of a kind whose orientation is still open, when
    /// every other one is known, from the parity the whole cube obeys.
    fn settle_last_orientation(&mut self) -> bool {
        let mut moved = false;
        for kind in [Kind::Corner, Kind::Edge] {
            let Some(pieces) = self.placements(kind) else {
                continue;
            };
            let sides = kind.sides();
            let open: Vec<usize> = (0..kind.count())
                .filter(|slot| (0..sides).any(|k| self.colors[kind.facelet(*slot, k)].is_none()))
                .collect();
            let [slot] = open[..] else { continue };
            // Every other cubie is fully coloured, so it names its own piece
            // and how far it is turned; the odd one out is whatever makes
            // them all add up to nothing.
            let mut total = 0;
            let mut readable = true;
            for other in (0..kind.count()).filter(|other| *other != slot) {
                match self.fit(kind, other, pieces[other]) {
                    Some(turn) => total += turn,
                    None => readable = false,
                }
            }
            if !readable {
                continue;
            }
            let turn = (sides - total % sides) % sides;
            for k in 0..sides {
                let i = kind.facelet(slot, k);
                self.colors[i] = Some(kind.color(pieces[slot], (k + turn) % sides));
            }
            moved = true;
        }
        moved
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// Corners and edges differ only in how many stickers they carry, so the two
/// passes over them are written once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Corner,
    Edge,
}

impl Kind {
    const fn count(self) -> usize {
        match self {
            Kind::Corner => 8,
            Kind::Edge => 12,
        }
    }

    const fn sides(self) -> usize {
        match self {
            Kind::Corner => 3,
            Kind::Edge => 2,
        }
    }

    /// The `k`th facelet of a slot, as an index.
    fn facelet(self, slot: usize, k: usize) -> usize {
        let (face, row, col) = match self {
            Kind::Corner => CORNERS[slot][k],
            Kind::Edge => EDGES[slot][k],
        };
        facelet_index(3, face, row, col)
    }

    /// The `k`th color of a piece. A solved cube has piece `p` in slot `p`,
    /// so a slot's own faces are the colors of the piece named after it.
    fn color(self, piece: usize, k: usize) -> Face {
        match self {
            Kind::Corner => CORNERS[piece][k].0,
            Kind::Edge => EDGES[piece][k].0,
        }
    }
}

/// A piece for every slot, none used twice: Kuhn's augmenting paths, which is
/// more than quick enough for twelve of them.
fn matching(options: &[Vec<usize>]) -> Option<Vec<usize>> {
    let n = options.len();
    let mut taken = vec![usize::MAX; n];
    for slot in 0..n {
        let mut seen = vec![false; n];
        if !augment(slot, options, &mut taken, &mut seen) {
            return None;
        }
    }
    let mut out = vec![0; n];
    for (piece, slot) in taken.iter().enumerate() {
        out[*slot] = piece;
    }
    Some(out)
}

fn augment(slot: usize, options: &[Vec<usize>], taken: &mut [usize], seen: &mut [bool]) -> bool {
    for piece in &options[slot] {
        if seen[*piece] {
            continue;
        }
        seen[*piece] = true;
        if taken[*piece] == usize::MAX || augment(taken[*piece], options, taken, seen) {
            taken[*piece] = slot;
            return true;
        }
    }
    false
}

fn index((face, row, col): Facelet) -> usize {
    facelet_index(3, face, row, col)
}

fn facelet_of(i: usize) -> Facelet {
    (Face::ALL[i / 9], ((i % 9) / 3) as u8, (i % 3) as u8)
}
