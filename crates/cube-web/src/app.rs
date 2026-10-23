//! The application state and everything it draws.

use crate::dom;
use crate::editor;
use crate::scan::{Scan, Shot};
use cube_core::entry::Editor as Entry;
use cube_core::export::{self, ease};
use cube_core::geom::{Face, facelet_from_label, facelet_label};
use cube_core::moves::Move;
use cube_core::notation;
use cube_core::palette::{Palette, Rgb, STANDARD};
use cube_core::render::cube3d::{self, Anim, CAMERA_PRESETS, Camera, Style3d};
use cube_core::render::net::{self, NetStyle};
use cube_core::render::{Highlights, Marks};
use cube_core::scramble::{Rng, scramble};
use cube_core::solver::search::Limits;
use cube_core::solver::{Kociemba, SolveError};
use cube_core::{Cube, Permutation, Turn};

/// A sticker the editor has not been given a color for yet.
///
/// Dark on purpose. White is one of the cube's own six colors, so a pale
/// blank reads as a white sticker rather than as an empty socket, and the
/// one thing the twin view has to show is which stickers are still missing.
const UNSET: Rgb = Rgb::from_hex(0x4b4b57);

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
    /// The cube being typed in, while the editor is open.
    pub entry: Entry,
    /// The camera, while it is reading a cube into the editor.
    pub scan: Scan,
    /// Whether the editor has the views: both of them show what is being
    /// entered rather than the state on the table.
    pub editing: bool,
    /// The sticker picked in a view, waiting for a color.
    pub entry_pick: Option<(Face, u8, u8)>,
    /// Set while a drag turns the camera, so letting go does not also count
    /// as a click on whatever sticker ended up under the pointer.
    pub dragged: bool,
    /// Built on the first solve and kept: the tables cost a moment.
    solver: Option<Kociemba>,
    /// Where phase 1 ends, when the maneuver came from the solver.
    phase_split: Option<usize>,
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
            entry: Entry::new(),
            scan: Scan::new(),
            editing: false,
            entry_pick: None,
            dragged: false,
            solver: None,
            phase_split: None,
        }
    }

    // ----- drawing -------------------------------------------------------

    pub fn render(&self) {
        self.render_3d(None);
        self.render_state();
        self.render_solution();
        self.render_transport();
    }

    /// Everything that changes when the cube does.
    pub fn render_state(&self) {
        self.render_net();
        self.render_permutation();
    }

    /// The highlight set, as something the renderers can draw.
    fn marks(&self) -> Marks {
        Marks::rings(&self.highlights, self.palette.highlight)
    }

    pub fn render_3d(&self, anim: Option<Anim>) {
        let (cube, pal, marks) = self.shown();
        dom::set_html(
            "view-3d",
            &cube3d::inner(&cube, &self.cam, anim.as_ref(), &self.style, &pal, &marks),
        );
    }

    /// What the two views draw: the state on the table, or, while the editor
    /// is open, the cube as far as it has been entered.
    ///
    /// A part-entered cube is not a `Cube` -- it has holes in it -- so it is
    /// drawn as a blank cube with the decided stickers painted on, which is
    /// what `Marks` is for. Both views take the same treatment, so the flat
    /// grid, the net and the cube in three dimensions always agree.
    fn shown(&self) -> (Cube, Palette, Marks) {
        if !self.editing {
            return (self.cube.clone(), self.palette, self.marks());
        }
        let mut marks = Marks::new();
        for face in Face::ALL {
            for row in 0..3 {
                for col in 0..3 {
                    if let Some(color) = self.entry.get((face, row, col)) {
                        marks.paint((face, row, col), self.palette.face(color));
                    }
                }
            }
        }
        if let Some(pick) = self.entry_pick {
            marks.ring_at(pick, self.palette.highlight);
        }
        // The palette the decided stickers are painted from is the real one;
        // only the ground under them is replaced.
        let blank = Palette {
            faces: [UNSET; 6],
            ..self.palette
        };
        (Cube::solved(3), blank, marks)
    }

    pub fn render_net(&self) {
        let (w, h) = self.net.extent(self.n);
        let svg = dom::by_id("view-net");
        let _ = svg.set_attribute("viewBox", &format!("0 0 {w} {h}"));
        let (cube, pal, marks) = self.shown();
        svg.set_inner_html(&net::markup(&cube, &self.net, &pal, &marks));
    }

    /// The state as a permutation of the numbered facelets, before the
    /// maneuver and as it stands now.
    fn render_permutation(&self) {
        let Some(start) = Permutation::of(&self.base) else {
            dom::set_html(
                "perm-body",
                &format!(
                    "<p class=\"perm-note\">A {n}x{n}x{n} has interchangeable stickers, \
                     so its state does not name one permutation.</p>",
                    n = self.n
                ),
            );
            return;
        };
        let mut html = String::new();
        html.push_str(&self.permutation_row("start", None, &start));
        if let Some(now) = Permutation::of(&self.cube) {
            let note = (!self.moves.is_empty())
                .then(|| format!("move {} of {}", self.cursor, self.moves.len()));
            html.push_str(&self.permutation_row("now", note.as_deref(), &now));
        }
        if !self.moves.is_empty() {
            let mut from_solved = Cube::solved(self.n);
            from_solved.apply_moves(&self.moves);
            if let Some(maneuver) = Permutation::of(&from_solved) {
                let note = notation::format(&self.moves);
                html.push_str(&self.permutation_row("maneuver", Some(&note), &maneuver));
            }
        }
        dom::set_html("perm-body", &html);
    }

    /// One labelled line of cycle notation, every number drawn as the facelet
    /// it stands for.
    fn permutation_row(&self, label: &str, note: Option<&str>, perm: &Permutation) -> String {
        let cycles = perm.cycles();
        let drawn = if cycles.is_empty() {
            "<span class=\"cycle\">()</span>".to_string()
        } else {
            cycles
                .iter()
                .map(|cycle| {
                    let chips: String = cycle.iter().map(|k| self.facelet_chip(*k)).collect();
                    format!("<span class=\"cycle\">({chips})</span>")
                })
                .collect()
        };
        let mut stat: Vec<String> = note.map(html_escape).into_iter().collect();
        if perm.is_identity() {
            stat.push("the identity: nothing has moved".to_string());
        } else {
            stat.push(format!(
                "order {} · {} of {} facelets moved · cycles {}",
                perm.order(),
                perm.moved(),
                perm.len(),
                perm.cycle_type()
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join("+")
            ));
        }
        format!(
            "<div class=\"perm-row\"><dt>{label}</dt>\
             <dd title=\"{}\"><code>{drawn}</code>\
             <span class=\"perm-stat\">{}</span></dd></div>",
            perm.to_cycle_notation(),
            stat.join(" · ")
        )
    }

    /// A facelet number as a little sticker: the color of the face it belongs
    /// to, as the net draws it when the cube is solved.
    fn facelet_chip(&self, number: u32) -> String {
        let Some((face, row, col)) = facelet_from_label(self.n, number) else {
            return format!("<span class=\"facelet\">{number}</span>");
        };
        let color = self.palette.face(face);
        let ink = if color.luma() > 0.55 {
            "#1a1a1f"
        } else {
            "#ffffff"
        };
        // Ringed here too when the facelet is ringed in the net.
        let ring = if self.highlights.contains(&(face, row, col)) {
            format!(";box-shadow:0 0 0 2px {}", self.palette.highlight.hex())
        } else {
            String::new()
        };
        format!(
            "<span class=\"facelet\" style=\"background:{};color:{ink}{ring}\" \
             data-facelet=\"{}-{row}-{col}\">{number}</span>",
            color.hex(),
            face.letter()
        )
    }

    /// The solution, with the move about to be played marked in whichever
    /// phase it belongs to.
    ///
    /// The two phases are the whole display: a solution already reads as a
    /// line of moves, so marking how far the stepper has got inside that
    /// line puts the answer where the reader is already looking, and keeps
    /// the move in the phase that explains what it is doing.
    fn render_solution(&self) {
        let Some(split) = self.phase_split else {
            dom::set_html("solution", "");
            return;
        };
        let (first, second) = self.moves.split_at(split.min(self.moves.len()));
        dom::set_html(
            "solution",
            &format!(
                "{}{}",
                self.phase_row(
                    "phase 1",
                    first,
                    0,
                    "into \u{27e8}U, D, R2, L2, F2, B2\u{27e9}",
                ),
                self.phase_row("phase 2", second, split, "inside it, to the solved cube"),
            ),
        );
    }

    /// One phase, each move a step the reader can jump to. `offset` is where
    /// this phase starts in the whole maneuver, which is what turns a move's
    /// place in the phase into its place in the stepper.
    fn phase_row(&self, label: &str, moves: &[Move], offset: usize, what: &str) -> String {
        let text = if moves.is_empty() {
            "<span class=\"move-none\">nothing to do</span>".to_string()
        } else {
            moves
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let step = offset + i;
                    let class = match step.cmp(&self.cursor) {
                        core::cmp::Ordering::Less => "move done",
                        core::cmp::Ordering::Equal => "move current",
                        core::cmp::Ordering::Greater => "move",
                    };
                    format!(
                        "<span class=\"{class}\" data-step=\"{}\" title=\"jump here\">{}</span>",
                        step + 1,
                        html_escape(&m.name)
                    )
                })
                .collect()
        };
        format!(
            "<div class=\"perm-row\"><dt>{label}</dt><dd><code>{text}</code>\
             <span class=\"perm-stat\">{} moves \u{b7} {what}</span></dd></div>",
            moves.len()
        )
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
            ("solve", self.n == 3),
            ("edit", self.n == 3),
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
        self.phase_split = None;
        dom::set_html("solution", "");
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
        if n != 3 {
            self.editor_close();
        }
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
            self.render_state();
            self.render_solution();
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

    /// Whether the tables are already built.
    pub fn solver_ready(&self) -> bool {
        self.solver.is_some()
    }

    /// Solve the cube as it stands, and load the answer as the maneuver.
    pub fn solve(&mut self) {
        if self.n != 3 {
            self.say("The solver knows the 3x3x3 only.", true);
            return;
        }
        if self.cube.is_solved() {
            self.say("Already solved.", false);
            return;
        }
        let solver = self.solver.get_or_insert_with(Kociemba::new);
        let started = dom::now();
        let found = solver.solve_with(&self.cube, Limits::default());
        let elapsed = dom::now() - started;
        let solution = match found {
            Ok(solution) => solution,
            Err(SolveError::Unreachable) => {
                self.say(
                    "These colors are not a cube that turning faces can reach.",
                    true,
                );
                return;
            }
            Err(e) => {
                self.say(&e.to_string(), true);
                return;
            }
        };

        let moves = solution.moves();
        let text = notation::format(&moves);
        dom::input("sequence").set_value(&text);
        // A maneuver is read against the canonical view the net is drawn
        // from, and the cube may have been turned to any angle while it was
        // being entered, so the camera goes back to it.
        self.cam = Camera::default();
        dom::select("preset").set_selected_index(0);
        self.base = self.cube.clone();
        self.moves = moves;
        self.cursor = 0;
        self.playing = false;
        self.running = None;
        self.phase_split = Some(solution.phase1.len());
        self.render();
        self.say(
            &format!(
                "Solved in {} moves: {} + {}. Found in {elapsed:.0} ms, {} nodes.",
                solution.len(),
                solution.phase1.len(),
                solution.phase2.len(),
                solution.nodes
            ),
            false,
        );
    }

    fn set_state(&mut self, cube: Cube) {
        self.cube = cube;
        self.base = self.cube.clone();
        self.cursor = 0;
        self.playing = false;
        self.running = None;
        self.phase_split = None;
        dom::set_html("solution", "");
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
        self.render_highlights();
    }

    pub fn set_highlights(&mut self, spec: &str) {
        self.highlights = parse_highlight_spec(self.n, spec);
        self.render_highlights();
    }

    /// Everything that shows a ring.
    fn render_highlights(&self) {
        self.render_net();
        self.render_3d(None);
        self.render_permutation();
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

    // ----- the editor ----------------------------------------------------

    pub fn render_editor(&self) {
        editor::render(&self.entry, &self.palette, self.entry_pick);
        self.render_3d(None);
        self.render_net();
        dom::input("editor-letters").set_value(
            &self
                .entry
                .cube()
                .map(|c| c.to_letters())
                .unwrap_or_default(),
        );
    }

    /// Open the editor on a blank cube: six centers and nothing else, which
    /// is what entering a cube means. Starting from the state already on the
    /// table is the other button.
    pub fn editor_open(&mut self) {
        self.entry = Entry::new();
        self.editing = true;
        self.entry_pick = None;
        let _ = dom::body().class_list().add_1("editing");
        let _ = dom::by_id("editor").remove_attribute("hidden");
        dom::reparent("fig-3d", "editor-stage");
        self.render_editor();
        self.say(
            "Blank cube: 48 facelets to go, 36 of them worth asking about.",
            false,
        );
    }

    /// Carry on from the state on the table rather than from nothing.
    pub fn editor_from_cube(&mut self) {
        self.entry = Entry::from_cube(&self.cube);
        self.editing = true;
        self.entry_pick = None;
        let _ = dom::body().class_list().add_1("editing");
        let _ = dom::by_id("editor").remove_attribute("hidden");
        dom::reparent("fig-3d", "editor-stage");
        self.render_editor();
        self.say("Editing the cube on the table.", false);
    }

    pub fn editor_close(&mut self) {
        self.editing = false;
        self.entry_pick = None;
        if self.scan.is_open() {
            self.scan.close();
        }
        let _ = dom::body().class_list().remove_1("editing");
        dom::reparent_first("fig-3d", "views");
        let _ = dom::by_id("editor").set_attribute("hidden", "");
        self.render();
        self.report_state();
    }

    /// Pick a sticker in either view, to give it a color from the strip
    /// under the cube. The two views and the grid stay the same cube, so a
    /// sticker can be reached whichever way round it is easier to see.
    /// Open a facelet's menu, or shut it if it is the one already open.
    pub fn editor_toggle_point(&mut self, key: &str) {
        let Some(facelet) = parse_facelet(key) else {
            return;
        };
        if self.entry_pick == Some(facelet) {
            self.editor_unpoint();
        } else {
            self.editor_point(key);
        }
    }

    pub fn editor_unpoint(&mut self) {
        self.entry_pick = None;
        self.render_editor();
    }

    pub fn editor_point(&mut self, key: &str) {
        let Some(facelet) = parse_facelet(key) else {
            return;
        };
        self.entry_pick = Some(facelet);
        self.render_editor();
    }

    /// Start from nothing but the six centers.
    pub fn editor_clear(&mut self) {
        self.entry.clear();
        self.render_editor();
        self.say(
            "Blank cube: 48 facelets to go, 36 of them worth asking about.",
            false,
        );
    }

    /// One facelet chosen, then everything that follows from it.
    ///
    /// An empty color takes the sticker back, and takes its cubie with it:
    /// a sticker whose neighbours are still entered is named by them and
    /// could only ever be given back the color it had.
    ///
    /// Nothing is settled after a take-back. Settling is what would undo it,
    /// since with nineteen pieces placed the twentieth has nowhere else to
    /// go; leaving it alone is what lets a second piece be freed and the two
    /// of them swapped.
    pub fn editor_set(&mut self, key: &str, letter: &str) {
        let Some(facelet) = parse_facelet(key) else {
            return;
        };
        self.entry_pick = None;
        let Some(color) = letter.chars().next().and_then(Face::from_letter) else {
            self.entry.clear_cubie(facelet);
            self.render_editor();
            self.say(
                "Taken back, with the rest of its piece. Free another piece to \
                 swap the two of them.",
                false,
            );
            return;
        };
        self.entry.choose(facelet, color);
        let settled = self.entry.settle();
        self.render_editor();
        if settled > 0 {
            self.say(
                &format!(
                    "{settled} more facelet{} followed from that.",
                    if settled == 1 { "" } else { "s" }
                ),
                false,
            );
        }
    }

    /// Take one shot, unless the face on show is not the one asked for.
    pub fn scan_capture(&mut self) {
        match self.scan.capture(&self.palette) {
            Shot::Taken => {
                self.render_scan();
                self.report_capture();
            }
            Shot::NotReady => self.say("The camera has not started yet.", true),
            Shot::WrongFace { wanted, looks_like } => self.say(
                &format!(
                    "That looks like the {} face, not {}. Turn the cube and try again.",
                    looks_like.letter(),
                    wanted.letter()
                ),
                true,
            ),
        }
    }

    fn report_capture(&self) {
        let left = 6 - self.scan.taken();
        self.say(
            &match left {
                0 => "All six faces taken. Read them into the editor.".to_string(),
                1 => "One face to go.".to_string(),
                n => format!("{n} faces to go."),
            },
            false,
        );
    }

    pub fn render_scan(&self) {
        self.scan.render(&self.palette);
    }

    /// Hand the six captured faces to the editor.
    ///
    /// Not as a state to accept: as a proposal. A camera gets a sticker
    /// wrong now and then, and one wrong sticker would make the whole
    /// reading impossible, so the stickers it was sure of are offered first
    /// and the doubtful ones only go in if they can still be true beside
    /// them. Whatever is left over stays open, with its menu ready.
    pub fn scan_apply(&mut self) {
        let Some(samples) = self.scan.samples() else {
            self.say("All six faces have to be captured first.", true);
            return;
        };
        self.scan.close();
        let reading = cube_core::scan::read(&samples);
        let doubtful: Vec<_> = reading.unsure.clone();
        let mut order: Vec<_> = all_facelets().filter(|f| !doubtful.contains(f)).collect();
        order.extend(doubtful);
        self.entry = Entry::new();
        self.entry.absorb(&reading.cube, &order);
        self.entry_pick = None;
        self.render_editor();
        let left = self.entry.undecided();
        self.say(
            &match left {
                0 => "Read off the camera, whole. Check it and use it.".to_string(),
                1 => {
                    "Read off the camera. One sticker would not fit; its menu is open.".to_string()
                }
                n => format!(
                    "Read off the camera. {n} stickers would not fit; their menus are open."
                ),
            },
            false,
        );
    }

    /// Read a whole state from the letters, the format `?state=` carries.
    pub fn editor_letters(&mut self, text: &str) {
        let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let Some(cube) = Cube::from_letters(3, &text) else {
            self.say(
                &format!(
                    "That is {} letters of U L F R B D; a 3x3 wants 54.",
                    text.chars().count()
                ),
                true,
            );
            return;
        };
        self.entry = Entry::from_cube(&cube);
        self.render_editor();
        self.say("Letters read.", false);
    }

    /// Put the typed cube on the table.
    pub fn editor_apply(&mut self) {
        let Some(cube) = self.entry.cube() else {
            self.say("Not every facelet has a color yet.", true);
            return;
        };
        self.set_state(cube);
        self.editor_close();
        self.report_colors();
    }

    /// What the colors on the cube amount to: a complaint if they are not a
    /// state, and the usual report if they are.
    pub fn report_colors(&self) {
        if self.n == 3
            && let Err(flaw) = cube_core::solver::check(&self.cube)
        {
            let message = match flaw {
                cube_core::solver::Flaw::Pieces => {
                    "Some piece shows a pair of colors no cubie has."
                }
                cube_core::solver::Flaw::Parity => {
                    "Real pieces, but no sequence of turns reaches them: one is \
                     swapped, flipped or twisted on its own."
                }
            };
            self.say(message, true);
            return;
        }
        self.report_state();
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
                    &self.marks(),
                    bg,
                );
                self.offer("cube-3d.svg", "image/svg+xml", &svg);
            }
            "svgnet" => {
                let svg = net::document(&self.cube, &self.net, &self.palette, &self.marks(), bg);
                self.offer("cube-net.svg", "image/svg+xml", &svg);
            }
            "strip" => {
                let svg = export::strip_svg(
                    &self.base,
                    &self.moves,
                    &self.cam,
                    &st,
                    &self.palette,
                    &self.marks(),
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
            &self.marks(),
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

/// Every facelet of a 3x3, in numbering order.
fn all_facelets() -> impl Iterator<Item = (Face, u8, u8)> {
    (0..54).map(|i| (Face::ALL[i / 9], ((i % 9) / 3) as u8, (i % 3) as u8))
}
