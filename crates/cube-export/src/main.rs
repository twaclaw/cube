//! Command line exporter: the same drawings as the app, for a blog build.

use cube_core::export;
use cube_core::export::mosaic;
use cube_core::geom::facelet_from_label;
use cube_core::palette::STANDARD;
use cube_core::render::cube3d::{CAMERA_PRESETS, Camera, Style3d};
use cube_core::render::net::NetStyle;
use cube_core::render::{Highlights, Marks};
use cube_core::scramble::{Rng, scramble};
use cube_core::{Cube, Move, notation};
use std::process::ExitCode;

const USAGE: &str = "\
cube-export: draw a cube or a maneuver as SVG or HTML

usage: cube-export [options] > out.svg

  -n, --size N          cube size (default 3)
  -s, --seq WORD        maneuver, e.g. \"R U R' U'\" or \"rfu\"
      --from WORD       maneuver applied before the one being drawn
      --scramble N      start from a random state of N turns
      --seed N          seed for --scramble
  -v, --view YAW,PITCH  camera angles in degrees
  -p, --preset NAME     named view (U F R, Isometric, Front, Up, ...)
      --zoom K          camera zoom (default 1)
  -m, --mode MODE       still | net | strip | anim | player | mosaic |
                        mosaic-player | table-tree | search-tree
                        (default still)
      --numbers         facelet numbers on the net
      --highlight LIST  facelet numbers to ring, e.g. 1,3,8-11
      --steps N         animation frames per move (default 8, 1 = states only)
      --ms N            milliseconds per move (default 420)
      --columns N       strip and mosaic columns (default 6)
      --size-px N       SVG viewBox side (default 300)
      --flat            notebook colors, no shading
      --background CSS  solid background instead of transparent

 grids on the clock (--mode mosaic | mosaic-player)
      --meet WORD       second half of the solution: draws the two sided
                        search, --seq forwards and this one backwards
      --pause N         turns to wait before the combined row (default 1)
      --hold N          milliseconds a landed state is held before the next
                        turn starts (default: a little under half a turn)
      --titles LIST     names for the rows, e.g. forwards,backwards,together
      --unnumbered      write the move alone under each cube, unnumbered
      --mark SHAPE      how the cubes two searches meet on are pointed at:
                        frame | dot | none (default frame)
      --mark-when WHEN  met (as they meet) | landed | end (default met)
      --mark-color CSS  the colour to point with, as #rrggbb
      --no-ghosts       leave the cells a cube has yet to reach blank

 trees (--mode table-tree | search-tree)
      --coord NAME      twist | flip | slice | corners | ud_edges | slice_perm
      --depth N         levels to draw (default 3)
      --moves LIST      moves to branch on, e.g. U,R2 (default: the table's own)
      --phase 1|2       which search to draw (default 1)
      --budget N        force one move budget instead of the first that works
      --cut N           cut branches drawn per node (default 2)
      --app URL         wrap in html, with a draggable cube from that app
  -o, --out FILE        write to FILE instead of stdout
  -h, --help            this text
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cube-export: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    n: u8,
    seq: String,
    from: String,
    scramble: usize,
    seed: u64,
    cam: Camera,
    mode: String,
    numbers: bool,
    highlight: String,
    steps: usize,
    ms: f64,
    columns: usize,
    size: f64,
    flat: bool,
    meet: String,
    pause: f64,
    hold: Option<f64>,
    titles: String,
    numbered: bool,
    mark: String,
    mark_when: String,
    mark_color: Option<String>,
    ghosts: bool,
    background: Option<String>,
    out: Option<String>,
    coord: String,
    depth: usize,
    moves: String,
    phase: u8,
    budget: Option<usize>,
    cut: usize,
    app: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            n: 3,
            seq: String::new(),
            from: String::new(),
            scramble: 0,
            seed: 20_251_016,
            cam: Camera::default(),
            mode: "still".to_string(),
            numbers: false,
            highlight: String::new(),
            steps: 8,
            ms: 420.0,
            columns: 6,
            size: 300.0,
            flat: false,
            meet: String::new(),
            pause: 1.0,
            hold: None,
            titles: String::new(),
            numbered: true,
            mark: "frame".to_string(),
            mark_when: "met".to_string(),
            mark_color: None,
            ghosts: true,
            background: None,
            out: None,
            coord: "corners".to_string(),
            depth: 3,
            moves: String::new(),
            phase: 1,
            budget: None,
            cut: 2,
            app: None,
        }
    }
}

fn run() -> Result<(), String> {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-n" | "--size" => a.n = value()?.parse().map_err(|_| "bad size")?,
            "-s" | "--seq" => a.seq = value()?,
            "--from" => a.from = value()?,
            "--scramble" => a.scramble = value()?.parse().map_err(|_| "bad length")?,
            "--seed" => a.seed = value()?.parse().map_err(|_| "bad seed")?,
            "-v" | "--view" => {
                let v = value()?;
                let (yaw, pitch) = v.split_once(',').ok_or("--view wants YAW,PITCH")?;
                a.cam.yaw = yaw.trim().parse().map_err(|_| "bad yaw")?;
                a.cam.pitch = pitch.trim().parse().map_err(|_| "bad pitch")?;
            }
            "-p" | "--preset" => {
                let name = value()?;
                let (_, yaw, pitch) = CAMERA_PRESETS
                    .iter()
                    .find(|(n, _, _)| n.eq_ignore_ascii_case(&name))
                    .ok_or_else(|| format!("no view called {name:?}"))?;
                a.cam.yaw = *yaw;
                a.cam.pitch = *pitch;
            }
            "--zoom" => a.cam.zoom = value()?.parse().map_err(|_| "bad zoom")?,
            "-m" | "--mode" => a.mode = value()?,
            "--numbers" => a.numbers = true,
            "--highlight" => a.highlight = value()?,
            "--steps" => a.steps = value()?.parse().map_err(|_| "bad step count")?,
            "--ms" => a.ms = value()?.parse().map_err(|_| "bad duration")?,
            "--columns" => a.columns = value()?.parse().map_err(|_| "bad column count")?,
            "--size-px" => a.size = value()?.parse().map_err(|_| "bad size")?,
            "--flat" => a.flat = true,
            "--meet" => a.meet = value()?,
            "--pause" => a.pause = value()?.parse().map_err(|_| "bad turn count")?,
            "--hold" => a.hold = Some(value()?.parse().map_err(|_| "bad duration")?),
            "--titles" => a.titles = value()?,
            "--unnumbered" => a.numbered = false,
            "--mark" => a.mark = value()?,
            "--mark-when" => a.mark_when = value()?,
            "--mark-color" => a.mark_color = Some(value()?),
            "--no-ghosts" => a.ghosts = false,
            "--background" => a.background = Some(value()?),
            "--coord" => a.coord = value()?,
            "--depth" => a.depth = value()?.parse().map_err(|_| "bad depth")?,
            "--moves" => a.moves = value()?,
            "--phase" => a.phase = value()?.parse().map_err(|_| "bad phase")?,
            "--budget" => a.budget = Some(value()?.parse().map_err(|_| "bad budget")?),
            "--cut" => a.cut = value()?.parse().map_err(|_| "bad count")?,
            "--app" => a.app = Some(value()?),
            "-o" | "--out" => a.out = Some(value()?),
            other => return Err(format!("unknown option {other}")),
        }
    }

    let mut cube = Cube::solved(a.n);
    if a.scramble > 0 {
        cube.apply_moves(&scramble(a.n, a.scramble, &mut Rng::new(a.seed)));
    }
    if !a.from.is_empty() {
        cube.apply_moves(&notation::parse(a.n, &a.from).map_err(|e| e.to_string())?);
    }
    let moves = notation::parse(a.n, &a.seq).map_err(|e| e.to_string())?;

    let highlights = parse_highlights(a.n, &a.highlight)?;
    let marks = Marks::rings(&highlights, STANDARD.highlight);
    let st = Style3d {
        size: a.size,
        shade: if a.flat {
            0.0
        } else {
            Style3d::default().shade
        },
        ..Style3d::default()
    };
    let net = NetStyle {
        cell: a.size / 12.0,
        numbers: a.numbers,
        ..NetStyle::default()
    };
    let bg = a.background.as_deref();
    let cam = a.cam.clamped();

    let text = match a.mode.as_str() {
        "still" => {
            let mut state = cube.clone();
            state.apply_moves(&moves);
            cube_core::render::cube3d::document(&state, &cam, None, &st, &STANDARD, &marks, bg)
        }
        "cycles" => {
            let mut state = cube.clone();
            state.apply_moves(&moves);
            let perm = cube_core::Permutation::of(&state)
                .ok_or("a cube this size does not name one permutation")?;
            cube_core::render::cycles::document(
                &perm,
                a.n,
                &STANDARD,
                &cube_core::render::CycleStyle::default(),
                bg,
            )
        }
        "net" => {
            let mut state = cube.clone();
            state.apply_moves(&moves);
            cube_core::render::net::document(&state, &net, &STANDARD, &marks, bg)
        }
        "strip" => export::strip_svg(&cube, &moves, &cam, &st, &STANDARD, &marks, a.columns, bg),
        "anim" => {
            let tl = export::timeline(&cube, &moves, &cam, &st, &STANDARD, &marks, a.steps, a.ms);
            export::animated_svg(&tl, bg)
        }
        "player" => {
            let tl = export::timeline(&cube, &moves, &cam, &st, &STANDARD, &marks, a.steps, a.ms);
            export::player_html(&tl, "Rubik's cube", &notation::format(&moves))
        }
        "mosaic" | "mosaic-player" => {
            let tracks = mosaic_tracks(&cube, &moves, &a)?;
            let layout = mosaic::Layout {
                columns: a.columns,
                numbered: a.numbered,
                ghosts: a.ghosts,
                highlight: mosaic::Highlight {
                    mark: match a.mark.as_str() {
                        "frame" | "none" => mosaic::Mark::Frame,
                        "dot" => mosaic::Mark::Dot,
                        other => return Err(format!("no mark called {other}: frame, dot or none")),
                    },
                    when: match a.mark_when.as_str() {
                        "met" => mosaic::When::Met,
                        "landed" => mosaic::When::Landed,
                        "end" => mosaic::When::End,
                        other => {
                            return Err(format!("no moment called {other}: met, landed or end"));
                        }
                    },
                    color: a.mark_color.as_deref().map(parse_rgb).transpose()?,
                },
                ..mosaic::Layout::default()
            };
            let pace = mosaic::Pace {
                ms: a.ms,
                hold: a.hold,
                steps: a.steps,
            };
            let tl = mosaic::timeline(&tracks, &cam, &st, &STANDARD, &marks, &layout, &pace);
            let word = tracks
                .last()
                .map(|t| notation::format(&t.moves))
                .unwrap_or_default();
            if a.mode == "mosaic" {
                export::animated_svg(&tl, bg)
            } else {
                export::player_html(&tl, "Rubik's cube", &word)
            }
        }
        "table-tree" => {
            let coord = parse_coord(&a.coord)?;
            let moves = parse_move_list(&a.moves)?;
            let tree = cube_graphs::table_tree(coord, &moves, a.depth);
            draw_tree(&tree, &a)
        }
        "search-tree" => {
            let phase = match a.phase {
                1 => cube_graphs::Phase::One,
                2 => cube_graphs::Phase::Two,
                other => return Err(format!("no phase {other}")),
            };
            let mut state = cube.clone();
            state.apply_moves(&moves);
            let tables = cube_core::solver::tables::Tables::new();
            let tree = cube_graphs::search_tree(&tables, &state, phase, a.budget, 4000, a.cut)
                .ok_or("that state is not one this search can start from")?;
            draw_tree(&tree, &a)
        }
        other => return Err(format!("unknown mode {other}")),
    };

    match a.out {
        Some(path) => std::fs::write(&path, text).map_err(|e| format!("{path}: {e}")),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

/// The tracks a mosaic draws: the sequence on its own, filling a grid, or the
/// three rows of a two sided search when `--meet` gives the second half.
fn mosaic_tracks(cube: &Cube, moves: &[Move], a: &Args) -> Result<Vec<mosaic::Track>, String> {
    let mut tracks = if a.meet.trim().is_empty() {
        vec![mosaic::Track::new(cube.clone(), moves.to_vec()).wrapping()]
    } else {
        let backward = notation::parse(a.n, &a.meet).map_err(|e| e.to_string())?;
        mosaic::meet(cube, moves, &backward, a.pause)
    };
    // --titles renames the rows in order; a blank one keeps the name the
    // figure gave itself.
    for (track, title) in tracks.iter_mut().zip(a.titles.split(',')) {
        if !title.trim().is_empty() {
            track.title = title.trim().to_string();
        }
    }
    if a.mark == "none" {
        for track in &mut tracks {
            track.highlight.clear();
        }
    }
    Ok(tracks)
}

/// `#rrggbb`, which is all the command line needs to name a colour.
fn parse_rgb(text: &str) -> Result<cube_core::palette::Rgb, String> {
    let hex = text.trim().strip_prefix('#').unwrap_or(text.trim());
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|_| hex.len() == 6)
        .map(cube_core::palette::Rgb::from_hex)
        .ok_or_else(|| format!("{text} is not a colour: write it as #rrggbb"))
}

/// `1,3,8-11` into facelets.
/// A tree, as one svg or wrapped in the html that can show a draggable cube.
fn draw_tree(tree: &cube_graphs::Tree, a: &Args) -> String {
    let style = cube_graphs::Style {
        flat: a.flat,
        camera: a.cam.clamped(),
        ..cube_graphs::Style::default()
    };
    match a.app.as_deref() {
        Some(url) => tree.html(&style, Some(url)),
        None => tree.svg(&style),
    }
}

fn parse_coord(name: &str) -> Result<cube_graphs::Coord, String> {
    use cube_graphs::Coord;
    match name {
        "twist" => Ok(Coord::Twist),
        "flip" => Ok(Coord::Flip),
        "slice" => Ok(Coord::Slice),
        "corners" => Ok(Coord::Corners),
        "ud_edges" | "ud-edges" => Ok(Coord::UdEdges),
        "slice_perm" | "slice-perm" => Ok(Coord::SlicePerm),
        other => Err(format!("no coordinate called {other:?}")),
    }
}

/// Move names to the solver's move indices.
fn parse_move_list(list: &str) -> Result<Vec<usize>, String> {
    use cube_core::solver::tables::{N_MOVES, move_name};
    let mut out = Vec::new();
    for want in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let found = (0..N_MOVES)
            .find(|&m| move_name(m) == want)
            .ok_or_else(|| format!("no move called {want:?}"))?;
        out.push(found);
    }
    Ok(out)
}

fn parse_highlights(n: u8, spec: &str) -> Result<Highlights, String> {
    let mut out = Highlights::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (lo, hi) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let lo: u32 = lo.parse().map_err(|_| format!("bad facelet {lo:?}"))?;
        let hi: u32 = hi.parse().map_err(|_| format!("bad facelet {hi:?}"))?;
        for label in lo..=hi {
            let facelet =
                facelet_from_label(n, label).ok_or_else(|| format!("no facelet {label}"))?;
            out.insert(facelet);
        }
    }
    Ok(out)
}
