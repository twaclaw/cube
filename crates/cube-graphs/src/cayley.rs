//! The Cayley graph of a group the cube generates, with a cube at every vertex.
//!
//! A vertex for every element of the group, an edge from `g` to `gs` for every
//! generator `s`, and in place of a dot the cube that the vertex *is*. The
//! graph is built breadth first from the solved cube, so every vertex knows
//! the shortest word that reaches it and how far out it sits; the number of
//! shells the vertices fall into is God's number for that generating set.
//!
//! The figure is one SVG with no script. Each cube's geometry is written once
//! into a `<symbol>` and placed twice with `<use>`, small at the vertex and
//! large in the panel that hovering or focusing it opens, the same way
//! [`crate::tree`] does.
//!
//! Only groups small enough to draw as cubes will build: past a few hundred
//! vertices a cube is smaller than the stickers on it, and the figure worth
//! making is a different one.

use std::collections::HashMap;
use std::f64::consts::PI;

use cube_core::Cube;
use cube_core::Move;
use cube_core::notation;
use cube_core::palette::{self, Palette};
use cube_core::render::Marks;
use cube_core::render::cube3d::{self, Style3d};
use cube_core::render::svg::{P2, num, open};

use crate::tree::{CUBE_UNITS, Detail, Style, escape, hash, text_w};

/// The slot in [`Cayley::step`] for a move that leaves the drawn ball: the
/// element is real, it is just further out than the figure goes.
pub const OUTSIDE: usize = usize::MAX;

/// Vertices past which `cayley` gives up.
///
/// The figure itself has no page to fit on: it is drawn at whatever size it
/// needs and [`Cayley::html`] hands it a pane to scroll in. What this limit
/// guards is the cost of the thing, since every vertex is a cube and a cube is
/// a couple of kilobytes of SVG.
pub const DEFAULT_LIMIT: usize = 2048;

/// The type the controls around a figure are set in. A notebook may put the
/// figure in a frame with no stylesheet of its own, where the browser would
/// otherwise fall back to a serif.
const FACE: &str = "font-family:ui-sans-serif,system-ui,-apple-system,sans-serif;color:#6f6f6a";

/// Edge colours, taken in the order the generators are given.
const WIRES: [&str; 8] = [
    "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948",
];

/// What [`Cayley::html`] puts the figure in.
///
/// A Cayley graph of any size is a big drawing, and shrinking it to a page
/// takes the cubes below the size at which they are cubes. So the figure keeps
/// its size and the page gives it somewhere to live.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pane {
    /// The figure at its own size in a box this many pixels tall, which
    /// scrolls when the figure is bigger. No script: a reader drags the
    /// scrollbars. Right when the figure nearly fits.
    Scroll(f64),
    /// The whole figure fitted into a box this many pixels tall, with the
    /// wheel to zoom into a region and a drag to pan. Needs a little script,
    /// and without it a reader still sees the whole graph, just small. Right
    /// when the figure is much bigger than the column it goes in.
    Zoom(f64),
    /// No box: the figure runs down the page at full size.
    Full,
}

/// Where the vertices go.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layout {
    /// Concentric rings, one per move from the solved cube. The identity sits
    /// in the middle and the number of rings is God's number for this
    /// generating set, which is the one thing about a Cayley graph that a
    /// drawing can actually show.
    #[default]
    Shells,
    /// One circle, in the order the breadth-first search found the vertices.
    Circle,
    /// A grid, for two generators that commute and whose orders multiply to
    /// the order of the group: the group is then a torus and this says so.
    /// Falls back to [`Layout::Circle`] when they do not.
    Grid,
    /// One ring, when alternating the generators walks the whole group and
    /// comes home. That walk is a devil's algorithm for the group, and this
    /// draws it as the cycle it is. Falls back to [`Layout::Circle`].
    Ring,
}

/// One element of the group, and how the search reached it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Vertex {
    pub cube: Cube,
    /// The shortest word that reaches it, as written. Empty at the identity.
    pub path: String,
    /// Moves from solved.
    pub dist: usize,
    /// The vertex it was first reached from, and the generator that did it.
    pub from: Option<(usize, usize)>,
    pub detail: Vec<Detail>,
}

/// One edge: `to` is where the generator in slot `via` takes `from`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub via: usize,
}

/// A Cayley graph, ready to draw.
#[derive(Clone, PartialEq, Debug)]
pub struct Cayley {
    /// The generators, as they were written.
    pub gens: Vec<String>,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    /// Where each generator takes each vertex: the graph, as a table.
    pub step: Vec<Vec<usize>>,
    /// The generator that undoes each one, when the set contains it. A
    /// generator that is its own inverse points at itself. This is what
    /// decides between a line and an arrow: when the set holds both `U` and
    /// `U'` the adjacency is symmetric, so it is one line drawn once.
    pub partner: Vec<Option<usize>>,
    pub layout: Layout,
    /// How far apart the vertices sit, as a multiple of a vertex's box.
    pub spread: f64,
    /// What [`Cayley::html`] puts the figure in.
    pub pane: Pane,
    /// A line under the figure saying what it is.
    pub caption: String,
    /// Whether every element is drawn. A graph cut off at a number of rings is
    /// a ball around solved, and its outer ring is where the drawing stops,
    /// not where the group does.
    pub complete: bool,
}

/// Build the Cayley graph of the group `gens` generate on an `n`x`n`x`n` cube.
///
/// Breadth first from solved, so vertex 0 is the solved cube. Returns an error
/// rather than a graph nobody can read when the group runs past `limit`
/// elements; [`DEFAULT_LIMIT`] is the size at which the cubes give out.
///
/// Distance counts only the generators given, so the graph is directed unless
/// the set is closed under inverses: with `["U", "D"]` the element `U'` is
/// three moves away, because `U'` is not one of them, and the diameter is 6.
/// Hand it `["U", "U'", "D", "D'"]` and `U'` is one move away and the diameter
/// is 4. Which of the two is wanted is a real choice rather than a detail: the
/// second is the distance the quarter-turn metric means, and the first is the
/// distance when a turn cannot be taken back.
pub fn cayley(n: u8, gens: &[&str], limit: usize) -> Result<Cayley, String> {
    cayley_rings(n, gens, None, limit)
}

/// The same graph cut off `rings` moves out from solved: the ball of that
/// radius, which is all of the Cayley graph a figure can hold once the group is
/// too big to draw whole. `None` draws every element.
///
/// Edges are drawn between vertices that are both in the ball; a move that
/// leads further out has nowhere on the page to land, so it is left out, and
/// [`Cayley::step`] records it as [`OUTSIDE`]. With `rings` at 2 and the six
/// face turns both ways, this is the first two rings of the 3x3x3's own Cayley
/// graph, out of the twenty-six there are in that metric.
pub fn cayley_rings(
    n: u8,
    gens: &[&str],
    rings: Option<usize>,
    limit: usize,
) -> Result<Cayley, String> {
    if gens.is_empty() {
        return Err("a Cayley graph needs at least one generator".to_string());
    }
    let words: Vec<Vec<Move>> = gens
        .iter()
        .map(|w| notation::parse(n, w).map_err(|e| format!("cannot read {w:?}: {e}")))
        .collect::<Result<_, _>>()?;

    let solved = Cube::solved(n);
    for (w, moves) in gens.iter().zip(&words) {
        let mut probe = solved.clone();
        probe.apply_moves(moves);
        if probe == solved {
            return Err(format!(
                "{w:?} does not move the cube, so it generates nothing"
            ));
        }
    }

    // Breadth first, so vertex order is by distance and the path kept for a
    // vertex is a shortest one.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut vertices = vec![Vertex {
        cube: solved.clone(),
        path: String::new(),
        dist: 0,
        from: None,
        detail: Vec::new(),
    }];
    let mut trail: Vec<Vec<Move>> = vec![Vec::new()];
    index.insert(solved.to_letters(), 0);
    let mut head = 0;
    while head < vertices.len() {
        let here = vertices[head].cube.clone();
        let dist = vertices[head].dist;
        if rings.is_some_and(|r| dist >= r) {
            // The last ring of the ball: its neighbours further out exist,
            // but they are not drawn, so they are not visited either.
            head += 1;
            continue;
        }
        for (gi, moves) in words.iter().enumerate() {
            let mut next = here.clone();
            next.apply_moves(moves);
            let key = next.to_letters();
            if index.contains_key(&key) {
                continue;
            }
            if vertices.len() >= limit {
                return Err(format!(
                    "these generators reach more than {limit} elements{within}; \
                     raise the limit to draw it anyway, bearing in mind that \
                     every vertex is a cube and a cube is a couple of \
                     kilobytes, or draw fewer rings",
                    within = rings.map_or(String::new(), |r| format!(" within {r} rings")),
                ));
            }
            index.insert(key, vertices.len());
            let mut word = trail[head].clone();
            word.extend(moves.iter().cloned());
            vertices.push(Vertex {
                cube: next,
                path: if vertices[head].path.is_empty() {
                    gens[gi].to_string()
                } else {
                    format!("{} {}", vertices[head].path, gens[gi])
                },
                dist: dist + 1,
                from: Some((head, gi)),
                detail: Vec::new(),
            });
            trail.push(word);
        }
        head += 1;
    }

    // Where each generator takes each vertex: everything below reads this
    // rather than turning cubes again.
    let mut step = vec![vec![0usize; words.len()]; vertices.len()];
    for (v, row) in step.iter_mut().enumerate() {
        for (gi, moves) in words.iter().enumerate() {
            let mut next = vertices[v].cube.clone();
            next.apply_moves(moves);
            row[gi] = index.get(&next.to_letters()).copied().unwrap_or(OUTSIDE);
        }
    }
    // Which generator undoes which: g_i * g_j is the identity exactly when
    // g_j is the inverse of g_i, and testing it at the identity settles it
    // everywhere.
    let partner: Vec<Option<usize>> = (0..words.len())
        .map(|i| {
            let first = step[0][i];
            (first != OUTSIDE)
                .then(|| (0..words.len()).find(|&j| step[first][j] == 0))
                .flatten()
        })
        .collect();

    // An edge for every generator at every vertex, except that an adjacency
    // two generators both describe is drawn once. A generator that is its own
    // inverse joins each pair twice; a generator whose inverse is also in the
    // set joins the same pairs backwards.
    let mut edges = Vec::new();
    for (from, row) in step.iter().enumerate() {
        for (gi, &to) in row.iter().enumerate() {
            if to == from || to == OUTSIDE {
                continue;
            }
            match partner[gi] {
                Some(j) if j == gi && to < from => continue,
                Some(j) if j < gi => continue,
                _ => {}
            }
            edges.push(Edge { from, to, via: gi });
        }
    }

    let order = vertices.len();
    for (v, vertex) in vertices.iter_mut().enumerate() {
        vertex.detail.push(Detail::new(
            "distance",
            if vertex.dist == 1 {
                "1 move".to_string()
            } else {
                format!("{} moves", vertex.dist)
            },
        ));
        vertex.detail.push(Detail::new(
            "order",
            element_order(n, &trail[v], order).to_string(),
        ));
    }

    // A generator and the one that undoes it are one kind of edge and share a
    // colour, so it is kinds of edge that have to fit in the palette.
    let kinds = (0..words.len())
        .filter(|&i| !matches!(partner[i], Some(j) if j < i))
        .count();
    if kinds > WIRES.len() {
        return Err(format!(
            "{kinds} kinds of edge is more than the {} colours there are to tell \
             them apart with",
            WIRES.len()
        ));
    }
    let complete = step.iter().all(|row| row.iter().all(|&to| to != OUTSIDE));
    Ok(Cayley {
        gens: gens.iter().map(|w| (*w).to_string()).collect(),
        vertices,
        edges,
        step,
        partner,
        layout: Layout::default(),
        spread: 2.0,
        pane: Pane::Scroll(640.0),
        caption: String::new(),
        complete,
    })
}

/// How many times a word has to be done before the cube comes back.
///
/// Every element of a finite group has an order dividing the group's, so the
/// loop cannot run past `cap`.
fn element_order(n: u8, word: &[Move], cap: usize) -> usize {
    let solved = Cube::solved(n);
    let mut cur = solved.clone();
    for k in 1..=cap.max(1) {
        cur.apply_moves(word);
        if cur == solved {
            return k;
        }
    }
    1
}

impl Cayley {
    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn with_spread(mut self, spread: f64) -> Self {
        self.spread = spread.max(1.1);
        self
    }

    /// What to put the figure in: a scrolling box, a zoomable one, or nothing.
    pub fn with_pane(mut self, pane: Pane) -> Self {
        self.pane = pane;
        self
    }

    /// Elements of the group.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// The longest shortest word, which is God's number for this generating
    /// set, and the number of rings a shell layout draws less one.
    pub fn diameter(&self) -> usize {
        self.vertices.iter().map(|v| v.dist).max().unwrap_or(0)
    }

    /// How many elements sit at each distance from solved.
    pub fn shells(&self) -> Vec<usize> {
        let mut out = vec![0usize; self.diameter() + 1];
        for v in &self.vertices {
            out[v.dist] += 1;
        }
        out
    }

    /// The order of the generator in slot `gi`.
    fn gen_order(&self, gi: usize) -> usize {
        let mut cur = self.step[0][gi];
        let mut k = 1;
        while cur != 0 && cur != OUTSIDE && k <= self.len() {
            cur = self.step[cur][gi];
            k += 1;
        }
        k
    }

    /// Where every vertex sits, centred on the origin.
    fn places(&self, st: &Style) -> Vec<P2> {
        let cell = st.node * self.spread;
        match self.layout {
            Layout::Shells => self.shell_places(cell, st.node),
            Layout::Grid => self.grid_places(cell).unwrap_or_else(|| {
                let order: Vec<usize> = (0..self.len()).collect();
                self.round(&order, cell)
            }),
            Layout::Ring => {
                let walk = self.ring_walk();
                let order = walk.unwrap_or_else(|| (0..self.len()).collect());
                self.round(&order, cell)
            }
            Layout::Circle => {
                let order: Vec<usize> = (0..self.len()).collect();
                self.round(&order, cell)
            }
        }
    }

    /// `order` laid round one circle, in that order.
    fn round(&self, order: &[usize], cell: f64) -> Vec<P2> {
        let n = order.len();
        let radius = if n > 2 {
            (cell / (2.0 * (PI / n as f64).sin())).max(cell)
        } else {
            cell * 0.5 * n as f64
        };
        let mut pos = vec![[0.0, 0.0]; self.len()];
        for (k, &v) in order.iter().enumerate() {
            let th = 2.0 * PI * k as f64 / n as f64;
            pos[v] = [radius * th.sin(), -radius * th.cos()];
        }
        pos
    }

    /// One ring per distance, each ring far enough out to hold its own
    /// vertices, and each vertex near whichever one reached it so that the
    /// edges stay short.
    fn shell_places(&self, cell: f64, node: f64) -> Vec<P2> {
        let deep = self.diameter();
        let mut rings: Vec<Vec<usize>> = vec![Vec::new(); deep + 1];
        for (i, v) in self.vertices.iter().enumerate() {
            rings[v.dist].push(i);
        }
        let mut pos = vec![[0.0, 0.0]; self.len()];
        let mut angle = vec![0.0f64; self.len()];
        let mut radius = 0.0f64;
        // Two different spacings. Going outward, consecutive rings only have to
        // clear each other, so a little over a vertex box will do. Going round,
        // a ring has to seat all its own vertices, which is what `cell` is for.
        let step = node + (cell - node) * 0.5;
        for ring in rings.iter().skip(1) {
            let count = ring.len() as f64;
            radius = (radius + step).max(count * cell / (2.0 * PI));
            let parent_angle = |v: usize| self.vertices[v].from.map_or(0.0, |(p, _)| angle[p]);
            let mut order = ring.clone();
            order.sort_by(|a, b| parent_angle(*a).total_cmp(&parent_angle(*b)).then(a.cmp(b)));
            // Even spacing is what keeps a ring from colliding with itself, so
            // the only freedom left is where the ring starts. Spend it on
            // putting each vertex as near as it can get to whichever vertex
            // reached it: the edges then run outward rather than across, and
            // consecutive rings stop lining up into spokes.
            let (mut sx, mut sy) = (0.0, 0.0);
            for (k, &v) in order.iter().enumerate() {
                let want = parent_angle(v) - 2.0 * PI * k as f64 / count;
                sx += want.cos();
                sy += want.sin();
            }
            let phase = if sx.hypot(sy) < 1e-9 {
                0.0
            } else {
                sy.atan2(sx)
            };
            for (k, &v) in order.iter().enumerate() {
                let th = phase + 2.0 * PI * k as f64 / count;
                angle[v] = th;
                pos[v] = [radius * th.sin(), -radius * th.cos()];
            }
        }
        pos
    }

    /// Two commuting generators whose powers fill the group: a torus, drawn
    /// as one.
    fn grid_places(&self, cell: f64) -> Option<Vec<P2>> {
        // A ball cut off before the group closes up is not a torus yet.
        if self.gens.len() != 2 || !self.complete {
            return None;
        }
        // a*b and b*a are the same element, so the group is abelian.
        if self.step[self.step[0][0]][1] != self.step[self.step[0][1]][0] {
            return None;
        }
        let (rows, cols) = (self.gen_order(0), self.gen_order(1));
        if rows * cols != self.len() {
            return None;
        }
        let mut pos = vec![[0.0, 0.0]; self.len()];
        let mut row_head = 0usize;
        for i in 0..rows {
            let mut cur = row_head;
            for j in 0..cols {
                pos[cur] = [
                    (j as f64 - (cols as f64 - 1.0) / 2.0) * cell,
                    (i as f64 - (rows as f64 - 1.0) / 2.0) * cell,
                ];
                cur = self.step[cur][1];
            }
            row_head = self.step[row_head][0];
        }
        Some(pos)
    }

    /// Alternating the generators, if that walks every element once and comes
    /// home: a Hamiltonian cycle, and a devil's algorithm for this group.
    fn ring_walk(&self) -> Option<Vec<usize>> {
        let g = self.gens.len();
        let mut walk = vec![0usize];
        let mut seen = vec![false; self.len()];
        seen[0] = true;
        let mut cur = 0usize;
        for i in 0..self.len() {
            cur = self.step[cur][i % g];
            if cur == OUTSIDE {
                return None;
            }
            if cur == 0 {
                break;
            }
            if seen[cur] {
                return None;
            }
            seen[cur] = true;
            walk.push(cur);
        }
        (cur == 0 && walk.len() == self.len()).then_some(walk)
    }

    /// One SVG document: the whole figure, panels and all, with no script.
    pub fn svg(&self, st: &Style) -> String {
        self.draw(st, None).0
    }

    /// The same figure in a little HTML, with a draggable cube when `app` is
    /// the base URL of the published app. Nothing is fetched until a reader
    /// clicks a vertex.
    pub fn html(&self, st: &Style, app: Option<&str>) -> String {
        self.html_sized(st, app).0
    }

    /// [`Cayley::html`], and how tall it comes out, in CSS pixels. A page that
    /// sets the figure in a frame of its own needs the second number, since a
    /// frame does not grow to fit what is in it.
    pub fn html_sized(&self, st: &Style, app: Option<&str>) -> (String, f64) {
        let (svg, id, size) = self.draw(st, app);
        // A drawing wider than a book column scrolls sideways, and the
        // scrollbar takes a line of its own.
        let sideways = if size[0] > 900.0 { 16.0 } else { 0.0 };
        let pane = match self.pane {
            Pane::Scroll(h) => size[1].min(h) + 2.0 + sideways,
            Pane::Zoom(h) => h + 2.0 + 34.0,
            Pane::Full => size[1] + sideways,
        };
        let keys = if self.legend_entries().len() > 1 {
            32.0
        } else {
            0.0
        };
        // The line inviting a click. The cube a click opens takes more, but
        // the figure grows its own frame to fit when that happens, so the
        // page does not carry an empty hole for it before anyone clicks.
        let player = if app.is_some() { 40.0 } else { 0.0 };
        let height = (keys + pane + player + 24.0).ceil();
        (self.wrap(svg, &id, size, app), height)
    }

    fn wrap(&self, svg: String, id: &str, size: P2, app: Option<&str>) -> String {
        let pane = self.pane_markup(&svg, id, size);
        let keys = self.keys_markup(id);
        let Some(base) = app else {
            return format!(
                "<div class=\"cube-cayley\" id=\"{id}-wrap\" style=\"{FACE}\">{keys}{pane}</div>"
            );
        };
        let join = if base.contains('?') { "&" } else { "?" };
        format!(
            "<div class=\"cube-cayley\" id=\"{id}-wrap\" style=\"{FACE}\">{keys}{pane}\
<div style=\"margin:.5rem auto 0;max-width:30rem;text-align:center\">\
<div style=\"font-size:.8rem;opacity:.7;margin-bottom:.25rem\" id=\"{id}-say\">\
Click a cube, or an edge for the cube it leads to.</div>\
<iframe id=\"{id}-app\" title=\"the cube at that vertex\" loading=\"lazy\" \
style=\"width:100%;height:320px;border:0;display:none\"></iframe></div></div>\
<script>(function(){{\
var w=document.getElementById(\"{id}-wrap\"),f=document.getElementById(\"{id}-app\"),\
s=document.getElementById(\"{id}-say\");\
function open(t){{var g=t.closest(\"[data-state]\");if(!g)return;\
f.src=\"{base}{join}embed=1&views=3d&controls=1&state=\"+g.getAttribute(\"data-state\");\
f.style.display=\"block\";\
s.textContent=\"Drag to turn \"+(g.getAttribute(\"data-path\")||\"the solved cube\")+\".\";\
var fe=window.frameElement;\
if(fe){{fe.style.height=(document.documentElement.scrollHeight+8)+\"px\";}}}}\
w.addEventListener(\"click\",function(e){{open(e.target);}});\
w.addEventListener(\"focusin\",function(e){{open(e.target);}});\
}})();</script>"
        )
    }

    /// A switch per kind of edge, so a reader can take the clutter down to the
    /// one move they are following. The rule is CSS, not script: a figure with
    /// a box unticked hides every edge of that generator, and its panel with
    /// it. A single generator gets no switches, there being nothing to choose.
    fn keys_markup(&self, id: &str) -> String {
        let entries = self.legend_entries();
        if entries.len() < 2 {
            return String::new();
        }
        let mut row = String::from(
            "<div style=\"display:flex;flex-wrap:wrap;gap:.15rem 1rem;\
             justify-content:center;align-items:center;margin:0 0 .45rem;\
             font-size:.8rem\"><span style=\"opacity:.55\">show</span>",
        );
        let mut css = String::new();
        for (gi, name) in &entries {
            row.push_str(&format!(
                "<label style=\"display:inline-flex;align-items:center;gap:.35rem;\
                 cursor:pointer\"><input type=\"checkbox\" id=\"{id}-k{gi}\" checked>\
                 <span style=\"display:inline-block;width:1.1rem;border-top:2px solid {};\
                 \"></span><code>{}</code></label>",
                self.colour(*gi),
                escape(name),
            ));
            css.push_str(&format!(
                "#{id}-wrap:has(#{id}-k{gi}:not(:checked)) .gen{gi}{{display:none}}"
            ));
        }
        row.push_str("</div>");
        format!("{row}<style>{css}</style>")
    }

    /// The box the figure goes in, and whatever it takes to move around it.
    fn pane_markup(&self, svg: &str, id: &str, size: P2) -> String {
        let frame = "max-width:100%;border:1px solid #8a8a8533;border-radius:8px";
        match self.pane {
            // Centred: a figure narrower than the column should not sit in
            // one corner of it.
            Pane::Full => format!(
                "<div style=\"overflow-x:auto;max-width:100%;text-align:center\">{svg}</div>"
            ),
            Pane::Scroll(h) => format!(
                "<div style=\"overflow:auto;max-height:{}px;text-align:center;{frame}\">\
                 {svg}</div>",
                num(h)
            ),
            Pane::Zoom(h) => {
                // Fit the whole thing into the box, then let the viewBox do the
                // zooming: the cubes stay vector all the way in.
                let fitted = svg.replacen(
                    &format!("width=\"{}\" height=\"{}\"", num(size[0]), num(size[1])),
                    &format!(
                        "width=\"100%\" height=\"{}\" style=\"display:block;\
                         touch-action:none;cursor:grab\"",
                        num(h)
                    ),
                    1,
                );
                format!(
                    "<div id=\"{id}-zoom\" style=\"{frame}\">{fitted}</div>\
<div style=\"display:flex;gap:.4rem;align-items:center;justify-content:center;\
margin-top:.35rem;font-size:.78rem;opacity:.7\">\
<button type=\"button\" id=\"{id}-out\">&minus;</button>\
<button type=\"button\" id=\"{id}-in\">+</button>\
<button type=\"button\" id=\"{id}-fit\">fit</button>\
<span>scroll to zoom, drag to pan</span></div>\
<script>(function(){{\
var p=document.getElementById(\"{id}-zoom\"),s=p.querySelector(\"svg\");\
var home=s.getAttribute(\"viewBox\").split(/\\s+/).map(Number),b=home.slice();\
function put(){{s.setAttribute(\"viewBox\",b.join(\" \"));}}\
function scale(k,fx,fy){{\
var w=Math.min(home[2],Math.max(home[2]/60,b[2]*k));\
var h=w*home[3]/home[2];\
b[0]+=(b[2]-w)*fx;b[1]+=(b[3]-h)*fy;b[2]=w;b[3]=h;put();}}\
s.addEventListener(\"wheel\",function(e){{e.preventDefault();\
var r=s.getBoundingClientRect();\
scale(Math.exp(e.deltaY*0.0016),(e.clientX-r.left)/r.width,(e.clientY-r.top)/r.height);\
}},{{passive:false}});\
var at=null;\
s.addEventListener(\"pointerdown\",function(e){{at=[e.clientX,e.clientY];\
s.setPointerCapture(e.pointerId);s.style.cursor=\"grabbing\";}});\
s.addEventListener(\"pointermove\",function(e){{if(!at)return;\
var r=s.getBoundingClientRect();\
b[0]-=(e.clientX-at[0])*b[2]/r.width;b[1]-=(e.clientY-at[1])*b[3]/r.height;\
at=[e.clientX,e.clientY];put();}});\
function drop(){{at=null;s.style.cursor=\"grab\";}}\
s.addEventListener(\"pointerup\",drop);s.addEventListener(\"pointercancel\",drop);\
document.getElementById(\"{id}-in\").onclick=function(){{scale(0.7,0.5,0.5);}};\
document.getElementById(\"{id}-out\").onclick=function(){{scale(1/0.7,0.5,0.5);}};\
document.getElementById(\"{id}-fit\").onclick=function(){{b=home.slice();put();}};\
}})();</script>"
                )
            }
        }
    }

    /// The figure, the id its CSS is scoped to, and the canvas it was drawn
    /// on, which is what a pane needs to fit it.
    fn draw(&self, st: &Style, app: Option<&str>) -> (String, String, P2) {
        let spots = self.places(st);
        let pad = 1.6 * st.font;
        let half = st.node / 2.0;
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &spots {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a] - half);
                hi[a] = hi[a].max(p[a] + half);
            }
        }
        let caption_h = if self.caption.is_empty() {
            0.0
        } else {
            2.2 * st.font
        };
        let legend_h = 2.0 * st.font;
        let content = [hi[0] - lo[0], hi[1] - lo[1]];
        let width = (content[0] + 2.0 * pad)
            .max(text_w(&self.caption, st.font * 1.05) + 2.0 * pad)
            .max(self.legend_width(st) + 2.0 * pad);
        let height = content[1] + 2.0 * pad + caption_h + legend_h;
        // Positions are centred on the origin; move them onto the canvas.
        let shift = [(width - content[0]) / 2.0 - lo[0], pad - lo[1]];
        let at = |v: usize| -> P2 { [spots[v][0] + shift[0], spots[v][1] + shift[1]] };
        let id = format!("cg{:x}", hash(&self.caption, self.len(), self.edges.len()));

        let style3d = Style3d {
            size: CUBE_UNITS,
            stroke: 2.4,
            shade: if st.flat {
                0.0
            } else {
                Style3d::default().shade
            },
            ..Style3d::default()
        };
        let pal: Palette = palette::STANDARD;
        let marks = Marks::new();

        let mut defs = String::from("<defs>");
        for (gi, _) in self.gens.iter().enumerate() {
            if self.partner[gi].is_some() {
                continue;
            }
            defs.push_str(&format!(
                "<marker id=\"{id}-a{gi}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
                 markerWidth=\"4.5\" markerHeight=\"4.5\" orient=\"auto\">\
                 <path d=\"M0 0L10 5L0 10Z\" fill=\"{}\"/></marker>",
                self.colour(gi)
            ));
        }
        let mut nodes = String::new();
        let mut panels = String::new();
        let mut rules = String::new();
        for v in 0..self.len() {
            defs.push_str(&format!(
                "<symbol id=\"{id}-c{v}\" viewBox=\"0 0 {u} {u}\">{body}</symbol>",
                u = num(CUBE_UNITS),
                body = cube3d::inner(
                    &self.vertices[v].cube,
                    &st.camera,
                    None,
                    &style3d,
                    &pal,
                    &marks
                ),
            ));
            nodes.push_str(&self.vertex_markup(v, &id, at(v), st, app));
            panels.push_str(&self.panel_markup(v, &id, at(v), st, [width, height]));
            rules.push_str(&format!(
                "#{id}:hover .n{v}:hover~.h,#{id} .n{v}:focus~.h{{opacity:0}}\
                 #{id}:has(.n{v}:hover) .p{v},#{id}:has(.n{v}:focus) .p{v}{{opacity:1}}",
            ));
        }
        defs.push_str("</defs>");

        let mut edges = String::new();
        // In a shell layout the rings are the point, so say where they are:
        // the reader counts them, and the count is God's number.
        if self.layout == Layout::Shells {
            let mut radii = vec![0.0f64; self.diameter() + 1];
            for (v, p) in spots.iter().enumerate() {
                radii[self.vertices[v].dist] = p[0].hypot(p[1]);
            }
            // A vertex sits at the centre of a box, so a guide drawn through
            // the centres cuts every cube on the ring in half and the ones on
            // the last ring look like they escaped. Draw each guide where its
            // shell ends instead: the rings become bands, each holding its own
            // cubes, and the outermost encloses the whole graph.
            for r in &radii {
                edges.push_str(&format!(
                    "<circle class=\"ring\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\"/>",
                    num(shift[0]),
                    num(shift[1]),
                    num(r + st.node / 2.0 + 1.5),
                ));
            }
        }
        let mid = [shift[0], shift[1]];
        for (k, e) in self.edges.iter().enumerate() {
            edges.push_str(&self.wire_markup(e, k, &id, at(e.from), at(e.to), mid, st, app));
            panels.push_str(&self.edge_panel_markup(
                e,
                k,
                &id,
                at(e.from),
                at(e.to),
                mid,
                st,
                [width, height],
            ));
            rules.push_str(&format!(
                "#{id}:has(.e{k}:hover) .pe{k},#{id}:has(.e{k}:focus) .pe{k}{{opacity:1}}"
            ));
        }

        let mut out = open(width, height, None);
        // Every word is grey: black disappears into a dark page, and white
        // into a light one. The card behind a panel is the one surface that
        // has to pick a side, so it follows the reader's theme.
        out.push_str(&format!(
            "<style>#{id} text{{font-family:ui-sans-serif,system-ui,-apple-system,sans-serif}}\
             #{id} .v{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}\
             #{id} .t{{fill:#6f6f6a}}#{id} .s{{fill:#8e8e88}}\
             #{id} .card{{fill:#f7f7f5;stroke:#8a8a8566;stroke-width:1}}\
             #{id} .ring{{stroke:#8a8a8540;stroke-width:1;stroke-dasharray:3 5}}\
             #{id} .n{{cursor:zoom-in}}#{id} .n:focus{{outline:none;cursor:zoom-out}}\
             #{id} .e{{cursor:pointer}}#{id} .e:focus{{outline:none}}\
             #{id} .p{{opacity:0;transition:opacity .12s ease-out;pointer-events:none}}\
             @media (prefers-color-scheme:dark){{\
             #{id} .t{{fill:#c3c2b7}}#{id} .s{{fill:#9a9a93}}\
             #{id} .ring{{stroke:#8a8a8555}}\
             #{id} .card{{fill:#23231f;stroke:#8a8a8577}}}}\
             {rules}</style>"
        ));
        out.push_str(&defs);
        out.push_str(&format!("<g id=\"{id}\">"));
        out.push_str(&edges);
        out.push_str(&nodes);
        out.push_str(&panels);
        out.push_str(&self.footer(st, width, height, caption_h, legend_h));
        out.push_str("</g></svg>");
        (out, id, [width, height])
    }

    /// One edge, bowed clear of whatever it would otherwise cross, and cut
    /// short of both cubes so that an arrowhead lands in the open.
    /// Where an edge is drawn: the two ends it runs between, after both have
    /// been pulled clear of their cubes, and the point it bends through.
    fn wire_path(&self, a: P2, b: P2, mid: P2, st: &Style) -> Option<(P2, P2, P2, f64)> {
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = d[0].hypot(d[1]);
        let trim = st.node * 0.62;
        if len <= 2.0 * trim + 2.0 {
            return None;
        }
        let u = [d[0] / len, d[1] / len];
        let p1 = [a[0] + u[0] * trim, a[1] + u[1] * trim];
        let p2 = [b[0] - u[0] * trim, b[1] - u[1] * trim];
        // Enough of a curve to tell two edges apart, never enough to balloon.
        let bow = (0.06 * len).min(0.8 * st.node);
        // Bow away from the middle of the figure. A chord that curves outward
        // goes round the crowd; the same chord curved inward goes through it.
        let seat = [(p1[0] + p2[0]) / 2.0, (p1[1] + p2[1]) / 2.0];
        let out = [seat[0] - mid[0], seat[1] - mid[1]];
        let side = if out[0] * -u[1] + out[1] * u[0] < 0.0 {
            -1.0
        } else {
            1.0
        };
        let ctrl = [seat[0] - u[1] * bow * side, seat[1] + u[0] * bow * side];
        Some((p1, ctrl, p2, len))
    }

    /// One edge, as something a reader can land on: the line itself, a fat
    /// transparent twin so that a two pixel curve is still worth aiming at,
    /// and the state it leads to, so clicking it opens that cube the way
    /// clicking a vertex does.
    #[allow(clippy::too_many_arguments)]
    fn wire_markup(
        &self,
        e: &Edge,
        k: usize,
        id: &str,
        a: P2,
        b: P2,
        mid: P2,
        st: &Style,
        app: Option<&str>,
    ) -> String {
        let Some((p1, ctrl, p2, len)) = self.wire_path(a, b, mid, st) else {
            return String::new();
        };
        let head = if self.partner[e.via].is_some() {
            String::new()
        } else {
            format!(" marker-end=\"url(#{id}-a{})\"", e.via)
        };
        // A short edge joins two cubes a reader is looking at; a long one
        // crosses the whole figure. Fading the long ones lets the local
        // structure show through what would otherwise be a hairball.
        let reach = (len / (10.0 * st.node)).clamp(0.0, 1.0);
        let fade = 0.65 - 0.45 * reach;
        let d = format!(
            "M{} {}Q{} {},{} {}",
            num(p1[0]),
            num(p1[1]),
            num(ctrl[0]),
            num(ctrl[1]),
            num(p2[0]),
            num(p2[1])
        );
        let data = if app.is_some() {
            format!(
                " data-state=\"{}\" data-path=\"{}\"",
                self.vertices[e.to].cube.to_letters(),
                escape(self.name_of(e.to))
            )
        } else {
            String::new()
        };
        format!(
            "<g class=\"e e{k} gen{via}\" tabindex=\"0\"{data}><title>{tip}</title>\
             <path d=\"{d}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"2\" \
             opacity=\"{fade}\"{head}/>\
             <path d=\"{d}\" fill=\"none\" stroke=\"transparent\" \
             stroke-width=\"{hit}\"/></g>",
            via = e.via,
            tip = escape(&self.edge_tooltip(e)),
            colour = self.colour(e.via),
            fade = num(fade),
            hit = num((st.node * 0.3).max(9.0)),
        )
    }

    /// What a vertex is called: its word, or `solved` at the identity.
    fn name_of(&self, v: usize) -> &str {
        if self.vertices[v].path.is_empty() {
            "solved"
        } else {
            &self.vertices[v].path
        }
    }

    fn edge_tooltip(&self, e: &Edge) -> String {
        format!(
            "{}:  {}  \u{2192}  {}",
            self.gens[e.via],
            self.name_of(e.from),
            self.name_of(e.to)
        )
    }

    /// The panel an edge opens: the move it is, and the cube it leads to.
    #[allow(clippy::too_many_arguments)]
    fn edge_panel_markup(
        &self,
        e: &Edge,
        k: usize,
        id: &str,
        a: P2,
        b: P2,
        mid: P2,
        st: &Style,
        canvas: P2,
    ) -> String {
        let Some((_, ctrl, _, _)) = self.wire_path(a, b, mid, st) else {
            return String::new();
        };
        let [width, height] = canvas;
        let cube = st.node * st.zoom * 0.8;
        let line = 1.55 * st.font;
        let rows = [
            ("from", self.name_of(e.from).to_string()),
            ("to", self.name_of(e.to).to_string()),
            ("distance", format!("{} out", self.vertices[e.to].dist)),
        ];
        let body = rows
            .iter()
            .map(|(k, v)| 0.56 * st.font * (k.chars().count() + v.chars().count() + 3) as f64)
            .fold(cube, f64::max)
            .min(26.0 * st.font);
        let pw = body.max(cube) + 2.0 * st.font;
        let ph = cube + line * (rows.len() as f64 + 1.6) + 1.4 * st.font;
        let px = (ctrl[0] + 14.0).min((width - pw - 4.0).max(4.0)).max(4.0);
        let py = (ctrl[1] - ph / 2.0).clamp(4.0, (height - ph - 4.0).max(4.0));

        let mut out = format!("<g class=\"p pe{k} gen{}\" opacity=\"0\">", e.via);
        out.push_str(&format!(
            "<rect class=\"card\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>",
            num(px),
            num(py),
            num(pw),
            num(ph),
        ));
        out.push_str(&format!(
            "<use href=\"#{id}-c{}\" x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{w}\"/>",
            e.to,
            num(px + (pw - cube) / 2.0),
            num(py + 0.7 * st.font),
            w = num(cube),
        ));
        let mut ty = py + 0.7 * st.font + cube + line;
        out.push_str(&format!(
            "<text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
             text-anchor=\"middle\" fill=\"{}\">{}</text>",
            num(px + pw / 2.0),
            num(ty),
            num(st.font * 1.05),
            self.colour(e.via),
            escape(&self.gens[e.via]),
        ));
        ty += line * 0.5;
        for (key, value) in &rows {
            ty += line;
            out.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>\
                 <text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"end\">{}</text>",
                num(px + st.font),
                num(ty),
                num(st.font),
                escape(key),
                num(px + pw - st.font),
                num(ty),
                num(st.font),
                escape(value),
            ));
        }
        out.push_str("</g>");
        out
    }

    fn vertex_markup(&self, v: usize, id: &str, at: P2, st: &Style, app: Option<&str>) -> String {
        let vertex = &self.vertices[v];
        let (x, y) = (at[0] - st.node / 2.0, at[1] - st.node / 2.0);
        let data = if app.is_some() {
            format!(
                " data-state=\"{}\" data-path=\"{}\"",
                vertex.cube.to_letters(),
                escape(&vertex.path)
            )
        } else {
            String::new()
        };
        let mut s = format!(
            "<g class=\"n n{v}\" tabindex=\"0\"{data}><title>{}</title>",
            escape(&self.tooltip(v)),
        );
        // A transparent box over the cube, so the whole vertex is one hit
        // target even where the cube does not reach the corners.
        s.push_str(&format!(
            "<use href=\"#{id}-c{v}\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{w}\"/>\
             <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{w}\" fill=\"transparent\"/>",
            x = num(x),
            y = num(y),
            w = num(st.node),
        ));
        s.push_str("</g>");
        s
    }

    /// The panel a vertex opens: the cube at full size, then its numbers.
    fn panel_markup(&self, v: usize, id: &str, at: P2, st: &Style, canvas: P2) -> String {
        let vertex = &self.vertices[v];
        let ([x, y], [width, height]) = (at, canvas);
        let cube = st.node * st.zoom;
        let line = 1.55 * st.font;
        let body = vertex
            .detail
            .iter()
            .map(|d| 0.56 * st.font * (d.key.chars().count() + d.value.chars().count() + 3) as f64)
            .fold(cube, f64::max)
            .min(26.0 * st.font);
        let pw = body.max(cube) + 2.0 * st.font;
        let ph = cube + line * (vertex.detail.len() as f64 + 1.6) + 1.4 * st.font;
        // Beside the vertex if there is room, otherwise on its other side, and
        // never past an edge of the figure.
        let px = if x + st.node / 2.0 + 12.0 + pw <= width {
            x + st.node / 2.0 + 12.0
        } else {
            (x - st.node / 2.0 - 12.0 - pw).max(4.0)
        };
        let py = (y - ph / 2.0).clamp(4.0, (height - ph - 4.0).max(4.0));

        let mut s = format!("<g class=\"p p{v}\" opacity=\"0\">");
        s.push_str(&format!(
            "<rect class=\"card\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>",
            num(px),
            num(py),
            num(pw),
            num(ph),
        ));
        s.push_str(&format!(
            "<use href=\"#{id}-c{v}\" x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{w}\"/>",
            num(px + (pw - cube) / 2.0),
            num(py + 0.7 * st.font),
            w = num(cube),
        ));
        let mut ty = py + 0.7 * st.font + cube + line;
        let head = if vertex.path.is_empty() {
            "solved".to_string()
        } else {
            vertex.path.clone()
        };
        s.push_str(&format!(
            "<text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
             text-anchor=\"middle\">{}</text>",
            num(px + pw / 2.0),
            num(ty),
            num(st.font * 1.05),
            escape(&head),
        ));
        ty += line * 0.5;
        for d in &vertex.detail {
            ty += line;
            s.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>\
                 <text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"end\">{}</text>",
                num(px + st.font),
                num(ty),
                num(st.font),
                escape(&d.key),
                num(px + pw - st.font),
                num(ty),
                num(st.font),
                escape(&d.value),
            ));
        }
        s.push_str("</g>");
        s
    }

    /// The legend names the generators by the colour their edges are drawn in,
    /// and the caption sits under it.
    fn footer(&self, st: &Style, width: f64, height: f64, caption_h: f64, legend_h: f64) -> String {
        let mut s = String::new();
        let mut y = height - caption_h - legend_h + 1.4 * st.font;
        let mut x = (width - self.legend_width(st)) / 2.0;
        for (gi, name) in self.legend_entries() {
            s.push_str(&format!(
                "<path d=\"M{} {}h{}\" stroke=\"{}\" stroke-width=\"2\"/>\
                 <text class=\"v s\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
                num(x),
                num(y - 0.3 * st.font),
                num(1.8 * st.font),
                self.colour(gi),
                num(x + 2.4 * st.font),
                num(y),
                num(st.font),
                escape(&name),
            ));
            x += 2.4 * st.font + text_w(&name, st.font) + 1.8 * st.font;
        }
        y += 1.9 * st.font;
        if !self.caption.is_empty() {
            s.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"middle\">{}</text>",
                num(width / 2.0),
                num(y),
                num(st.font * 1.05),
                escape(&self.caption),
            ));
        }
        s
    }

    fn legend_width(&self, st: &Style) -> f64 {
        self.legend_entries()
            .iter()
            .map(|(_, name)| 2.4 * st.font + text_w(name, st.font) + 1.8 * st.font)
            .sum::<f64>()
            - 1.8 * st.font
    }

    /// The colour of generator `gi`'s edges: one per kind of edge, in the
    /// order the kinds first appear, so `U` and `U'` share one and the next
    /// pair takes the next.
    fn colour(&self, gi: usize) -> &'static str {
        let head = match self.partner[gi] {
            Some(j) if j < gi => j,
            _ => gi,
        };
        let slot = (0..head)
            .filter(|&k| !matches!(self.partner[k], Some(j) if j < k))
            .count();
        WIRES[slot % WIRES.len()]
    }

    /// What the legend lists: one entry per adjacency, so a generator and the
    /// generator that undoes it share a line and a colour.
    fn legend_entries(&self) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (gi, name) in self.gens.iter().enumerate() {
            match self.partner[gi] {
                Some(j) if j < gi => continue,
                Some(j) if j > gi => out.push((gi, format!("{name} / {}", self.gens[j]))),
                _ => out.push((gi, name.clone())),
            }
        }
        out
    }

    fn tooltip(&self, v: usize) -> String {
        let vertex = &self.vertices[v];
        let mut s = if vertex.path.is_empty() {
            "solved".to_string()
        } else {
            vertex.path.clone()
        };
        for d in &vertex.detail {
            s.push_str(&format!("\n{}: {}", d.key, d.value));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ud() -> Cayley {
        cayley(3, &["U", "D"], DEFAULT_LIMIT).expect("a group of sixteen")
    }

    #[test]
    fn u_and_d_generate_a_group_of_sixteen() {
        let g = ud();
        assert_eq!(g.len(), 16);
        // The graph is the one these generators draw, and U' is not among
        // them, so U^3 costs three. The elements are U^i D^j at distance i+j.
        assert_eq!(g.diameter(), 6);
        assert_eq!(g.shells(), vec![1, 2, 3, 4, 3, 2, 1]);
        assert_eq!(g.shells().iter().sum::<usize>(), g.len());
    }

    #[test]
    fn handing_it_the_inverses_too_gives_the_undirected_graph() {
        let g = cayley(3, &["U", "U'", "D", "D'"], DEFAULT_LIMIT).expect("still sixteen");
        assert_eq!(g.len(), 16, "the same group, reached a different way");
        // Now two turns in each coordinate reach everything: C4 x C4 at last
        // looks like the torus it is.
        assert_eq!(g.diameter(), 4);
        assert_eq!(g.shells(), vec![1, 4, 6, 4, 1]);
        // U' undoes U, so each pair is one adjacency drawn once: 16 vertices
        // with two adjacencies each, counted once, is 32 edges rather than 64.
        assert_eq!(g.partner, vec![Some(1), Some(0), Some(3), Some(2)]);
        assert_eq!(g.edges.len(), 32);
    }

    #[test]
    fn the_half_turns_of_u_and_r_make_a_ring_of_twelve() {
        let g = cayley(3, &["U2", "R2"], DEFAULT_LIMIT).expect("a group of twelve");
        assert_eq!(g.len(), 12);
        assert!(
            g.partner.iter().enumerate().all(|(i, p)| *p == Some(i)),
            "both generators are their own inverse"
        );
        // Every vertex has one edge of each colour, counted once.
        assert_eq!(g.edges.len(), 12);
        // Alternating them walks the whole group: a devil's algorithm.
        assert_eq!(g.ring_walk().map(|w| w.len()), Some(12));
    }

    #[test]
    fn every_vertex_is_reached_by_a_shortest_word() {
        let g = ud();
        for v in &g.vertices {
            let moves = notation::parse(3, &v.path).expect("the path parses");
            let mut cube = Cube::solved(3);
            cube.apply_moves(&moves);
            assert_eq!(cube, v.cube, "the path does not reach its own vertex");
        }
        assert_eq!(g.vertices[0].path, "", "the identity is vertex zero");
    }

    #[test]
    fn the_step_table_agrees_with_the_edges() {
        let g = ud();
        for (v, row) in g.step.iter().enumerate() {
            for (gi, &u) in row.iter().enumerate() {
                let drawn = g.edges.iter().any(|e| {
                    e.via == gi && ((e.from == v && e.to == u) || (e.to == v && e.from == u))
                });
                assert!(drawn || u == v, "the edge {v} -{gi}-> {u} went missing");
            }
        }
    }

    #[test]
    fn a_grid_layout_is_used_when_the_generators_commute() {
        let g = ud().with_layout(Layout::Grid);
        let cell = Style::default().node * g.spread;
        assert!(g.grid_places(cell).is_some(), "C4 x C4 is a torus");
        // Four rows of four, so four distinct x and four distinct y.
        let pos = g.places(&Style::default());
        let round = |v: f64| (v * 100.0).round() as i64;
        let mut xs: Vec<i64> = pos.iter().map(|p| round(p[0])).collect();
        xs.sort_unstable();
        xs.dedup();
        assert_eq!(xs.len(), 4);
    }

    #[test]
    fn a_grid_falls_back_when_the_generators_do_not_commute() {
        let g = cayley(3, &["U2", "R2"], DEFAULT_LIMIT)
            .expect("a group of twelve")
            .with_layout(Layout::Grid);
        let cell = Style::default().node * g.spread;
        assert!(g.grid_places(cell).is_none(), "U2 and R2 do not commute");
        assert_eq!(g.places(&Style::default()).len(), g.len());
    }

    #[test]
    fn shells_put_the_identity_in_the_middle_and_the_rest_outside() {
        let g = ud();
        let st = Style::default();
        let pos = g.places(&st);
        assert_eq!(pos[0], [0.0, 0.0], "the solved cube is the centre");
        for (v, p) in pos.iter().enumerate().skip(1) {
            let r = p[0].hypot(p[1]);
            assert!(r > 0.0, "vertex {v} sits on top of the identity");
        }
        // A vertex further out is drawn further out.
        for a in &g.vertices {
            for b in &g.vertices {
                if a.dist >= b.dist {
                    continue;
                }
                let ra = pos[g.vertices.iter().position(|v| v == a).unwrap()];
                let rb = pos[g.vertices.iter().position(|v| v == b).unwrap()];
                assert!(ra[0].hypot(ra[1]) < rb[0].hypot(rb[1]) + 1e-9);
            }
        }
    }

    #[test]
    fn svg_has_one_cube_and_one_panel_per_vertex() {
        let g = ud().with_caption("a sample");
        let svg = g.svg(&Style::default());
        assert!(svg.starts_with("<svg "), "an svg document");
        assert!(svg.ends_with("</svg>"));
        assert_eq!(svg.matches("<symbol ").count(), g.len());
        // One <use> at the vertex and one in the panel, per vertex.
        // Two uses a vertex, one at the vertex and one in its panel, plus one
        // in each edge's panel for the cube that edge leads to.
        assert_eq!(svg.matches("<use ").count(), 2 * g.len() + g.edges.len());
        assert_eq!(svg.matches("class=\"p pe").count(), g.edges.len());
        assert_eq!(
            svg.matches("class=\"p p").count() - svg.matches("class=\"p pe").count(),
            g.len()
        );
        assert!(svg.contains(":has("), "panels open on hover or focus");
        assert!(svg.contains("tabindex=\"0\""), "and on the keyboard");
        assert!(!svg.contains("<script"), "the svg carries no script");
    }

    #[test]
    fn edges_are_drawn_once_each_and_coloured_by_generator() {
        let g = ud();
        let svg = g.svg(&Style::default());
        assert_eq!(svg.matches("<g class=\"e e").count(), g.edges.len());
        for gi in 0..g.gens.len() {
            assert!(svg.contains(g.colour(gi)), "generator {gi} has no colour");
        }
    }

    #[test]
    fn a_generator_that_is_not_its_own_inverse_gets_an_arrow() {
        let g = ud();
        assert_eq!(g.partner[0], None, "U' is not in this generating set");
        let svg = g.svg(&Style::default());
        assert!(svg.contains("<marker "), "a directed edge needs a head");
        assert!(svg.contains("marker-end="));
        let ring = cayley(3, &["U2", "R2"], DEFAULT_LIMIT).expect("twelve");
        assert!(
            !ring.svg(&Style::default()).contains("<marker "),
            "an involution is one undirected edge, so no arrow"
        );
        let both = cayley(3, &["U", "U'", "D", "D'"], DEFAULT_LIMIT).expect("sixteen");
        assert!(
            !both.svg(&Style::default()).contains("<marker "),
            "a generating set closed under inverses draws lines, not arrows"
        );
    }

    #[test]
    fn an_edge_can_be_landed_on_and_says_where_it_goes() {
        let g = ud();
        let svg = g.svg(&Style::default());
        // A two pixel curve is hard to hit, so each edge carries a fat
        // transparent twin, and a panel that opens the way a vertex's does.
        assert_eq!(svg.matches("stroke=\"transparent\"").count(), g.edges.len());
        assert_eq!(svg.matches("class=\"e e").count(), g.edges.len());
        assert!(
            svg.contains(":has(.e0:hover) .pe0"),
            "an edge opens its panel"
        );
        assert!(
            svg.contains("\u{2192}"),
            "and its tooltip says what leads where"
        );
    }

    #[test]
    fn every_edge_is_tagged_with_the_move_it_is() {
        let g = cayley(3, &["U", "D"], DEFAULT_LIMIT).expect("sixteen");
        let svg = g.svg(&Style::default());
        for gi in 0..g.gens.len() {
            let drawn = g.edges.iter().filter(|e| e.via == gi).count();
            assert_eq!(
                svg.matches(&format!("gen{gi}\"")).count(),
                2 * drawn,
                "each edge of generator {gi} tags both itself and its panel"
            );
        }
    }

    #[test]
    fn the_switches_hide_one_move_at_a_time() {
        let g = ud();
        let html = g.html(&Style::default(), None);
        assert_eq!(
            html.matches("type=\"checkbox\"").count(),
            g.legend_entries().len(),
            "one switch per kind of edge"
        );
        assert!(html.contains(":not(:checked)) .gen0{display:none}"));
        assert!(
            !html.contains("<script"),
            "the switches are css, not script"
        );
        // One generator is no choice at all.
        let lone = cayley(3, &["U"], DEFAULT_LIMIT).expect("four");
        assert!(!lone.html(&Style::default(), None).contains("checkbox"));
    }

    #[test]
    fn a_shell_guide_encloses_its_own_cubes() {
        // A guide drawn through the centres cuts every cube on the ring in
        // half, and the ones on the last ring look like they escaped.
        let st = Style::default();
        let g = cayley(3, &["U", "D"], DEFAULT_LIMIT)
            .expect("sixteen")
            .with_layout(Layout::Shells);
        let svg = g.svg(&st);
        let mut guides = Vec::new();
        for part in svg.split("<circle class=\"ring\" ").skip(1) {
            let at = part.find("r=\"").expect("the circle has a radius") + 3;
            guides.push(
                part[at..]
                    .split('"')
                    .next()
                    .expect("a value")
                    .parse::<f64>()
                    .expect("a number"),
            );
        }
        assert_eq!(guides.len(), g.diameter() + 1, "a guide for every shell");

        // Shell positions are centred on the origin, so a vertex's reach is
        // its radius plus half the box its cube is drawn in.
        for (v, p) in g.places(&st).iter().enumerate() {
            let reach = p[0].hypot(p[1]) + st.node / 2.0;
            let guide = guides[g.vertices[v].dist];
            assert!(
                reach <= guide + 1e-6,
                "vertex {v} on ring {} reaches {reach}, past its guide at {guide}",
                g.vertices[v].dist
            );
        }
    }

    #[test]
    fn rings_cut_the_graph_off_at_that_many_moves() {
        let whole = cayley(3, &["U", "U'", "D", "D'"], DEFAULT_LIMIT).expect("sixteen");
        assert!(whole.complete);
        let ball = cayley_rings(3, &["U", "U'", "D", "D'"], Some(2), DEFAULT_LIMIT)
            .expect("the ball of radius two");
        assert!(!ball.complete, "the group goes on past two rings");
        assert_eq!(ball.diameter(), 2);
        assert_eq!(ball.shells(), whole.shells()[..3].to_vec());
        // Every drawn edge stays inside the ball.
        assert!(
            ball.edges
                .iter()
                .all(|e| e.to < ball.len() && e.from < ball.len())
        );
        assert!(ball.step.iter().flatten().any(|&t| t == OUTSIDE));
        // Enough rings to hold the whole group is the whole group.
        let all = cayley_rings(3, &["U", "U'", "D", "D'"], Some(9), DEFAULT_LIMIT).expect("all");
        assert!(all.complete);
        assert_eq!(all.len(), 16);
    }

    #[test]
    fn two_rings_of_the_whole_cube_group_can_be_drawn() {
        // The six face turns both ways: the quarter-turn Cayley graph of the
        // 3x3x3 itself, 4.3e19 vertices, of which the first two rings hold
        // 1 + 12 + 114.
        let gens = [
            "U", "U'", "D", "D'", "L", "L'", "R", "R'", "F", "F'", "B", "B'",
        ];
        let ball = cayley_rings(3, &gens, Some(2), DEFAULT_LIMIT).expect("127 cubes");
        assert_eq!(ball.shells(), vec![1, 12, 114]);
        assert!(!ball.complete);
        let svg = ball.with_layout(Layout::Shells).svg(&Style::default());
        assert_eq!(svg.matches("<symbol ").count(), 127);
    }

    #[test]
    fn a_ball_that_has_not_closed_is_not_drawn_as_a_torus() {
        let g = cayley_rings(3, &["U", "D"], Some(2), DEFAULT_LIMIT)
            .expect("six")
            .with_layout(Layout::Grid);
        let cell = Style::default().node * g.spread;
        assert!(g.grid_places(cell).is_none());
        assert_eq!(g.places(&Style::default()).len(), g.len());
        assert!(g.ring_walk().is_none());
    }

    #[test]
    fn html_adds_a_player_only_when_an_app_is_given() {
        let g = ud();
        let st = Style::default();
        let plain = g.html(&st, None);
        assert!(!plain.contains("<iframe"));
        assert!(!plain.contains("<script"));
        assert!(
            plain.contains("overflow:auto"),
            "a big figure scrolls in its pane"
        );
        assert!(!plain.contains("<script"), "scrolling needs no script");
        let full = g.clone().with_pane(Pane::Full).html(&st, None);
        assert!(
            !full.contains("max-height"),
            "full height when asked for it"
        );
        let zoom = g.clone().with_pane(Pane::Zoom(500.0)).html(&st, None);
        assert!(
            zoom.contains("width=\"100%\""),
            "the figure is fitted to the pane"
        );
        assert!(zoom.contains("wheel"), "the wheel zooms into a region");
        assert!(zoom.contains("pointermove"), "and a drag pans");
        let live = g.html(&st, Some("https://example.com/cube/"));
        assert!(live.contains("<iframe"));
        assert!(
            live.contains("loading=\"lazy\""),
            "nothing fetched up front"
        );
        // Every vertex carries the cube it is, and every edge the cube it
        // leads to, so clicking either one opens that state.
        assert_eq!(
            live.matches("data-state=\"").count(),
            g.len() + g.edges.len()
        );
    }

    #[test]
    fn a_group_too_big_to_draw_says_so_instead_of_drawing_it() {
        let err = cayley(3, &["U", "R"], 200).expect_err("<U,R> is 73 million");
        assert!(err.contains("200"), "the message says what the limit was");
        assert!(
            err.contains("raise"),
            "and that the limit is the caller's to move"
        );
    }

    #[test]
    fn nonsense_generators_are_refused() {
        assert!(cayley(3, &[], DEFAULT_LIMIT).is_err(), "no generators");
        assert!(cayley(3, &["Q"], DEFAULT_LIMIT).is_err(), "not a move");
        assert!(
            cayley(3, &["U U'"], DEFAULT_LIMIT).is_err(),
            "a word that does nothing generates nothing"
        );
    }

    #[test]
    fn two_figures_do_not_share_a_css_scope() {
        let st = Style::default();
        let one = ud().with_caption("first").svg(&st);
        let two = ud().with_caption("second").svg(&st);
        let id = |s: &str| s[s.find("#cg").unwrap()..][..8].to_string();
        assert_ne!(id(&one), id(&two));
    }
}
