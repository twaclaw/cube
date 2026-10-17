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
use cube_core::solver::{Kociemba, SolveError, search::Limits};
use cube_core::{Cube as CoreCube, Permutation as CorePermutation, notation};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
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
    module.add_function(wrap_pyfunction!(embed, module)?)?;
    module.add_function(wrap_pyfunction!(solve, module)?)?;
    module.add_function(wrap_pyfunction!(faces, module)?)?;
    module.add("__doc__", "The cube drawings, for notebooks.")?;
    Ok(())
}
