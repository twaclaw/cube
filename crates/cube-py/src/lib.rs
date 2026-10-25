//! Python bindings: the same drawings the app shows, for a notebook.
//!
//! Everything returns a [`Drawing`], which is an SVG string that notebooks
//! display inline and that can be saved next to a blog post.

use cube_core::export;
use cube_core::geom::{Face, facelet_from_label, facelet_label};
use cube_core::palette::{BLANK, Palette, Rgb, STANDARD};
use cube_core::pieces::{self, Kind};
use cube_core::render::{
    Camera, CycleStyle, Facelet, Marks, NetStyle, Style3d, cube3d, cycles, net,
};
use cube_core::scramble::{Rng, scramble};
use cube_core::solver::tables::{N_MOVES, move_name};
use cube_core::solver::{Kociemba, SolveError, search::Limits};
use cube_core::{Cube as CoreCube, Permutation as CorePermutation, notation};
use cube_graphs as graphs;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Colours a mark may be named with, beyond `#rrggbb`.
const NAMED_COLORS: &[(&str, u32)] = &[
    ("red", 0xd1365a),
    ("green", 0x009e60),
    ("blue", 0x0051ba),
    ("yellow", 0xffd500),
    ("orange", 0xff5800),
    ("white", 0xfdfdfd),
    ("black", 0x1a1a1f),
    ("gray", 0x8a8a96),
    ("grey", 0x8a8a96),
    ("magenta", 0xd10a6e),
    ("pink", 0xff86b3),
    ("purple", 0x8450c8),
    ("cyan", 0x00b2c4),
    ("teal", 0x009a8f),
    ("lime", 0x7ac70c),
    ("gold", 0xe0a800),
    ("navy", 0x18356b),
];

/// Colours used for orbits when none are given.
const ORBIT_COLORS: &[&str] = &[
    "red", "blue", "green", "orange", "purple", "teal", "gold", "pink", "navy", "lime",
];

fn parse_color(text: &str) -> PyResult<Rgb> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        let value = u32::from_str_radix(hex, 16)
            .map_err(|_| PyValueError::new_err(format!("{text} is not a colour")))?;
        return Ok(match hex.len() {
            6 => Rgb::from_hex(value),
            3 => {
                let (r, g, b) = (value >> 8 & 0xf, value >> 4 & 0xf, value & 0xf);
                Rgb::from_hex(r * 0x110000 + g * 0x1100 + b * 0x11)
            }
            _ => return Err(PyValueError::new_err(format!("{text} is not a colour"))),
        });
    }
    NAMED_COLORS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(text))
        .map(|(_, hex)| Rgb::from_hex(*hex))
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "no colour called {text}: use a name or #rrggbb"
            ))
        })
}

/// A key of a marks dict: a facelet number, or a cubie by its letters.
fn facelets_of_key(n: u8, key: &Bound<'_, PyAny>) -> PyResult<Vec<Facelet>> {
    if let Ok(number) = key.extract::<u32>() {
        return facelet_from_label(n, number)
            .map(|f| vec![f])
            .ok_or_else(|| PyKeyError::new_err(format!("this cube has no facelet {number}")));
    }
    let text: String = key
        .cast::<PyString>()
        .map_err(|_| PyKeyError::new_err("mark keys are facelet numbers or cubie names"))?
        .extract()?;
    if let Ok(number) = text.trim().parse::<u32>() {
        return facelet_from_label(n, number)
            .map(|f| vec![f])
            .ok_or_else(|| PyKeyError::new_err(format!("this cube has no facelet {number}")));
    }
    let slot = pieces::named(n, &text)
        .ok_or_else(|| PyKeyError::new_err(format!("no cubie called {text}")))?;
    Ok(pieces::facelets(n, slot))
}

/// `{"UFR": "red", 25: "#0051ba"}` as something the renderers understand.
fn marks_from(n: u8, fills: Option<&Bound<'_, PyDict>>, rings: Option<&Bound<'_, PyDict>>) -> PyResult<Marks> {
    let mut marks = Marks::new();
    for (source, is_fill) in [(fills, true), (rings, false)] {
        let Some(dict) = source else { continue };
        for (key, value) in dict.iter() {
            let color = parse_color(&value.extract::<String>()?)?;
            for facelet in facelets_of_key(n, &key)? {
                if is_fill {
                    marks.paint(facelet, color);
                } else {
                    marks.ring_at(facelet, color);
                }
            }
        }
    }
    Ok(marks)
}

fn palette(blank: bool) -> Palette {
    if blank { BLANK } else { STANDARD }
}

fn parse_moves(n: u8, word: &str) -> PyResult<Vec<cube_core::Move>> {
    notation::parse(n, word).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// One drawing, ready to show in a notebook or save beside a post.
#[pyclass(module = "magiccube")]
#[derive(Clone)]
pub struct Drawing {
    text: String,
    is_svg: bool,
}

#[pymethods]
impl Drawing {
    /// The SVG, or the HTML for a player.
    #[getter]
    fn text(&self) -> &str {
        &self.text
    }

    #[getter]
    fn svg(&self) -> &str {
        &self.text
    }

    /// Write it to a file and return the path, for `![](that path)`.
    fn save(&self, path: &str) -> PyResult<String> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &self.text)?;
        Ok(path.to_string())
    }

    fn _repr_html_(&self) -> String {
        if self.is_svg {
            return self.text.clone();
        }
        let quoted = self.text.replace('&', "&amp;").replace('"', "&quot;");
        format!("<iframe srcdoc=\"{quoted}\" width=\"100%\" height=\"460\" style=\"border:0\"></iframe>")
    }

    fn __repr__(&self) -> String {
        format!(
            "<Drawing {} {} bytes>",
            if self.is_svg { "svg" } else { "html" },
            self.text.len()
        )
    }
}

impl Drawing {
    fn svg_of(text: String) -> Self {
        Self {
            text,
            is_svg: true,
        }
    }
}

/// A state read as a permutation of the numbered facelets.
#[pyclass(module = "magiccube")]
#[derive(Clone)]
pub struct Permutation {
    inner: CorePermutation,
    n: u8,
}

#[pymethods]
impl Permutation {
    /// The disjoint cycles, as lists of facelet numbers.
    #[getter]
    fn cycles(&self) -> Vec<Vec<u32>> {
        self.inner.cycles()
    }

    /// How often it must repeat to come back to the start.
    #[getter]
    fn order(&self) -> u64 {
        self.inner.order()
    }

    /// The cycle lengths, longest first.
    #[getter]
    fn cycle_type(&self) -> Vec<usize> {
        self.inner.cycle_type()
    }

    /// How many facelets are not where they started.
    #[getter]
    fn moved(&self) -> usize {
        self.inner.moved()
    }

    #[getter]
    fn notation(&self) -> String {
        self.inner.to_cycle_notation()
    }

    /// The cycles drawn, every number as the facelet it stands for.
    #[pyo3(signature = (width=760.0, background=None))]
    fn draw(&self, width: f64, background: Option<&str>) -> Drawing {
        let style = CycleStyle {
            width,
            ..CycleStyle::default()
        };
        Drawing::svg_of(cycles::document(
            &self.inner,
            self.n,
            &STANDARD,
            &style,
            background,
        ))
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "<Permutation order {} moving {} facelets>",
            self.order(),
            self.moved()
        )
    }
}

/// A two phase solution.
#[pyclass(module = "magiccube")]
#[derive(Clone)]
pub struct Solution {
    #[pyo3(get)]
    phase1: String,
    #[pyo3(get)]
    phase2: String,
    #[pyo3(get)]
    moves: String,
    #[pyo3(get)]
    length: usize,
    #[pyo3(get)]
    nodes: u64,
}

#[pymethods]
impl Solution {
    fn __repr__(&self) -> String {
        format!(
            "<Solution {} moves: {} + {}>",
            self.length,
            self.phase1.split_whitespace().count(),
            self.phase2.split_whitespace().count()
        )
    }

    fn __str__(&self) -> String {
        self.moves.clone()
    }
}

fn solver() -> &'static Kociemba {
    static SOLVER: OnceLock<Kociemba> = OnceLock::new();
    SOLVER.get_or_init(Kociemba::new)
}

/// A cube, drawn the way the app draws it.
#[pyclass(module = "magiccube")]
#[derive(Clone)]
pub struct Cube {
    inner: CoreCube,
}

#[pymethods]
impl Cube {
    /// A solved cube, or one built from a facelet string.
    #[new]
    #[pyo3(signature = (n=3, state=None))]
    fn new(n: u8, state: Option<&str>) -> PyResult<Self> {
        let inner = match state {
            Some(letters) => CoreCube::from_letters(n, letters)
                .ok_or_else(|| PyValueError::new_err("that is not a state of this cube"))?,
            None => CoreCube::solved(n),
        };
        Ok(Self { inner })
    }

    /// A random state, reproducible from `seed`.
    #[staticmethod]
    #[pyo3(signature = (seed=0, moves=25, n=3))]
    fn scrambled(seed: u64, moves: usize, n: u8) -> Self {
        let mut inner = CoreCube::solved(n);
        inner.apply_moves(&scramble(n, moves, &mut Rng::new(seed)));
        Self { inner }
    }

    #[getter]
    fn n(&self) -> u8 {
        self.inner.n()
    }

    /// The facelet colours as letters, which `Cube(state=...)` reads back.
    #[getter]
    fn state(&self) -> String {
        self.inner.to_letters()
    }

    #[getter]
    fn is_solved(&self) -> bool {
        self.inner.is_solved()
    }

    /// Turn the cube. Returns itself, so calls can be chained.
    fn apply(mut slf: PyRefMut<'_, Self>, word: &str) -> PyResult<Py<Self>> {
        let moves = parse_moves(slf.inner.n(), word)?;
        slf.inner.apply_moves(&moves);
        Ok(slf.into())
    }

    /// The cube this maneuver would leave, without touching this one.
    fn moved(&self, word: &str) -> PyResult<Self> {
        let mut inner = self.inner.clone();
        inner.apply_moves(&parse_moves(inner.n(), word)?);
        Ok(Self { inner })
    }

    fn copy(&self) -> Self {
        self.clone()
    }

    /// The flat net.
    #[pyo3(signature = (numbers=false, marks=None, rings=None, blank=false, cell=34.0, background=None))]
    fn net(
        &self,
        numbers: bool,
        marks: Option<&Bound<'_, PyDict>>,
        rings: Option<&Bound<'_, PyDict>>,
        blank: bool,
        cell: f64,
        background: Option<&str>,
    ) -> PyResult<Drawing> {
        let style = NetStyle {
            numbers,
            cell,
            ..NetStyle::default()
        };
        let marks = marks_from(self.inner.n(), marks, rings)?;
        Ok(Drawing::svg_of(net::document(
            &self.inner,
            &style,
            &palette(blank),
            &marks,
            background,
        )))
    }

    /// The cube in three dimensions.
    #[pyo3(signature = (yaw=38.0, pitch=24.0, zoom=1.0, marks=None, rings=None, blank=false,
                        size=300.0, flat=false, background=None))]
    #[allow(clippy::too_many_arguments)]
    fn view3d(
        &self,
        yaw: f64,
        pitch: f64,
        zoom: f64,
        marks: Option<&Bound<'_, PyDict>>,
        rings: Option<&Bound<'_, PyDict>>,
        blank: bool,
        size: f64,
        flat: bool,
        background: Option<&str>,
    ) -> PyResult<Drawing> {
        let camera = Camera { yaw, pitch, zoom }.clamped();
        let style = Style3d {
            size,
            shade: if flat { 0.0 } else { Style3d::default().shade },
            ..Style3d::default()
        };
        let marks = marks_from(self.inner.n(), marks, rings)?;
        Ok(Drawing::svg_of(cube3d::document(
            &self.inner,
            &camera,
            None,
            &style,
            &palette(blank),
            &marks,
            background,
        )))
    }

    /// The state as a permutation, if the colours name one.
    fn permutation(&self) -> PyResult<Permutation> {
        CorePermutation::of(&self.inner)
            .map(|inner| Permutation {
                inner,
                n: self.inner.n(),
            })
            .ok_or_else(|| {
                PyValueError::new_err(
                    "a cube this size has interchangeable stickers, so its state does not name \
                     one permutation",
                )
            })
    }

    /// The cycles drawn straight away.
    #[pyo3(signature = (width=760.0, background=None))]
    fn cycles(&self, width: f64, background: Option<&str>) -> PyResult<Drawing> {
        Ok(self.permutation()?.draw(width, background))
    }

    /// Solve it with Kociemba's two phase algorithm.
    fn solve(&self) -> PyResult<Solution> {
        if self.inner.n() != 3 {
            return Err(PyValueError::new_err("the solver knows the 3x3x3 only"));
        }
        match solver().solve_with(&self.inner, Limits::default()) {
            Ok(found) => Ok(Solution {
                phase1: notation::format(&found.phase1),
                phase2: notation::format(&found.phase2),
                moves: notation::format(&found.moves()),
                length: found.len(),
                nodes: found.nodes,
            }),
            Err(SolveError::Unreachable) => Err(PyValueError::new_err(
                "these colours are not a cube that turning faces can reach",
            )),
            Err(e) => Err(PyValueError::new_err(e.to_string())),
        }
    }

    /// The cubies a maneuver leaves where they are, as a marks dict.
    #[pyo3(signature = (word, color="green"))]
    fn fixed(&self, word: &str, color: &str) -> PyResult<BTreeMap<String, String>> {
        let n = self.inner.n();
        let moves = parse_moves(n, word)?;
        Ok(named_marks(n, &pieces::fixed(n, &moves), color))
    }

    /// The cubies a maneuver carries elsewhere, as a marks dict.
    #[pyo3(signature = (word, color="red"))]
    fn disturbed(&self, word: &str, color: &str) -> PyResult<BTreeMap<String, String>> {
        let n = self.inner.n();
        let moves = parse_moves(n, word)?;
        Ok(named_marks(n, &pieces::disturbed(n, &moves), color))
    }

    fn __repr__(&self) -> String {
        let n = self.inner.n();
        format!(
            "<Cube {n}x{n}x{n} {}>",
            if self.inner.is_solved() {
                "solved"
            } else {
                "scrambled"
            }
        )
    }
}

fn named_marks(n: u8, slots: &[pieces::Slot], color: &str) -> BTreeMap<String, String> {
    slots
        .iter()
        .map(|slot| (pieces::name(n, *slot), color.to_string()))
        .collect()
}

fn of_kind(kind: Kind, color: &str, n: u8) -> BTreeMap<String, String> {
    named_marks(n, &pieces::of_kind(n, kind), color)
}

/// Every corner cubie, as a marks dict.
#[pyfunction]
#[pyo3(signature = (color="red", n=3))]
fn corners(color: &str, n: u8) -> BTreeMap<String, String> {
    of_kind(Kind::Corner, color, n)
}

/// Every edge cubie, as a marks dict.
#[pyfunction]
#[pyo3(signature = (color="red", n=3))]
fn edges(color: &str, n: u8) -> BTreeMap<String, String> {
    of_kind(Kind::Edge, color, n)
}

/// Every centre cubie, as a marks dict.
#[pyfunction]
#[pyo3(signature = (color="red", n=3))]
fn centers(color: &str, n: u8) -> BTreeMap<String, String> {
    of_kind(Kind::Center, color, n)
}

/// Named cubies, as a marks dict.
#[pyfunction]
#[pyo3(signature = (names, color="red", n=3))]
fn cubies(names: Vec<String>, color: &str, n: u8) -> PyResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for name in names {
        let slot = pieces::named(n, &name)
            .ok_or_else(|| PyKeyError::new_err(format!("no cubie called {name}")))?;
        out.insert(pieces::name(n, slot), color.to_string());
    }
    Ok(out)
}

/// Facelets by number, as a marks dict.
#[pyfunction]
#[pyo3(signature = (numbers, color="red", n=3))]
fn facelets(numbers: Vec<u32>, color: &str, n: u8) -> PyResult<BTreeMap<u32, String>> {
    for number in &numbers {
        if facelet_from_label(n, *number).is_none() {
            return Err(PyKeyError::new_err(format!("no facelet {number}")));
        }
    }
    Ok(numbers
        .into_iter()
        .map(|number| (number, color.to_string()))
        .collect())
}

/// The cubies each maneuver in `generators` can shuffle among themselves, one
/// colour per orbit. Colouring the orbits of `U D R2 L2 F2 B2` shows what
/// phase 2 of Kociemba's algorithm can and cannot move.
#[pyfunction]
#[pyo3(signature = (generators, colors=None, n=3, skip_singletons=true))]
fn orbits(
    generators: Vec<String>,
    colors: Option<Vec<String>>,
    n: u8,
    skip_singletons: bool,
) -> PyResult<BTreeMap<String, String>> {
    let parsed: Result<Vec<_>, _> = generators.iter().map(|w| parse_moves(n, w)).collect();
    let groups = pieces::orbits(n, &parsed?);
    let palette: Vec<String> = colors.unwrap_or_else(|| {
        ORBIT_COLORS
            .iter()
            .map(|name| (*name).to_string())
            .collect()
    });
    let mut out = BTreeMap::new();
    for (i, group) in groups
        .iter()
        .filter(|group| !(skip_singletons && group.len() == 1))
        .enumerate()
    {
        let color = palette[i % palette.len()].clone();
        for slot in group {
            out.insert(pieces::name(n, *slot), color.clone());
        }
    }
    Ok(out)
}

/// The number every facelet carries in the net, by cubie.
#[pyfunction]
#[pyo3(signature = (name, n=3))]
fn facelet_numbers(name: &str, n: u8) -> PyResult<Vec<u32>> {
    let slot =
        pieces::named(n, name).ok_or_else(|| PyKeyError::new_err(format!("no cubie {name}")))?;
    Ok(pieces::facelets(n, slot)
        .into_iter()
        .filter_map(|(face, row, col)| facelet_label(n, face, row, col))
        .collect())
}

/// Every state of a maneuver, side by side in one SVG.
#[pyfunction]
#[pyo3(signature = (cube, word, columns=6, yaw=38.0, pitch=24.0, size=200.0, background=None))]
#[allow(clippy::too_many_arguments)]
fn strip(
    cube: &Cube,
    word: &str,
    columns: usize,
    yaw: f64,
    pitch: f64,
    size: f64,
    background: Option<&str>,
) -> PyResult<Drawing> {
    let moves = parse_moves(cube.inner.n(), word)?;
    let camera = Camera {
        yaw,
        pitch,
        zoom: 1.0,
    }
    .clamped();
    let style = Style3d {
        size,
        ..Style3d::default()
    };
    Ok(Drawing::svg_of(export::strip_svg(
        &cube.inner,
        &moves,
        &camera,
        &style,
        &STANDARD,
        &Marks::new(),
        columns,
        background,
    )))
}

fn timeline(cube: &Cube, word: &str, steps: usize, ms: f64) -> PyResult<export::Timeline> {
    let moves = parse_moves(cube.inner.n(), word)?;
    Ok(export::timeline(
        &cube.inner,
        &moves,
        &Camera::default(),
        &Style3d::default(),
        &STANDARD,
        &Marks::new(),
        steps,
        ms,
    ))
}

/// A maneuver as an animated SVG, which works inside an `<img>`.
#[pyfunction]
#[pyo3(signature = (cube, word, steps=8, ms=420.0, background=None))]
fn animation(
    cube: &Cube,
    word: &str,
    steps: usize,
    ms: f64,
    background: Option<&str>,
) -> PyResult<Drawing> {
    Ok(Drawing::svg_of(export::animated_svg(
        &timeline(cube, word, steps, ms)?,
        background,
    )))
}

/// A maneuver as a self contained HTML page with play, step and scrub.
#[pyfunction]
#[pyo3(signature = (cube, word, steps=8, ms=420.0, title="Rubik's cube"))]
fn player(cube: &Cube, word: &str, steps: usize, ms: f64, title: &str) -> PyResult<Drawing> {
    let line = word.to_string();
    Ok(Drawing {
        text: export::player_html(&timeline(cube, word, steps, ms)?, title, &line),
        is_svg: false,
    })
}

/// The settings a track in a mosaic understands.
const TRACK_KEYS: &[&str] = &[
    "cube", "word", "row", "col", "back", "delay", "ms", "hold", "wrap", "labels", "title",
    "first", "last",
];

fn dict_text(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    match dict.get_item(key)? {
        Some(value) => value.extract(),
        None => Ok(String::new()),
    }
}

fn dict_count(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<usize> {
    match dict.get_item(key)? {
        Some(value) => value.extract(),
        None => Ok(0),
    }
}

fn dict_flag(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<bool> {
    match dict.get_item(key)? {
        Some(value) => value.extract(),
        None => Ok(false),
    }
}

fn dict_number(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<f64> {
    match dict.get_item(key)? {
        Some(value) => value.extract(),
        None => Ok(0.0),
    }
}

/// A duration the track may leave to the figure.
fn dict_ms(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    match dict.get_item(key)? {
        Some(value) if !value.is_none() => value.extract().map(Some),
        _ => Ok(None),
    }
}

fn dict_list(dict: &Bound<'_, PyDict>, key: &str) -> PyResult<Vec<String>> {
    match dict.get_item(key)? {
        Some(value) => value.extract(),
        None => Ok(Vec::new()),
    }
}

/// One row of a mosaic, read off the dict that describes it.
fn mosaic_track(item: &Bound<'_, PyAny>) -> PyResult<export::mosaic::Track> {
    let dict = item
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("every track is a dict"))?;
    for key in dict.keys() {
        let name: String = key.extract()?;
        if !TRACK_KEYS.contains(&name.as_str()) {
            return Err(PyKeyError::new_err(format!(
                "no track setting called {name}: one of {}",
                TRACK_KEYS.join(", ")
            )));
        }
    }
    let value = dict
        .get_item("cube")?
        .ok_or_else(|| PyKeyError::new_err("every track wants a cube to start from"))?;
    let cube = value
        .cast::<Cube>()
        .map_err(|_| PyValueError::new_err("a track's cube is a Cube"))?
        .borrow()
        .inner
        .clone();
    let moves = parse_moves(cube.n(), &dict_text(dict, "word")?)?;
    let back = dict_flag(dict, "back")?;
    // A backward track is given the state its moves arrive at and undoes them
    // one at a time, which is the half of a search that starts from the goal.
    let mut track = if back {
        export::mosaic::Track::rewound(cube, &moves)
    } else {
        export::mosaic::Track::new(cube, moves)
    };
    track.row = dict_count(dict, "row")?;
    track.col = dict_count(dict, "col")?;
    track.delay = dict_number(dict, "delay")?;
    track.ms = dict_ms(dict, "ms")?;
    track.hold = dict_ms(dict, "hold")?;
    track.wrap = dict_flag(dict, "wrap")?;
    track.labels = dict_list(dict, "labels")?;
    track.title = dict_text(dict, "title")?;
    track.first = dict_text(dict, "first")?;
    track.last = dict_text(dict, "last")?;
    if back && track.col < track.moves.len() {
        return Err(PyValueError::new_err(format!(
            "a track filling leftwards from column {} has room for {} moves, not {}",
            track.col,
            track.col,
            track.moves.len()
        )));
    }
    Ok(track)
}

/// The rows a mosaic draws, and the maneuver to write under it.
fn mosaic_plan(
    cube: Option<&Cube>,
    word: &str,
    tracks: Option<&Bound<'_, PyList>>,
    meet: Option<&str>,
    pause: f64,
    titles: Option<Vec<String>>,
) -> PyResult<(Vec<export::mosaic::Track>, String)> {
    let (mut rows, caption) = if let Some(list) = tracks {
        let rows = list
            .iter()
            .map(|item| mosaic_track(&item))
            .collect::<PyResult<Vec<_>>>()?;
        let longest = rows
            .iter()
            .max_by_key(|t| t.moves.len())
            .map(|t| notation::format(&t.moves))
            .unwrap_or_default();
        (rows, longest)
    } else {
        let cube = cube.ok_or_else(|| {
            PyValueError::new_err("a mosaic wants a cube and a maneuver, or a list of tracks")
        })?;
        let n = cube.inner.n();
        let forward = parse_moves(n, word)?;
        match meet {
            Some(second) => {
                let backward = parse_moves(n, second)?;
                let whole = notation::format(&[forward.clone(), backward.clone()].concat());
                (
                    export::mosaic::meet(&cube.inner, &forward, &backward, pause),
                    whole,
                )
            }
            None => (
                vec![export::mosaic::Track::new(cube.inner.clone(), forward).wrapping()],
                word.trim().to_string(),
            ),
        }
    };
    // Row names given here win over the ones the figure chose; a blank one
    // leaves that row as it was.
    for (row, title) in rows.iter_mut().zip(titles.unwrap_or_default()) {
        if !title.is_empty() {
            row.title = title;
        }
    }
    Ok((rows, caption))
}

#[allow(clippy::too_many_arguments)]
fn mosaic_timeline(
    cube: Option<&Cube>,
    word: &str,
    tracks: Option<&Bound<'_, PyList>>,
    meet: Option<&str>,
    pause: f64,
    titles: Option<Vec<String>>,
    columns: usize,
    steps: usize,
    ms: f64,
    hold: Option<f64>,
    size: f64,
    yaw: f64,
    pitch: f64,
    numbered: bool,
    ghosts: bool,
) -> PyResult<(export::Timeline, String)> {
    let (rows, caption) = mosaic_plan(cube, word, tracks, meet, pause, titles)?;
    let camera = Camera {
        yaw,
        pitch,
        zoom: 1.0,
    }
    .clamped();
    let style = Style3d {
        size,
        ..Style3d::default()
    };
    let layout = export::mosaic::Layout {
        columns: columns.max(1),
        numbered,
        ghosts,
        ..export::mosaic::Layout::default()
    };
    let pace = export::mosaic::Pace { ms, hold, steps };
    let tl = export::mosaic::timeline(
        &rows,
        &camera,
        &style,
        &STANDARD,
        &Marks::new(),
        &layout,
        &pace,
    );
    Ok((tl, caption))
}

/// A maneuver animated into a grid: every state lands in a cell of its own,
/// and the turn that reaches it plays inside that cell as the cube arrives.
/// The still grid of `strip`, in other words, drawing itself one cube per move.
///
/// Give it `cube` and `word` for one sequence filling a grid `columns` wide;
/// `meet` as well for the two sided figure, where `word` walks forwards from
/// the scramble along the top row, `meet` walks backwards from the solved cube
/// along the second, the two stop in the same column, and a third row then
/// carries the whole solution across; or `tracks` to place the rows yourself:
///
///     [{"cube": c, "word": "R U", "row": 0, "col": 0, "back": False,
///       "delay": 0, "ms": None, "hold": None, "wrap": False,
///       "labels": ["one", "two"], "title": "forwards",
///       "first": "scrambled", "last": "meet"}]
///
/// A backward track is given the state its moves arrive at and undoes them one
/// at a time, filling leftwards from `col`. `delay` holds a row still for that
/// many turns of the figure's pace, which is how a row waits for two others to
/// finish, and a row that sets its own `ms` and `hold` runs at its own speed
/// while sharing the clock.
///
/// `labels` writes something of your own under the cube each move reaches, in
/// place of the move's name, and `title` names the whole row in the margin
/// beside it. `titles` names the rows of a figure you did not build by hand.
/// `ms` is how long a turn takes and `hold` how long a landed state is left up
/// before the next turn starts.
#[pyfunction]
#[pyo3(signature = (cube=None, word="", tracks=None, meet=None, pause=1.0, titles=None,
                    columns=8, steps=6, ms=420.0, hold=None, size=160.0, yaw=38.0,
                    pitch=24.0, numbered=true, ghosts=true, background=None))]
#[allow(clippy::too_many_arguments)]
fn mosaic(
    cube: Option<&Cube>,
    word: &str,
    tracks: Option<&Bound<'_, PyList>>,
    meet: Option<&str>,
    pause: f64,
    titles: Option<Vec<String>>,
    columns: usize,
    steps: usize,
    ms: f64,
    hold: Option<f64>,
    size: f64,
    yaw: f64,
    pitch: f64,
    numbered: bool,
    ghosts: bool,
    background: Option<&str>,
) -> PyResult<Drawing> {
    let (tl, _) = mosaic_timeline(
        cube, word, tracks, meet, pause, titles, columns, steps, ms, hold, size, yaw, pitch,
        numbered, ghosts,
    )?;
    Ok(Drawing::svg_of(export::animated_svg(&tl, background)))
}

/// The same grid as `mosaic`, as a self contained HTML page with play, step
/// and scrub. The arrows step from one landed state to the next, so a two
/// sided figure steps both of its rows at once.
#[pyfunction]
#[pyo3(signature = (cube=None, word="", tracks=None, meet=None, pause=1.0, titles=None,
                    columns=8, steps=6, ms=420.0, hold=None, size=160.0, yaw=38.0,
                    pitch=24.0, numbered=true, ghosts=true, title="Rubik's cube"))]
#[allow(clippy::too_many_arguments)]
fn mosaic_player(
    cube: Option<&Cube>,
    word: &str,
    tracks: Option<&Bound<'_, PyList>>,
    meet: Option<&str>,
    pause: f64,
    titles: Option<Vec<String>>,
    columns: usize,
    steps: usize,
    ms: f64,
    hold: Option<f64>,
    size: f64,
    yaw: f64,
    pitch: f64,
    numbered: bool,
    ghosts: bool,
    title: &str,
) -> PyResult<Drawing> {
    let (tl, caption) = mosaic_timeline(
        cube, word, tracks, meet, pause, titles, columns, steps, ms, hold, size, yaw, pitch,
        numbered, ghosts,
    )?;
    Ok(Drawing {
        text: export::player_html(&tl, title, &caption),
        is_svg: false,
    })
}

/// Percent encode a query value.
fn encoded(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => other
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect(),
        })
        .collect()
}

/// An `<iframe>` of the published app, showing this state: the one export that
/// stays interactive, because the page itself does the drawing.
#[pyfunction]
#[pyo3(signature = (cube, word="", base_url="https://twaclaw.github.io/cube/", height=420,
                    numbers=false, autoplay=false, views=None, controls=false,
                    yaw=38.0, pitch=24.0))]
#[allow(clippy::too_many_arguments)]
fn embed(
    cube: &Cube,
    word: &str,
    base_url: &str,
    height: u32,
    numbers: bool,
    autoplay: bool,
    views: Option<&str>,
    controls: bool,
    yaw: f64,
    pitch: f64,
) -> PyResult<String> {
    parse_moves(cube.inner.n(), word)?;
    let mut query = vec![
        format!("n={}", cube.inner.n()),
        format!("state={}", cube.inner.to_letters()),
        format!("cam={yaw:.0},{pitch:.0}"),
        "embed=1".to_string(),
    ];
    if !word.trim().is_empty() {
        query.push(format!("seq={}", encoded(word.trim())));
    }
    for (flag, on) in [("numbers", numbers), ("autoplay", autoplay), ("controls", controls)] {
        if on {
            query.push(format!("{flag}=1"));
        }
    }
    if let Some(which) = views {
        query.push(format!("views={which}"));
    }
    Ok(export::embed_snippet(base_url, &query.join("&"), height))
}

/// Solve a cube and return both phases.
#[pyfunction]
fn solve(cube: &Cube) -> PyResult<Solution> {
    cube.solve()
}

/// Where a tree figure gets its draggable cube, if it gets one.
fn app_url(app: Option<&Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    let Some(value) = app else {
        return Ok(None);
    };
    if let Ok(flag) = value.extract::<bool>() {
        return Ok(flag.then(|| "https://twaclaw.github.io/cube/".to_string()));
    }
    Ok(Some(value.extract::<String>()?))
}

fn tree_style(size: f64, zoom: f64, flat: bool, yaw: f64, pitch: f64) -> graphs::Style {
    graphs::Style {
        node: size,
        zoom,
        flat,
        camera: Camera {
            yaw,
            pitch,
            zoom: 1.0,
        }
        .clamped(),
        ..graphs::Style::default()
    }
}

/// A tree as a drawing: one SVG, or HTML when an app can host a draggable cube.
fn tree_drawing(tree: &graphs::Tree, style: &graphs::Style, app: Option<String>) -> Drawing {
    match app {
        Some(url) => Drawing {
            text: tree.html(style, Some(&url)),
            is_svg: false,
        },
        None => Drawing::svg_of(tree.svg(style)),
    }
}

/// The tree of a solver table being filled, from the solved cube outward.
///
/// `coord` is one of twist, flip, slice, corners, ud_edges, slice_perm;
/// `moves` are the moves to branch on, written as the solver writes them, and
/// an empty list means the whole move set the table is built with. Keep both
/// small: three moves and four levels is already forty cubes.
#[pyfunction]
#[pyo3(signature = (coord="corners", moves=None, depth=3, size=46.0, zoom=3.4,
                    flat=false, yaw=34.0, pitch=26.0, app=None))]
#[allow(clippy::too_many_arguments)]
fn table_tree(
    coord: &str,
    moves: Option<Vec<String>>,
    depth: usize,
    size: f64,
    zoom: f64,
    flat: bool,
    yaw: f64,
    pitch: f64,
    app: Option<&Bound<'_, PyAny>>,
) -> PyResult<Drawing> {
    let which = match coord {
        "twist" => graphs::Coord::Twist,
        "flip" => graphs::Coord::Flip,
        "slice" => graphs::Coord::Slice,
        "corners" => graphs::Coord::Corners,
        "ud_edges" => graphs::Coord::UdEdges,
        "slice_perm" => graphs::Coord::SlicePerm,
        other => {
            return Err(PyValueError::new_err(format!(
                "no coordinate called {other:?}: try twist, flip, slice, corners, \
                 ud_edges or slice_perm"
            )));
        }
    };
    let indices = move_indices(moves.as_deref())?;
    let tree = graphs::table_tree(which, &indices, depth);
    let style = tree_style(size, zoom, flat, yaw, pitch);
    Ok(tree_drawing(&tree, &style, app_url(app)?))
}

/// The tree an iterative deepening search walks, at the budget that works.
#[pyfunction]
#[pyo3(signature = (cube, phase=1, budget=None, cut=2, max_nodes=4000, size=46.0,
                    zoom=3.4, flat=false, yaw=34.0, pitch=26.0, app=None))]
#[allow(clippy::too_many_arguments)]
fn search_tree(
    cube: &Cube,
    phase: u8,
    budget: Option<usize>,
    cut: usize,
    max_nodes: usize,
    size: f64,
    zoom: f64,
    flat: bool,
    yaw: f64,
    pitch: f64,
    app: Option<&Bound<'_, PyAny>>,
) -> PyResult<Drawing> {
    let which = match phase {
        1 => graphs::Phase::One,
        2 => graphs::Phase::Two,
        other => return Err(PyValueError::new_err(format!("no phase {other}: 1 or 2"))),
    };
    let tree = graphs::search_tree(
        solver().tables(),
        &cube.inner,
        which,
        budget,
        max_nodes,
        cut,
    )
    .ok_or_else(|| {
        PyValueError::new_err(
            "that state is not one this search can start from: it is unreachable, \
             or phase 2 was asked for a cube that has not reached the subgroup",
        )
    })?;
    let style = tree_style(size, zoom, flat, yaw, pitch);
    Ok(tree_drawing(&tree, &style, app_url(app)?))
}

/// Move names to the indices the solver keeps them in.
fn move_indices(names: Option<&[String]>) -> PyResult<Vec<usize>> {
    let Some(names) = names else {
        return Ok(Vec::new());
    };
    names
        .iter()
        .map(|want| {
            (0..N_MOVES)
                .find(|&m| move_name(m) == want.as_str())
                .ok_or_else(|| PyValueError::new_err(format!("no move called {want:?}")))
        })
        .collect()
}

/// The faces, in the order facelet numbers run.
#[pyfunction]
fn faces() -> Vec<String> {
    Face::ALL.iter().map(|f| f.letter().to_string()).collect()
}

#[pymodule]
fn magiccube(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<Cube>()?;
    module.add_class::<Drawing>()?;
    module.add_class::<Permutation>()?;
    module.add_class::<Solution>()?;
    module.add_function(wrap_pyfunction!(corners, module)?)?;
    module.add_function(wrap_pyfunction!(edges, module)?)?;
    module.add_function(wrap_pyfunction!(centers, module)?)?;
    module.add_function(wrap_pyfunction!(cubies, module)?)?;
    module.add_function(wrap_pyfunction!(facelets, module)?)?;
    module.add_function(wrap_pyfunction!(facelet_numbers, module)?)?;
    module.add_function(wrap_pyfunction!(orbits, module)?)?;
    module.add_function(wrap_pyfunction!(strip, module)?)?;
    module.add_function(wrap_pyfunction!(animation, module)?)?;
    module.add_function(wrap_pyfunction!(player, module)?)?;
    module.add_function(wrap_pyfunction!(mosaic, module)?)?;
    module.add_function(wrap_pyfunction!(mosaic_player, module)?)?;
    module.add_function(wrap_pyfunction!(embed, module)?)?;
    module.add_function(wrap_pyfunction!(solve, module)?)?;
    module.add_function(wrap_pyfunction!(table_tree, module)?)?;
    module.add_function(wrap_pyfunction!(search_tree, module)?)?;
    module.add_function(wrap_pyfunction!(faces, module)?)?;
    module.add("__doc__", "The cube drawings, for notebooks.")?;
    Ok(())
}
