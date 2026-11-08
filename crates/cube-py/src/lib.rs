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
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
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
            PyValueError::new_err(format!("no colour called {text}: use a name or #rrggbb"))
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
fn marks_from(
    n: u8,
    fills: Option<&Bound<'_, PyDict>>,
    rings: Option<&Bound<'_, PyDict>>,
) -> PyResult<Marks> {
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
#[pyclass(module = "magic_cube")]
#[derive(Clone)]
pub struct Drawing {
    text: String,
    is_svg: bool,
    /// How tall a frame the HTML needs, when the drawing knows; otherwise a
    /// frame gets the old fixed height.
    height: Option<f64>,
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
        let height = self
            .height
            .map_or("460".to_string(), |h| format!("{}", h.ceil()));
        format!(
            "<iframe srcdoc=\"{quoted}\" width=\"100%\" height=\"{height}\" style=\"border:0\"></iframe>"
        )
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
            height: None,
        }
    }
}

/// A state read as a permutation of the numbered facelets.
#[pyclass(module = "magic_cube")]
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
#[pyclass(module = "magic_cube")]
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
#[pyclass(module = "magic_cube")]
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
        height: None,
    })
}

/// The settings a track in a mosaic understands.
const TRACK_KEYS: &[&str] = &[
    "cube", "word", "row", "col", "back", "delay", "ms", "hold", "wrap", "labels", "title", "mark",
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

/// Cells to mark, as indices, or `True` for the one the track ends on.
fn dict_cells(dict: &Bound<'_, PyDict>, key: &str, cells: usize) -> PyResult<Vec<usize>> {
    match dict.get_item(key)? {
        None => Ok(Vec::new()),
        Some(value) => match value.extract::<bool>() {
            Ok(true) => Ok(vec![cells - 1]),
            Ok(false) => Ok(Vec::new()),
            Err(_) => value.extract(),
        },
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
    track.highlight = dict_cells(dict, "mark", track.cells())?;
    if let Some(past) = track.highlight.iter().find(|i| **i >= track.cells()) {
        return Err(PyValueError::new_err(format!(
            "a track of {} cells has no cell {past} to mark",
            track.cells()
        )));
    }
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
    mark: Option<bool>,
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
    // A two sided figure points at the pair it meets on of its own accord;
    // anything else points at the cube each row ends on, and only when asked.
    match mark {
        Some(true) => {
            for row in rows.iter_mut().filter(|r| r.highlight.is_empty()) {
                let last = row.cells() - 1;
                row.highlight = vec![last];
            }
        }
        Some(false) => {
            for row in rows.iter_mut() {
                row.highlight.clear();
            }
        }
        None => {}
    }
    Ok((rows, caption))
}

/// `frame` or `dot`, and when it shows: `met`, `landed` or `end`.
fn highlight_of(
    shape: &str,
    when: &str,
    color: Option<&str>,
) -> PyResult<export::mosaic::Highlight> {
    let mark = match shape {
        "frame" => export::mosaic::Mark::Frame,
        "dot" => export::mosaic::Mark::Dot,
        other => {
            return Err(PyValueError::new_err(format!(
                "no mark called {other:?}: frame or dot"
            )));
        }
    };
    let when = match when {
        "met" => export::mosaic::When::Met,
        "landed" => export::mosaic::When::Landed,
        "end" => export::mosaic::When::End,
        other => {
            return Err(PyValueError::new_err(format!(
                "no moment called {other:?}: met, landed or end"
            )));
        }
    };
    Ok(export::mosaic::Highlight {
        mark,
        when,
        color: color.map(parse_color).transpose()?,
    })
}

#[allow(clippy::too_many_arguments)]
fn mosaic_timeline(
    cube: Option<&Cube>,
    word: &str,
    tracks: Option<&Bound<'_, PyList>>,
    meet: Option<&str>,
    pause: f64,
    titles: Option<Vec<String>>,
    mark: Option<bool>,
    mark_shape: &str,
    mark_when: &str,
    mark_color: Option<&str>,
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
    let (rows, caption) = mosaic_plan(cube, word, tracks, meet, pause, titles, mark)?;
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
        highlight: highlight_of(mark_shape, mark_when, mark_color)?,
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
///
/// A marked cube gets a frame round it, or a dot under it with
/// `mark_shape="dot"`, in `mark_color`. A two sided figure marks the pair its
/// searches meet on; `mark=False` turns that off and `mark=True` marks the
/// cube every row ends on, while a track of your own says which of its cells
/// to mark with `"mark": [3]`, or `True` for the one it ends on. `mark_when`
/// says at what moment: `"met"` the moment every marked cube has arrived,
/// which for a two sided search is where the halves meet, `"landed"` each as
/// its own cube arrives, `"end"` once the whole figure has played out.
#[pyfunction]
#[pyo3(signature = (cube=None, word="", tracks=None, meet=None, pause=1.0, titles=None,
                    mark=None, mark_shape="frame", mark_when="met", mark_color=None,
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
    mark: Option<bool>,
    mark_shape: &str,
    mark_when: &str,
    mark_color: Option<&str>,
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
        cube, word, tracks, meet, pause, titles, mark, mark_shape, mark_when, mark_color, columns,
        steps, ms, hold, size, yaw, pitch, numbered, ghosts,
    )?;
    Ok(Drawing::svg_of(export::animated_svg(&tl, background)))
}

/// The same grid as `mosaic`, as a self contained HTML page with play, step
/// and scrub. The arrows step from one landed state to the next, so a two
/// sided figure steps both of its rows at once.
#[pyfunction]
#[pyo3(signature = (cube=None, word="", tracks=None, meet=None, pause=1.0, titles=None,
                    mark=None, mark_shape="frame", mark_when="met", mark_color=None,
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
    mark: Option<bool>,
    mark_shape: &str,
    mark_when: &str,
    mark_color: Option<&str>,
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
        cube, word, tracks, meet, pause, titles, mark, mark_shape, mark_when, mark_color, columns,
        steps, ms, hold, size, yaw, pitch, numbered, ghosts,
    )?;
    Ok(Drawing {
        text: export::player_html(&tl, title, &caption),
        is_svg: false,
        height: None,
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

/// Facelets to ring, as a list of numbers or the app's own `1,3,8-11` string.
/// Checked against this cube, so a number that does not exist fails here
/// rather than quietly ringing nothing.
fn highlights(n: u8, rings: &Bound<'_, PyAny>) -> PyResult<Vec<u32>> {
    let numbers = match rings.extract::<Vec<u32>>() {
        Ok(list) => list,
        Err(_) => {
            let text: String = rings.extract().map_err(|_| {
                PyValueError::new_err(
                    "rings is a list of facelet numbers, or a string such as \"1,3,8-11\"",
                )
            })?;
            let number = |part: &str| {
                part.trim().parse::<u32>().map_err(|_| {
                    PyValueError::new_err(format!("{part:?} is not a facelet number or a range"))
                })
            };
            let mut out = Vec::new();
            for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                match part.split_once('-') {
                    Some((first, last)) => {
                        let (first, last) = (number(first)?, number(last)?);
                        if first > last {
                            return Err(PyValueError::new_err(format!("{part:?} runs backwards")));
                        }
                        out.extend(first..=last);
                    }
                    None => out.push(number(part)?),
                }
            }
            out
        }
    };
    for which in &numbers {
        if facelet_from_label(n, *which).is_none() {
            return Err(PyKeyError::new_err(format!(
                "this cube has no facelet {which}"
            )));
        }
    }
    Ok(numbers)
}

/// The figures an embed may show, in the order the app lays them out.
const EMBED_VIEWS: [&str; 3] = ["3d", "net", "perm"];

/// The figures `views` names, from one name, a comma separated string such as
/// `"3d,perm"`, or a list such as `["3d", "perm"]`. They come back in page
/// order, each once, since that is the order the app shows them in anyway.
fn view_list(views: &Bound<'_, PyAny>) -> PyResult<Vec<&'static str>> {
    let asked: Vec<String> = match views.extract::<String>() {
        Ok(text) => text.split(',').map(|v| v.trim().to_string()).collect(),
        Err(_) => views.extract::<Vec<String>>().map_err(|_| {
            PyTypeError::new_err(
                "views is a view name, a string such as \"3d,perm\", or a list of names",
            )
        })?,
    };
    let asked: Vec<&str> = asked
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .collect();
    if let Some(bad) = asked.iter().find(|v| !EMBED_VIEWS.contains(v)) {
        return Err(PyValueError::new_err(format!(
            "no view called {bad:?}: try 3d, net or perm"
        )));
    }
    if asked.is_empty() {
        return Err(PyValueError::new_err(
            "views names no view: try 3d, net or perm, or leave it out for the default",
        ));
    }
    Ok(EMBED_VIEWS
        .into_iter()
        .filter(|v| asked.contains(v))
        .collect())
}

/// An `<iframe>` of the published app, showing this state: the one export that
/// stays interactive, because the page itself does the drawing.
///
/// `views` picks the figures on show: `"3d"`, `"net"` or `"perm"` alone, or
/// several as `"3d,perm"` or `["3d", "perm"]`. With `controls` the app's view
/// select offers only those, so a reader cannot switch to one left out. Left
/// as `None`, an embed shows the 3D cube and the net.
///
/// `titles` puts a small header on top of a view, keyed by the view's name:
/// `{"3d": "after R U", "perm": "as cycles"}`. Any view can go without one.
///
/// `rings` and `caption` are what a figure in a document needs beyond the
/// app's own state, so the markup comes back wrapped in a `cube-embed` div,
/// and in a `<figure>` when there is a caption to carry.
#[pyfunction]
#[pyo3(signature = (cube, word="", base_url="https://twaclaw.github.io/cube/", height=420,
                    numbers=false, autoplay=false, views=None, controls=false,
                    yaw=38.0, pitch=24.0, rings=None, caption=None, titles=None))]
#[allow(clippy::too_many_arguments)]
fn embed(
    cube: &Cube,
    word: &str,
    base_url: &str,
    height: u32,
    numbers: bool,
    autoplay: bool,
    views: Option<&Bound<'_, PyAny>>,
    controls: bool,
    yaw: f64,
    pitch: f64,
    rings: Option<&Bound<'_, PyAny>>,
    caption: Option<&str>,
    titles: Option<std::collections::HashMap<String, String>>,
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
    for (flag, on) in [
        ("numbers", numbers),
        ("autoplay", autoplay),
        ("controls", controls),
    ] {
        if on {
            query.push(format!("{flag}=1"));
        }
    }
    if let Some(views) = views {
        query.push(format!("views={}", view_list(views)?.join(",")));
    }
    if let Some(titles) = titles {
        if let Some(bad) = titles.keys().find(|v| !EMBED_VIEWS.contains(&v.as_str())) {
            return Err(PyValueError::new_err(format!(
                "no view called {bad:?} to title: try 3d, net or perm"
            )));
        }
        for view in EMBED_VIEWS {
            if let Some(text) = titles.get(view).map(|t| t.trim())
                && !text.is_empty()
            {
                query.push(format!("title-{view}={}", encoded(text)));
            }
        }
    }
    if let Some(rings) = rings {
        let numbers = highlights(cube.inner.n(), rings)?;
        if !numbers.is_empty() {
            let list: Vec<String> = numbers.iter().map(u32::to_string).collect();
            query.push(format!("hl={}", list.join(",")));
        }
    }
    Ok(export::embed_figure(
        base_url,
        &query.join("&"),
        height,
        caption,
    ))
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
            height: None,
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

/// The Cayley graph of the group a few maneuvers generate, with the cube that
/// each element *is* drawn at every vertex.
///
/// `gens` are the generators, written the way the rest of the library writes
/// moves. Distance counts only the generators given, so `["U", "D"]` puts `U'`
/// three moves out; pass `["U", "U'", "D", "D'"]` for the undirected graph
/// where it is one, and the edges then come out as lines rather than arrows.
///
/// `rings` stops the graph that many moves out from solved, which is how a
/// group too big to draw whole is drawn at all: `rings=2` with the twelve
/// quarter turns is the first two rings of the 3x3x3's own Cayley graph.
/// `None` draws every element.
///
/// `layout` is one of
///
/// * `shells`, concentric rings one move apart, with the solved cube in the
///   middle: the number of rings is God's number for these generators;
/// * `grid`, for two generators that commute, which draws the torus they are;
/// * `ring`, when alternating the generators walks the whole group and comes
///   home, which draws that walk as the cycle it is;
/// * `circle`, everything on one circle in the order the search found it.
///
/// A big graph is drawn at the size it needs rather than squeezed onto a page,
/// so `pane` says how the page should hold it:
///
/// * `scroll`, the figure at full size in a box that scrolls;
/// * `zoom`, the whole figure fitted into the box, with the wheel to zoom into
///   a region and a drag to pan, which is the one to use in a narrow column;
/// * `full`, no box at all.
///
/// `pane_height` is how tall that box is, in pixels.
#[pyfunction]
#[pyo3(signature = (gens=None, n=3, rings=None, layout="shells", spread=2.0, limit=None,
                    caption="", size=46.0, zoom=3.4, flat=false, yaw=34.0,
                    pitch=26.0, pane="scroll", pane_height=640.0, app=None))]
#[allow(clippy::too_many_arguments)]
fn cayley(
    gens: Option<Vec<String>>,
    n: u8,
    rings: Option<usize>,
    layout: &str,
    spread: f64,
    limit: Option<usize>,
    caption: &str,
    size: f64,
    zoom: f64,
    flat: bool,
    yaw: f64,
    pitch: f64,
    pane: &str,
    pane_height: f64,
    app: Option<&Bound<'_, PyAny>>,
) -> PyResult<Drawing> {
    let words = gens.unwrap_or_else(|| vec!["U".to_string(), "D".to_string()]);
    let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
    let which = match layout {
        "shells" => graphs::Layout::Shells,
        "circle" => graphs::Layout::Circle,
        "grid" => graphs::Layout::Grid,
        "ring" => graphs::Layout::Ring,
        other => {
            return Err(PyValueError::new_err(format!(
                "no layout called {other:?}: try shells, circle, grid or ring"
            )));
        }
    };
    let box_ = match pane {
        "scroll" => graphs::Pane::Scroll(pane_height),
        "zoom" => graphs::Pane::Zoom(pane_height),
        "full" | "none" => graphs::Pane::Full,
        other => {
            return Err(PyValueError::new_err(format!(
                "no pane called {other:?}: try scroll, zoom or full"
            )));
        }
    };
    let graph = graphs::cayley_rings(n, &borrowed, rings, limit.unwrap_or(graphs::DEFAULT_LIMIT))
        .map_err(PyValueError::new_err)?
        .with_layout(which)
        .with_spread(spread)
        .with_pane(box_)
        .with_caption(caption);
    let style = tree_style(size, zoom, flat, yaw, pitch);
    let url = app_url(app)?;
    if url.is_none() && graph.pane == graphs::Pane::Full {
        return Ok(Drawing::svg_of(graph.svg(&style)));
    }
    // A pane is HTML, so a graph that wants one is not a bare SVG, and it says
    // how tall it is so that the frame a notebook puts it in does not cut it.
    let (text, height) = graph.html_sized(&style, url.as_deref());
    Ok(Drawing {
        text,
        is_svg: false,
        height: Some(height),
    })
}

#[pymodule]
fn magic_cube(module: &Bound<'_, PyModule>) -> PyResult<()> {
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
    module.add_function(wrap_pyfunction!(cayley, module)?)?;
    module.add_function(wrap_pyfunction!(faces, module)?)?;
    module.add("__doc__", "The cube drawings, for notebooks.")?;
    Ok(())
}
