//! Capture mode: reading a real cube through the camera.
//!
//! One face at a time. The overlay draws a square grid over the middle of the
//! frame, the user fills it with a face and presses the button, and the nine
//! cells are sampled where the stickers are. The six faces together are the
//! 54 colors [`cube_core::scan`] turns back into a state.
//!
//! Nothing here tries to find the cube in the picture. The grid is fixed, so
//! what the user sees over the video is exactly what gets measured, and a
//! misframed shot is obvious before it is taken rather than mysterious
//! afterwards.

use crate::dom;
use cube_core::Rgb;
use cube_core::geom::Face;
use cube_core::palette::Palette;
use cube_core::scan::Samples;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, MediaStream};

/// Side of one grid cell on the small canvas each frame is drawn into.
const CELL: u32 = 48;
/// Side of the whole sampling square there.
const GRID: u32 = 3 * CELL;
/// How much of the frame's short side the grid covers. The overlay in the
/// page is written to the same fraction, so the two agree by construction.
pub const COVERAGE: f64 = 0.72;
/// How much of a cell is actually read, centered: the rest is sticker edge,
/// the dark gap between stickers, and whatever the user's framing is off by.
const PATCH: f64 = 0.52;

/// The faces in the order they are asked for, which is numbering order: the
/// samples then land in the order [`cube_core::scan::read`] wants them.
const ORDER: [Face; 6] = Face::ALL;

pub struct Scan {
    stream: Option<MediaStream>,
    /// Colors taken off each face, in numbering order; `None` until shot.
    faces: [Option<[Rgb; 9]>; 6],
    /// Which face the prompt is asking for.
    at: usize,
}

impl Scan {
    pub fn new() -> Self {
        Self {
            stream: None,
            faces: [None; 6],
            at: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    /// Show the camera, starting again from the first face.
    pub fn open(&mut self, stream: MediaStream) {
        self.stop_stream();
        self.faces = [None; 6];
        self.at = 0;
        video().set_src_object(Some(&stream));
        let _ = video().play();
        self.stream = Some(stream);
        let _ = dom::by_id("scanner").remove_attribute("hidden");
    }

    /// Put the camera down. The light on the device goes out only when every
    /// track is stopped, not when the element lets go of the stream.
    pub fn close(&mut self) {
        self.stop_stream();
        video().set_src_object(None);
        let _ = dom::by_id("scanner").set_attribute("hidden", "");
    }

    fn stop_stream(&mut self) {
        let Some(stream) = self.stream.take() else {
            return;
        };
        for track in stream.get_tracks().iter() {
            let track: web_sys::MediaStreamTrack = track.unchecked_into();
            track.stop();
        }
    }

    /// Read the nine cells of the frame on screen right now.
    fn sample(&self) -> Option<[Rgb; 9]> {
        let video = video();
        let (w, h) = (video.video_width(), video.video_height());
        if w == 0 || h == 0 {
            return None;
        }
        // The element shows the video with `object-fit: cover` in a square,
        // so the centered square of the frame is precisely what is on screen.
        let side = COVERAGE * f64::from(w.min(h));
        let (sx, sy) = ((f64::from(w) - side) / 2.0, (f64::from(h) - side) / 2.0);
        let context = canvas_context()?;
        context
            .draw_image_with_html_video_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &video,
                sx,
                sy,
                side,
                side,
                0.0,
                0.0,
                f64::from(GRID),
                f64::from(GRID),
            )
            .ok()?;
        let data = context
            .get_image_data(0.0, 0.0, f64::from(GRID), f64::from(GRID))
            .ok()?
            .data();
        Some(core::array::from_fn(|cell| {
            patch(&data, (cell / 3) as u32, (cell % 3) as u32)
        }))
    }

    /// Store the frame on screen as the face being asked for, and move on to
    /// the next one still wanted.
    pub fn capture(&mut self) -> bool {
        let Some(colors) = self.sample() else {
            return false;
        };
        self.faces[self.at] = Some(colors);
        self.at = (self.at + 1..6)
            .chain(0..self.at)
            .find(|k| self.faces[*k].is_none())
            .unwrap_or(self.at);
        true
    }

    /// Work on a different face, to take it again or turn it.
    pub fn go_to(&mut self, k: usize) {
        if k < 6 {
            self.at = k;
        }
    }

    /// Turn the face on show a quarter turn clockwise, for a shot taken with
    /// the cube held the wrong way up.
    pub fn rotate(&mut self) -> bool {
        let Some(colors) = self.faces[self.at] else {
            return false;
        };
        // The cell that ends up at (row, col) is the one that was at
        // (2 - col, row).
        self.faces[self.at] = Some(core::array::from_fn(|i| {
            let (row, col) = (i / 3, i % 3);
            colors[(2 - col) * 3 + row]
        }));
        true
    }

    pub fn taken(&self) -> usize {
        self.faces.iter().filter(|f| f.is_some()).count()
    }

    /// Every face, flattened the way the reader wants them.
    pub fn samples(&self) -> Option<Samples> {
        let faces: Vec<[Rgb; 9]> = self.faces.iter().copied().collect::<Option<Vec<_>>>()?;
        Some(core::array::from_fn(|i| faces[i / 9][i % 9]))
    }

    // ----- the panel -----------------------------------------------------

    /// The prompt, the grid over the video and the six thumbnails.
    pub fn render(&self, pal: &Palette) {
        let face = ORDER[self.at];
        dom::set_html(
            "scan-prompt",
            &format!(
                "Show the {} face, with {} at the top.",
                swatch(face, pal),
                swatch(above(face), pal)
            ),
        );
        dom::set_html("scan-lines", &overlay());
        dom::set_html("scan-faces", &self.thumbnails(pal));
        dom::set_text(
            "scan-shoot",
            if self.faces[self.at].is_some() {
                "take again"
            } else {
                "capture"
            },
        );
        let left = 6 - self.taken();
        dom::set_text(
            "scan-count",
            &match left {
                0 => "All six faces taken.".to_string(),
                1 => "One face to go.".to_string(),
                n => format!("{n} faces to go."),
            },
        );
        // Once the last face is in, reading it is the thing to do next, so
        // that is the button that looks like it.
        let ready = left == 0;
        let read: web_sys::HtmlButtonElement = dom::by_id("scan-read").unchecked_into();
        read.set_disabled(!ready);
        let _ = read.class_list().toggle_with_force("primary", ready);
        let _ = dom::by_id("scan-shoot")
            .class_list()
            .toggle_with_force("primary", !ready);
    }

    /// One block per face: the colors taken off it, or an empty outline.
    fn thumbnails(&self, pal: &Palette) -> String {
        let mut html = String::new();
        for (k, face) in ORDER.iter().enumerate() {
            let cells: String = match self.faces[k] {
                Some(colors) => colors
                    .iter()
                    .map(|c| format!("<i style=\"background:{}\"></i>", c.hex()))
                    .collect(),
                None => "<i></i>".repeat(9),
            };
            let here = if k == self.at { " at" } else { "" };
            html.push_str(&format!(
                "<li><button type=\"button\" class=\"thumb{here}\" data-face=\"{k}\" \
                 title=\"the {letter} face\"><span class=\"thumb-grid\">{cells}</span>\
                 <span class=\"thumb-name\"><i style=\"background:{}\"></i>{letter}</span>\
                 </button></li>",
                pal.face(*face).hex(),
                letter = face.letter(),
            ));
        }
        html
    }

    /// Paint the nine colors the grid is reading right now into the overlay,
    /// so a shot taken through a reflection or a shadow can be seen going
    /// wrong rather than found wrong afterwards.
    pub fn tick(&self) {
        let Some(colors) = self.sample() else {
            return;
        };
        let lo = 50.0 - COVERAGE * 50.0;
        let step = COVERAGE * 100.0 / 3.0;
        let dot = step * 0.2;
        let mut html = String::new();
        for (cell, color) in colors.iter().enumerate() {
            let (row, col) = (cell / 3, cell % 3);
            html.push_str(&format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{dot:.2}\" height=\"{dot:.2}\" \
                 rx=\"{:.2}\" fill=\"{}\"/>",
                lo + (col as f64 + 0.5) * step - dot / 2.0,
                lo + (row as f64 + 0.5) * step - dot / 2.0,
                dot * 0.25,
                color.hex(),
            ));
        }
        dom::set_html("scan-marks", &html);
    }
}

impl Default for Scan {
    fn default() -> Self {
        Self::new()
    }
}

/// Ask the browser for the back camera. The page must be a secure context,
/// which over the network means https: served plain over a LAN address there
/// is no camera to be had, and saying so is more use than a bare failure.
pub async fn request() -> Result<MediaStream, String> {
    let devices = dom::window()
        .navigator()
        .media_devices()
        .map_err(|_| "This page cannot reach a camera. A camera needs https, or localhost.")?;
    let wanted = web_sys::MediaTrackConstraints::new();
    wanted.set_facing_mode_str("environment");
    let constraints = web_sys::MediaStreamConstraints::new();
    constraints.set_audio_bool(false);
    constraints.set_video_media_track_constraints(&wanted);
    let promise = devices
        .get_user_media_with_constraints(&constraints)
        .map_err(|_| "This browser will not open a camera here.")?;
    let stream = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| "No camera: the browser refused, or there is none to use.")?;
    Ok(stream.unchecked_into())
}

/// The face shown above this one when it is square to the camera.
///
/// The net draws every face with a fixed neighbour above it -- U above the
/// side faces, B above U, F above D -- so naming that neighbour is all the
/// user needs to hold the cube the way the rest of the program reads it.
fn above(f: Face) -> Face {
    let up = f.down().scale(-1);
    Face::ALL
        .into_iter()
        .find(|g| g.normal() == up)
        .expect("every face has one above it")
}

/// A color as a small square, for naming a face without naming a color: the
/// cube in the user's hands is whatever colors it is.
fn swatch(f: Face, pal: &Palette) -> String {
    format!(
        "<span class=\"swatch\" style=\"background:{}\"></span>{}",
        pal.face(f).hex(),
        f.letter()
    )
}

/// The grid drawn over the video, in the same fraction of the frame that
/// [`Scan::sample`] reads. Corners only: the stickers should be visible, not
/// the lines.
fn overlay() -> String {
    let lo = 50.0 - COVERAGE * 50.0;
    let step = COVERAGE * 100.0 / 3.0;
    let tick = step * 0.3;
    let mut d = String::new();
    for row in 0..3 {
        for col in 0..3 {
            let (x, y) = (lo + col as f64 * step, lo + row as f64 * step);
            for (cx, cy, sx, sy) in [
                (x, y, 1.0, 1.0),
                (x + step, y, -1.0, 1.0),
                (x, y + step, 1.0, -1.0),
                (x + step, y + step, -1.0, -1.0),
            ] {
                d.push_str(&format!(
                    "M{:.2} {:.2}h{:.2}M{:.2} {:.2}v{:.2}",
                    cx,
                    cy,
                    sx * tick,
                    cx,
                    cy,
                    sy * tick
                ));
            }
        }
    }
    format!(
        "<path d=\"{d}\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"1.1\" \
         stroke-linecap=\"round\" opacity=\".9\"/>\
         <path d=\"{d}\" fill=\"none\" stroke=\"#101014\" stroke-width=\"2.4\" \
         stroke-linecap=\"round\" opacity=\".35\"/>"
    )
}

/// The color of one cell: the median of the pixels in the middle of it.
///
/// A median rather than a mean, because the bright speck a lamp leaves on a
/// glossy sticker would drag a mean a long way and moves a median not at all.
fn patch(data: &[u8], row: u32, col: u32) -> Rgb {
    let inset = ((1.0 - PATCH) * 0.5 * f64::from(CELL)) as u32;
    let lo = (row * CELL + inset, col * CELL + inset);
    let hi = ((row + 1) * CELL - inset, (col + 1) * CELL - inset);
    let mut channels: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for y in lo.0..hi.0 {
        for x in lo.1..hi.1 {
            let i = ((y * GRID + x) * 4) as usize;
            for (k, channel) in channels.iter_mut().enumerate() {
                channel.push(data[i + k]);
            }
        }
    }
    let mut median = [0u8; 3];
    for (k, channel) in channels.iter_mut().enumerate() {
        channel.sort_unstable();
        median[k] = channel[channel.len() / 2];
    }
    Rgb(median[0], median[1], median[2])
}

fn video() -> HtmlVideoElement {
    dom::by_id("scan-video").unchecked_into()
}

/// The offscreen canvas every frame is drawn into, kept at the grid's size so
/// the browser does the scaling and the averaging that goes with it.
fn canvas_context() -> Option<CanvasRenderingContext2d> {
    let canvas: HtmlCanvasElement = dom::by_id("scan-canvas").unchecked_into();
    if canvas.width() != GRID {
        canvas.set_width(GRID);
        canvas.set_height(GRID);
    }
    canvas
        .get_context("2d")
        .ok()
        .flatten()
        .map(|c| c.unchecked_into())
}
