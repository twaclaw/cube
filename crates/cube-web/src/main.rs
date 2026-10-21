//! Wiring: the page's controls, the camera drag and the animation loop.

mod app;
mod dom;

use app::App;
use cube_core::Cube;
use cube_core::render::cube3d::CAMERA_PRESETS;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{KeyboardEvent, MouseEvent, PointerEvent, WheelEvent};

type Shared = Rc<RefCell<App>>;
/// Holds the frame callback alive for as long as the animation runs.
type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn now() -> f64 {
    dom::now()
}

/// Show the figure the view select names, hiding the rest.
///
/// An empty name is the default: the two cube views share the row, with the
/// permutation panel underneath. That panel is the one asymmetry -- it is a
/// page of text rather than a picture, so an embed leaves it out unless it
/// was asked for by name. Whether this is an embed is read back off the body
/// rather than threaded through, so the change handler needs no state.
fn apply_view(view: &str) {
    let classes = dom::document().body().expect("a body").class_list();
    for name in ["hide-3d", "hide-net", "hide-perm"] {
        let _ = classes.remove_1(name);
    }
    let hidden: &[&str] = match view {
        "3d" => &["hide-net", "hide-perm"],
        "net" => &["hide-3d", "hide-perm"],
        "perm" => &["hide-3d", "hide-net"],
        _ if classes.contains("embed") => &["hide-perm"],
        _ => &[],
    };
    for name in hidden {
        let _ = classes.add_1(name);
    }
}

fn main() {
    console_error_panic_hook::set_once();
    app::fill_presets();

    let params = dom::params();
    // A reloaded page comes back with the controls as the browser left them.
    // The query string wins; otherwise take the controls at their word.
    let restored = |id: &str| dom::select(id).value().parse::<u8>().ok();
    let n = params
        .get("n")
        .and_then(|v| v.parse::<u8>().ok())
        .or_else(|| restored("size"))
        .filter(|v| (2..=9).contains(v))
        .unwrap_or(3);
    let mut state = App::new(n);

    if let Some(letters) = params.get("state")
        && let Some(cube) = Cube::from_letters(n, &letters)
    {
        state.base = cube.clone();
        state.cube = cube;
    }
    if let Some(cam) = params.get("cam")
        && let Some((yaw, pitch)) = cam.split_once(',')
        && let (Ok(yaw), Ok(pitch)) = (yaw.trim().parse(), pitch.trim().parse())
    {
        state.cam.yaw = yaw;
        state.cam.pitch = pitch;
    }
    if let Some(zoom) = params.get("zoom").and_then(|v| v.parse().ok()) {
        state.cam.zoom = zoom;
    }
    state.cam = state.cam.clamped();
    state.numbers = match params.get("numbers") {
        Some(v) => v != "0",
        None => dom::input("numbers").checked(),
    };
    state.net.numbers = state.numbers;
    state.zoom_on = dom::input("zoom-on").checked();
    if let Some(speed) = params
        .get("speed")
        .and_then(|v| v.parse().ok())
        .or_else(|| dom::input("speed").value().parse().ok())
    {
        state.speed_ms = speed;
    }
    let highlights = params
        .get("hl")
        .unwrap_or_else(|| dom::input("highlight").value());
    state.highlights = app::parse_highlight_spec(n, &highlights);

    let embed = params.get("embed").is_some_and(|v| v != "0");
    let body = dom::document().body().expect("a body");
    let classes = body.class_list();
    if embed {
        let _ = classes.add_1("embed");
        if params.get("controls").is_some_and(|v| v != "0") {
            let _ = classes.add_1("with-controls");
        }
    }
    // ?views= names the one figure to keep, and is also where the view select
    // starts. Anything unrecognised falls back to the default rather than
    // leaving the select showing a value it does not have.
    let view = match params.get("views").as_deref() {
        Some("3d") => "3d",
        Some("net") => "net",
        Some("perm") => "perm",
        _ => "",
    };
    dom::select("view").set_value(view);
    apply_view(view);
    dom::on_id::<web_sys::Event, _>("view", "change", |_| {
        apply_view(&dom::select("view").value());
    });

    let shared: Shared = Rc::new(RefCell::new(state));
    wire(&shared);

    if let Some(seq) = params.get("seq") {
        dom::input("sequence").set_value(&seq);
        let result = shared.borrow_mut().load(&seq);
        if let Err(message) = result {
            shared.borrow().say(&message, true);
        } else if params.get("autoplay").is_some_and(|v| v != "0") {
            play(&shared);
        }
    }

    let app = shared.borrow();
    app.render();
    app.report_state();
    app.sync_controls();
}

/// Run one move, and keep running while the animation lasts.
fn step(shared: &Shared, forward: bool) {
    let started = shared.borrow_mut().begin(forward, now());
    if started {
        animate(shared.clone());
    }
}

fn play(shared: &Shared) {
    {
        let mut app = shared.borrow_mut();
        if app.playing {
            app.playing = false;
            app.render();
            return;
        }
        if !app.can_step(true) {
            app.jump_to(0);
        }
        app.playing = true;
    }
    step(shared, true);
}

/// The frame loop, alive only while a turn is being drawn.
fn animate(shared: Shared) {
    let holder: FrameLoop = Rc::new(RefCell::new(None));
    let keep = holder.clone();
    *keep.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        let again = shared.borrow_mut().frame(timestamp);
        if again {
            if let Some(closure) = holder.borrow().as_ref() {
                dom::request_frame(closure);
            }
        } else {
            holder.borrow_mut().take();
        }
    }) as Box<dyn FnMut(f64)>));
    if let Some(closure) = keep.borrow().as_ref() {
        dom::request_frame(closure);
    }
}

/// The views that can be pinned, each with the figure it marks.
const PINNABLE: [(&str, &str); 3] = [
    ("pin-3d", "fig-3d"),
    ("pin-net", "fig-net"),
    ("pin-perm", "fig-perm"),
];

/// Pin one view and clear the rest. `None` leaves the views as they were
/// written, which is what unticking the only ticked box does.
fn set_pinned(chosen: Option<&str>) {
    for (pin, figure) in PINNABLE {
        let on = chosen == Some(pin);
        dom::input(pin).set_checked(on);
        let classes = dom::by_id(figure).class_list();
        let _ = if on {
            classes.add_1("pinned")
        } else {
            classes.remove_1("pinned")
        };
    }
}

fn wire(shared: &Shared) {
    // the maneuver
    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("sequence-form", "submit", move |e| {
        e.prevent_default();
        let text = dom::input("sequence").value();
        // Loading only arms the maneuver; play or the stepper runs it.
        if let Err(message) = app.borrow_mut().load(&text) {
            app.borrow().say(&message, true);
        }
    });

    for (id, forward) in [("step-fwd", true), ("step-back", false)] {
        let app = shared.clone();
        dom::on_id::<MouseEvent, _>(id, "click", move |_| step(&app, forward));
    }

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("play", "click", move |_| play(&app));

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("to-start", "click", move |_| app.borrow_mut().jump_to(0));

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("to-end", "click", move |_| app.borrow_mut().apply_rest());

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("chips", "click", move |e| {
        let Some(target) = e.target() else { return };
        let element: web_sys::Element = target.unchecked_into();
        if let Some(step) = element.get_attribute("data-step")
            && let Ok(k) = step.parse::<usize>()
        {
            app.borrow_mut().jump_to(k);
        }
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("speed", "input", move |_| {
        let value = dom::input("speed").value().parse().unwrap_or(420.0);
        app.borrow_mut().speed_ms = value;
    });

    // whole states
    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("scramble", "click", move |_| {
        let seed = (js_sys::Math::random() * (u32::MAX as f64)) as u64
            | ((js_sys::Math::random() * (u32::MAX as f64)) as u64) << 32;
        app.borrow_mut().scramble(seed);
    });

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("solved", "click", move |_| app.borrow_mut().set_solved());

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("solve", "click", move |_| {
        // Building the tables blocks, so let the message paint first.
        let waiting = app.borrow().solver_ready();
        app.borrow().say(
            if waiting {
                "Solving..."
            } else {
                "Building the solver tables, a moment, once..."
            },
            false,
        );
        let app = app.clone();
        dom::defer(20, move || app.borrow_mut().solve());
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("size", "change", move |_| {
        let n = dom::select("size").value().parse().unwrap_or(3);
        app.borrow_mut().resize(n);
        let text = dom::input("sequence").value();
        if !text.trim().is_empty() {
            let result = app.borrow_mut().load(&text);
            if let Err(message) = result {
                app.borrow().say(&message, true);
            }
        }
    });

    // Pinning: at most one view is kept under the console on a narrow screen,
    // so that pressing play does not leave it off the top of the phone.
    for (pin, _) in PINNABLE {
        dom::on_id::<web_sys::Event, _>(pin, "change", move |_| {
            let chosen = dom::input(pin).checked().then_some(pin);
            set_pinned(chosen);
        });
    }

    // the views
    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("numbers", "change", move |_| {
        let on = dom::input("numbers").checked();
        let mut app = app.borrow_mut();
        app.numbers = on;
        app.net.numbers = on;
        app.render_net();
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("zoom-on", "change", move |_| {
        app.borrow_mut().zoom_on = dom::input("zoom-on").checked();
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("preset", "change", move |_| {
        let name = dom::select("preset").value();
        if let Some((_, yaw, pitch)) = CAMERA_PRESETS.iter().find(|(n, _, _)| *n == name) {
            let mut app = app.borrow_mut();
            app.cam.yaw = *yaw;
            app.cam.pitch = *pitch;
            app.cam = app.cam.clamped();
            app.render_3d(None);
        }
    });

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("view-reset", "click", move |_| {
        let mut app = app.borrow_mut();
        app.cam = Default::default();
        dom::select("preset").set_selected_index(0);
        app.render_3d(None);
    });

    // highlights
    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("view-net", "click", move |e| {
        let Some(target) = e.target() else { return };
        let element: web_sys::Element = target.unchecked_into();
        if let Some(key) = element.get_attribute("data-facelet") {
            app.borrow_mut().toggle_facelet(&key);
        }
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("perm-body", "click", move |e| {
        let Some(target) = e.target() else { return };
        let element: web_sys::Element = target.unchecked_into();
        if let Some(key) = element.get_attribute("data-facelet") {
            app.borrow_mut().toggle_facelet(&key);
        }
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("highlight", "input", move |_| {
        let spec = dom::input("highlight").value();
        app.borrow_mut().set_highlights(&spec);
    });

    let app = shared.clone();
    dom::on_id::<MouseEvent, _>("highlight-clear", "click", move |_| {
        dom::input("highlight").set_value("");
        app.borrow_mut().set_highlights("");
    });

    // the camera
    let app = shared.clone();
    dom::on_id::<PointerEvent, _>("view-3d", "pointerdown", move |e| {
        let svg = dom::by_id("view-3d");
        let _ = svg.set_pointer_capture(e.pointer_id());
        let _ = svg.class_list().add_1("dragging");
        app.borrow_mut().drag = Some((e.client_x() as f64, e.client_y() as f64));
    });

    let app = shared.clone();
    dom::on_id::<PointerEvent, _>("view-3d", "pointermove", move |e| {
        let mut app = app.borrow_mut();
        let Some((x, y)) = app.drag else { return };
        let (dx, dy) = (e.client_x() as f64 - x, e.client_y() as f64 - y);
        app.drag = Some((e.client_x() as f64, e.client_y() as f64));
        app.cam.yaw -= dx * 0.45;
        app.cam.pitch += dy * 0.40;
        app.cam = app.cam.clamped();
        if app.running.is_none() {
            app.render_3d(None);
        }
    });

    for event in ["pointerup", "pointercancel", "pointerleave"] {
        let app = shared.clone();
        dom::on_id::<PointerEvent, _>("view-3d", event, move |_| {
            let _ = dom::by_id("view-3d").class_list().remove_1("dragging");
            app.borrow_mut().drag = None;
        });
    }

    let app = shared.clone();
    dom::on_id::<WheelEvent, _>("view-3d", "wheel", move |e| {
        let mut app = app.borrow_mut();
        if !app.zoom_on {
            return;
        }
        e.prevent_default();
        app.cam.zoom *= (-e.delta_y() * 0.0015).exp();
        app.cam = app.cam.clamped();
        app.render_3d(None);
    });

    // exports
    let app = shared.clone();
    dom::on(
        dom::document()
            .query_selector(".menu-body")
            .ok()
            .flatten()
            .unwrap()
            .unchecked_ref(),
        "click",
        move |e: web_sys::Event| {
            let Some(target) = e.target() else { return };
            let element: web_sys::Element = target.unchecked_into();
            if let Some(what) = element.get_attribute("data-export") {
                app.borrow().export(&what);
            }
        },
    );

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("export-flat", "change", move |_| {
        app.borrow_mut().export_flat = dom::input("export-flat").checked();
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("export-bg", "change", move |_| {
        app.borrow_mut().export_bg = dom::input("export-bg").checked();
    });

    let app = shared.clone();
    dom::on_id::<web_sys::Event, _>("export-steps", "change", move |_| {
        let steps = dom::input("export-steps").value().parse().unwrap_or(8);
        app.borrow_mut().export_steps = steps;
    });

    // keyboard
    let app = shared.clone();
    dom::on::<KeyboardEvent, _>(
        dom::document().unchecked_ref(),
        "keydown",
        move |e: KeyboardEvent| {
            let typing = dom::document()
                .active_element()
                .map(|el| el.tag_name())
                .is_some_and(|tag| tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT");
            if typing {
                return;
            }
            match e.key().as_str() {
                "ArrowRight" => step(&app, true),
                "ArrowLeft" => step(&app, false),
                " " => {
                    e.prevent_default();
                    play(&app);
                }
                _ => {}
            }
        },
    );
}
