//! The application state and everything it draws.

use crate::dom;
use cube_core::export::{self, ease};
use cube_core::geom::{Face, facelet_from_label, facelet_label};
use cube_core::moves::Move;
use cube_core::notation;
use cube_core::palette::{Palette, STANDARD};
use cube_core::render::Highlights;
use cube_core::render::cube3d::{self, Anim, CAMERA_PRESETS, Camera, Style3d};
use cube_core::render::net::{self, NetStyle};
use cube_core::scramble::{Rng, scramble};
use cube_core::{Cube, Turn};

/// A turn being drawn, between two states.
#[derive(Clone, Copy)]
pub struct Running {
    pub turn: Turn,
    pub start: f64,
    pub duration: f64,
    pub forward: bool,
}

pub struct App {
    pub n: u8,
    /// The state the loaded maneuver starts from.
    pub base: Cube,
    pub cube: Cube,
    pub moves: Vec<Move>,
    /// How many moves of the maneuver have been applied.
    pub cursor: usize,
    pub cam: Camera,
    pub zoom_on: bool,
    pub numbers: bool,
    pub highlights: Highlights,
    pub speed_ms: f64,
    pub playing: bool,
    pub running: Option<Running>,
    /// Where the pointer was last seen while turning the camera.
    pub drag: Option<(f64, f64)>,
    pub style: Style3d,
    pub net: NetStyle,
    pub palette: Palette,
    pub export_flat: bool,
    pub export_bg: bool,
    pub export_steps: usize,
}

impl App {
    pub fn new(n: u8) -> Self {
        Self {
            n,
            base: Cube::solved(n),
            cube: Cube::solved(n),
            moves: Vec::new(),
            cursor: 0,
            cam: Camera::default(),
            zoom_on: false,
            numbers: true,
            highlights: Highlights::new(),
            speed_ms: 420.0,
            playing: false,
            running: None,
            drag: None,
            style: Style3d::default(),
            net: NetStyle {
                numbers: true,
                ..NetStyle::default()
            },
            palette: STANDARD,
            export_flat: false,
            export_bg: true,
            export_steps: 8,
        }
    }

    // ----- drawing -------------------------------------------------------

    pub fn render(&self) {
        self.render_3d(None);
        self.render_net();
        self.render_chips();
        self.render_transport();
    }

    pub fn render_3d(&self, anim: Option<Anim>) {
        dom::set_html(
            "view-3d",
            &cube3d::inner(
                &self.cube,
                &self.cam,
                anim.as_ref(),
                &self.style,
                &self.palette,
                &self.highlights,
            ),
        );
    }

    pub fn render_net(&self) {
        let (w, h) = self.net.extent(self.n);
        let svg = dom::by_id("view-net");
        let _ = svg.set_attribute("viewBox", &format!("0 0 {w} {h}"));
        svg.set_inner_html(&net::markup(
            &self.cube,
            &self.net,
            &self.palette,
            &self.highlights,
        ));
    }

    fn render_chips(&self) {
        let mut html = String::new();
        for (i, m) in self.moves.iter().enumerate() {
            let class = if i < self.cursor {
                "done"
            } else if i == self.cursor {
                "current"
            } else {
                ""
            };
            html.push_str(&format!(
                "<li class=\"{class}\" data-step=\"{}\" title=\"jump here\">{}</li>",
                i + 1,
                html_escape(&m.name)
            ));
        }
        dom::set_html("chips", &html);
    }

    fn render_transport(&self) {
        let play = dom::by_id("play");
        play.set_inner_html(if self.playing {
            "&#10073;&#10073; pause"
        } else {
            "&#9654; play"
        });
        for (id, enabled) in [
            ("step-back", self.cursor > 0),
            ("to-start", self.cursor > 0),
            ("step-fwd", self.cursor < self.moves.len()),
            ("to-end", self.cursor < self.moves.len()),
            ("play", !self.moves.is_empty()),
        ] {
            let el: web_sys::HtmlButtonElement =
                wasm_bindgen::JsCast::unchecked_into(dom::by_id(id));
            el.set_disabled(!enabled);
        }
    }

    pub fn say(&self, message: &str, error: bool) {
        dom::set_text("status", message);
        let el = dom::by_id("status");
        let _ = el.set_attribute("class", if error { "status error" } else { "status" });
    }

    pub fn report_state(&self) {
        let solved = if self.cube.is_solved() {
            "solved"
        } else if self.cube.is_uniform() {
            "solved, re-oriented"
        } else {
            "scrambled"
        };
        let position = if self.moves.is_empty() {
            String::new()
        } else {
            format!(" · move {} of {}", self.cursor, self.moves.len())
        };
        self.say(&format!("{solved}{position}"), false);
    }

    // ----- the maneuver -------------------------------------------------

    pub fn load(&mut self, text: &str) -> Result<(), String> {
        let moves = notation::parse(self.n, text).map_err(|e| e.to_string())?;
        self.moves = moves;
        self.cursor = 0;
        self.base = self.cube.clone();
        self.playing = false;
        self.running = None;
        self.render();
        if self.moves.is_empty() {
            self.say("Nothing to do: the maneuver is empty.", false);
        } else {
            self.say(
                &format!(
                    "{} move{} loaded. Step with the arrows, or play.",
                    self.moves.len(),
                    if self.moves.len() == 1 { "" } else { "s" }
                ),
                false,
            );
        }
        Ok(())
    }

    /// Start again with a different cube size.
    pub fn resize(&mut self, n: u8) {
        self.n = n;
        self.moves.clear();
        self.cursor = 0;
        self.highlights.clear();
        self.playing = false;
        self.running = None;
        self.cube = Cube::solved(n);
        self.base = self.cube.clone();
        self.sync_highlight_field();
        self.render();
        self.say(&format!("A {n}x{n}x{n} cube, solved."), false);
    }

    /// Make the controls show what the state actually is, which a reloaded
    /// page does not do by itself.
    pub fn sync_controls(&self) {
        dom::input("numbers").set_checked(self.numbers);
        dom::input("zoom-on").set_checked(self.zoom_on);
        dom::input("speed").set_value(&format!("{:.0}", self.speed_ms));
        dom::input("export-flat").set_checked(self.export_flat);
        dom::input("export-bg").set_checked(self.export_bg);
        dom::input("export-steps").set_value(&format!("{}", self.export_steps));
        dom::select("size").set_value(&format!("{}", self.n));
        self.sync_highlight_field();
    }

    pub fn state_at(&self, k: usize) -> Cube {
        let mut cube = self.base.clone();
        cube.apply_moves(self.moves.iter().take(k));
        cube
    }

    pub fn jump_to(&mut self, k: usize) {
        self.cursor = k.min(self.moves.len());
        self.cube = self.state_at(self.cursor);
        self.playing = false;
        self.running = None;
        self.render();
        self.report_state();
    }

    pub fn can_step(&self, forward: bool) -> bool {
        if forward {
            self.cursor < self.moves.len()
        } else {
            self.cursor > 0
        }
    }

    /// Start drawing one move. Returns false when there is nothing to do.
    pub fn begin(&mut self, forward: bool, now: f64) -> bool {
        if self.running.is_some() || !self.can_step(forward) {
            return false;
        }
        let turn = if forward {
            self.moves[self.cursor].turn
        } else {
            self.moves[self.cursor - 1].turn.inverse()
        };
        let quarters = turn.quarters.unsigned_abs().max(1) as f64;
        self.running = Some(Running {
            turn,
            start: now,
            duration: self.speed_ms * (0.6 + 0.4 * quarters),
            forward,
        });
        true
    }

    /// Advance the animation; returns true while more frames are wanted.
    pub fn frame(&mut self, now: f64) -> bool {
        let Some(run) = self.running else {
            return false;
        };
        let t = ((now - run.start) / run.duration.max(1.0)).clamp(0.0, 1.0);
        if t < 1.0 {
            self.render_3d(Some(Anim {
                turn: run.turn,
                angle: run.turn.angle() * ease(t),
            }));
            return true;
        }
        self.cube.apply(&run.turn);
        self.cursor = if run.forward {
            self.cursor + 1
        } else {
            self.cursor - 1
        };
        self.running = None;
        if self.playing && self.begin(true, now) {
            self.render_net();
            self.render_chips();
            self.render_transport();
            return true;
        }
        self.playing = false;
        self.render();
        self.report_state();
        false
    }

    // ----- state changes -------------------------------------------------

    pub fn scramble(&mut self, seed: u64) {
        let moves = scramble(self.n, 8 * self.n as usize, &mut Rng::new(seed));
        let mut cube = Cube::solved(self.n);
        cube.apply_moves(&moves);
        self.set_state(cube);
        self.say("Scrambled.", false);
    }

    pub fn set_solved(&mut self) {
        self.set_state(Cube::solved(self.n));
        self.say("Solved state.", false);
    }

    fn set_state(&mut self, cube: Cube) {
        self.cube = cube;
        self.base = self.cube.clone();
        self.cursor = 0;
        self.playing = false;
        self.running = None;
        self.render();
    }

    pub fn apply_rest(&mut self) {
        self.jump_to(self.moves.len());
        self.say("Maneuver applied.", false);
    }

    // ----- highlights ----------------------------------------------------

    pub fn toggle_facelet(&mut self, key: &str) {
        let Some(facelet) = parse_facelet(key) else {
            return;
        };
        if !self.highlights.remove(&facelet) {
            self.highlights.insert(facelet);
        }
        self.sync_highlight_field();
        self.render_net();
        self.render_3d(None);
    }

    pub fn set_highlights(&mut self, spec: &str) {
        self.highlights = parse_highlight_spec(self.n, spec);
        self.render_net();
        self.render_3d(None);
    }

    pub fn sync_highlight_field(&self) {
        let mut labels: Vec<u32> = self
            .highlights
            .iter()
            .filter_map(|(f, r, c)| facelet_label(self.n, *f, *r, *c))
            .collect();
        labels.sort_unstable();
        let text = labels
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        dom::input("highlight").set_value(&text);
    }

    // ----- exports -------------------------------------------------------

    fn export_style(&self) -> Style3d {
        Style3d {
            shade: if self.export_flat {
                0.0
            } else {
                self.style.shade
            },
            ..self.style
        }
    }

    fn background(&self) -> Option<&'static str> {
        self.export_bg.then_some("#ffffff")
    }

    pub fn export(&self, what: &str) {
        let st = self.export_style();
        let bg = self.background();
        let seq = notation::format(&self.moves);
        match what {
            "svg3d" => {
                let svg = cube3d::document(
                    &self.cube,
                    &self.cam,
                    None,
                    &st,
                    &self.palette,
                    &self.highlights,
                    bg,
                );
                self.offer("cube-3d.svg", "image/svg+xml", &svg);
            }
            "svgnet" => {
                let svg = net::document(&self.cube, &self.net, &self.palette, &self.highlights, bg);
                self.offer("cube-net.svg", "image/svg+xml", &svg);
            }
            "strip" => {
                let svg = export::strip_svg(
                    &self.base,
                    &self.moves,
                    &self.cam,
                    &st,
                    &self.palette,
                    &self.highlights,
                    6,
                    bg,
                );
                self.offer("cube-steps.svg", "image/svg+xml", &svg);
            }
            "anim" => {
                let tl = self.timeline(&st);
                let svg = export::animated_svg(&tl, bg);
                self.offer("cube-animation.svg", "image/svg+xml", &svg);
            }
            "player" => {
                let tl = self.timeline(&st);
                let html = export::player_html(&tl, "Rubik's cube", &seq);
                self.offer("cube-animation.html", "text/html", &html);
            }
            "embed" => {
                let snippet = export::embed_snippet(&dom::page_url(), &self.query(), 420);
                dom::copy_to_clipboard(&snippet);
                self.say("Embed snippet copied to the clipboard.", false);
            }
            _ => {}
        }
    }

    fn timeline(&self, st: &Style3d) -> export::Timeline {
        export::timeline(
            &self.base,
            &self.moves,
            &self.cam,
            st,
            &self.palette,
            &self.highlights,
            self.export_steps,
            self.speed_ms,
        )
    }

    fn offer(&self, name: &str, mime: &str, data: &str) {
        dom::download(name, mime, data);
        self.say(
            &format!("{name} written, {}.", human_size(data.len())),
            false,
        );
    }

    /// Query string that reproduces what is on screen.
    pub fn query(&self) -> String {
        let mut parts = vec![
            format!("n={}", self.n),
            format!("state={}", self.base.to_letters()),
            format!("cam={:.0},{:.0}", self.cam.yaw, self.cam.pitch),
        ];
        if !self.moves.is_empty() {
            parts.push(format!("seq={}", encode(&notation::format(&self.moves))));
        }
        if self.numbers {
            parts.push("numbers=1".to_string());
        }
        if !self.highlights.is_empty() {
            let labels: Vec<String> = {
                let mut v: Vec<u32> = self
                    .highlights
                    .iter()
                    .filter_map(|(f, r, c)| facelet_label(self.n, *f, *r, *c))
                    .collect();
                v.sort_unstable();
                v.iter().map(u32::to_string).collect()
            };
            parts.push(format!("hl={}", labels.join(",")));
        }
        parts.push("embed=1".to_string());
        parts.join("&")
    }
}

fn encode(s: &str) -> String {
    s.chars()
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// `"U-0-1"`, as written on every sticker.
fn parse_facelet(key: &str) -> Option<(Face, u8, u8)> {
    let mut parts = key.split('-');
    let face = Face::from_letter(parts.next()?.chars().next()?)?;
    let row = parts.next()?.parse().ok()?;
    let col = parts.next()?.parse().ok()?;
    Some((face, row, col))
}

/// `"1,3,8-11"` as facelet numbers.
pub fn parse_highlight_spec(n: u8, spec: &str) -> Highlights {
    let mut out = Highlights::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (lo, hi) = part.split_once('-').unwrap_or((part, part));
        let (Ok(lo), Ok(hi)) = (lo.trim().parse::<u32>(), hi.trim().parse::<u32>()) else {
            continue;
        };
        for label in lo..=hi.min(lo + 999) {
            if let Some(facelet) = facelet_from_label(n, label) {
                out.insert(facelet);
            }
        }
    }
    out
}

/// Fill in the camera preset menu.
pub fn fill_presets() {
    let mut html = String::new();
    for (name, _, _) in CAMERA_PRESETS {
        html.push_str(&format!("<option>{name}</option>"));
    }
    dom::set_html("preset", &html);
}
