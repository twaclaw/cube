//! Command line exporter: the same drawings as the app, for a blog build.

use cube_core::export;
use cube_core::geom::facelet_from_label;
use cube_core::palette::STANDARD;
use cube_core::render::Highlights;
use cube_core::render::cube3d::{CAMERA_PRESETS, Camera, Style3d};
use cube_core::render::net::NetStyle;
use cube_core::scramble::{Rng, scramble};
use cube_core::{Cube, notation};
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
  -m, --mode MODE       still | net | strip | anim | player (default still)
      --numbers         facelet numbers on the net
      --highlight LIST  facelet numbers to ring, e.g. 1,3,8-11
      --steps N         animation frames per move (default 8, 1 = states only)
      --ms N            milliseconds per move (default 420)
      --columns N       strip columns (default 6)
      --size-px N       SVG viewBox side (default 300)
      --flat            notebook colors, no shading
      --background CSS  solid background instead of transparent
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
    background: Option<String>,
    out: Option<String>,
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
            background: None,
            out: None,
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
            "--background" => a.background = Some(value()?),
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
            cube_core::render::cube3d::document(&state, &cam, None, &st, &STANDARD, &highlights, bg)
        }
        "net" => {
            let mut state = cube.clone();
            state.apply_moves(&moves);
            cube_core::render::net::document(&state, &net, &STANDARD, &highlights, bg)
        }
        "strip" => export::strip_svg(
            &cube,
            &moves,
            &cam,
            &st,
            &STANDARD,
            &highlights,
            a.columns,
            bg,
        ),
        "anim" => {
            let tl = export::timeline(
                &cube,
                &moves,
                &cam,
                &st,
                &STANDARD,
                &highlights,
                a.steps,
                a.ms,
            );
            export::animated_svg(&tl, bg)
        }
        "player" => {
            let tl = export::timeline(
                &cube,
                &moves,
                &cam,
                &st,
                &STANDARD,
                &highlights,
                a.steps,
                a.ms,
            );
            export::player_html(&tl, "Rubik's cube", &notation::format(&moves))
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

/// `1,3,8-11` into facelets.
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
