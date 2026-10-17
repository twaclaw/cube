//! Exports meant to be dropped into a blog: standalone SVG, an animated SVG
//! flipbook, and a self-contained HTML player.
//!
//! Every export goes through the same renderer as the screen, so a figure in a
//! post is pixel for pixel what the app showed.

use crate::cube::Cube;
use crate::moves::Move;
use crate::palette::Palette;
use crate::render::svg::{num, open};
use crate::render::{Camera, Marks, Style3d, cube3d, cube3d::Anim};

/// One drawn moment of a sequence.
pub struct Frame {
    /// SVG markup, without the enclosing `<svg>`.
    pub markup: String,
    pub label: String,
    pub ms: f64,
    /// A move has landed here: the cube is between turns, not mid turn. The
    /// player steps from one of these to the next.
    pub is_state: bool,
}

/// A whole sequence, drawn.
pub struct Timeline {
    pub frames: Vec<Frame>,
    pub size: f64,
}

impl Timeline {
    pub fn total_ms(&self) -> f64 {
        self.frames.iter().map(|f| f.ms).sum()
    }

    pub fn bytes(&self) -> usize {
        self.frames.iter().map(|f| f.markup.len()).sum()
    }
}

/// Smooth start and end, so a turn does not jerk.
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Draw a sequence. `steps` frames per move: 1 gives the states only, 6 to 10
/// give a smooth turn.
#[allow(clippy::too_many_arguments)]
pub fn timeline(
    cube: &Cube,
    moves: &[Move],
    cam: &Camera,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
    steps: usize,
    ms_per_move: f64,
) -> Timeline {
    let steps = steps.max(1);
    let mut state = cube.clone();
    let mut frames = vec![Frame {
        markup: cube3d::inner(&state, cam, None, st, pal, marks),
        label: "start".to_string(),
        ms: ms_per_move.max(1.0),
        is_state: true,
    }];
    for m in moves {
        let total = m.turn.angle();
        // The steps before the last are mid turn; the last one is the move
        // completed, which is the still picture of the new state.
        for s in 1..steps {
            let anim = Anim {
                turn: m.turn,
                angle: total * ease(s as f64 / steps as f64),
            };
            frames.push(Frame {
                markup: cube3d::inner(&state, cam, Some(&anim), st, pal, marks),
                label: m.name.clone(),
                ms: ms_per_move / steps as f64,
                is_state: false,
            });
        }
        state.apply(&m.turn);
        frames.push(Frame {
            markup: cube3d::inner(&state, cam, None, st, pal, marks),
            label: m.name.clone(),
            ms: ms_per_move / steps as f64,
            is_state: true,
        });
    }
    if let Some(last) = frames.last_mut() {
        last.ms = ms_per_move.max(1.0);
    }
    Timeline {
        frames,
        size: st.size,
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// An animated SVG: one group per frame, cycled with SMIL. Works inside an
/// `<img>` tag, so it drops straight into Markdown.
pub fn animated_svg(tl: &Timeline, background: Option<&str>) -> String {
    let total = tl.total_ms().max(1.0);
    let mut s = open(tl.size, tl.size, background);
    let mut elapsed = 0.0;
    for frame in &tl.frames {
        let start = elapsed / total;
        let end = (elapsed + frame.ms) / total;
        elapsed += frame.ms;
        let (values, times) = if start <= 0.0 {
            ("1;0".to_string(), format!("0;{}", num4(end)))
        } else if end >= 1.0 {
            ("0;1".to_string(), format!("0;{}", num4(start)))
        } else {
            (
                "0;1;0".to_string(),
                format!("0;{};{}", num4(start), num4(end)),
            )
        };
        s.push_str(&format!(
            "<g opacity=\"0\"><animate attributeName=\"opacity\" calcMode=\"discrete\" \
             values=\"{values}\" keyTimes=\"{times}\" dur=\"{}s\" repeatCount=\"indefinite\"/>{}</g>",
            num4(total / 1000.0),
            frame.markup
        ));
    }
    s.push_str("</svg>");
    s
}

fn num4(v: f64) -> String {
    let mut s = format!("{v:.4}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s.is_empty() { "0".to_string() } else { s }
}

/// A self-contained page: play, pause, step, scrub. No dependencies.
pub fn player_html(tl: &Timeline, title: &str, sequence: &str) -> String {
    let frames: Vec<String> = tl
        .frames
        .iter()
        .map(|f| format!("[\"{}\",{}]", escape_js(&f.markup), num(f.ms)))
        .collect();
    let labels: Vec<String> = tl
        .frames
        .iter()
        .map(|f| format!("\"{}\"", escape_js(&f.label)))
        .collect();
    // Frames where a move has landed: what the arrows step between.
    let states: Vec<String> = tl
        .frames
        .iter()
        .enumerate()
        .filter(|(_, f)| f.is_state)
        .map(|(i, _)| i.to_string())
        .collect();
    format!(
        r##"<!doctype html>
<meta charset="utf-8">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --ink: #16161a; --soft: #6b6b76; --line: #d5d5de; --bg: #fbfbfd; }}
@media (prefers-color-scheme: dark) {{
  :root {{ --ink: #ececf2; --soft: #9a9aa6; --line: #33333d; --bg: #14141a; }}
}}
body {{ margin: 0; background: var(--bg); color: var(--ink);
       font: 14px/1.5 ui-sans-serif, system-ui, sans-serif; }}
figure {{ margin: 0; display: grid; gap: .5rem; justify-items: center; padding: 1rem; }}
svg {{ width: min(100%, {size}px); height: auto; }}
.bar {{ display: flex; gap: .5rem; align-items: center; }}
button {{ font: inherit; color: inherit; background: none; border: 1px solid var(--line);
          border-radius: .5rem; padding: .25rem .6rem; cursor: pointer; }}
button:hover {{ border-color: var(--soft); }}
output {{ color: var(--soft); min-width: 6ch; font-variant-numeric: tabular-nums; }}
input[type=range] {{ accent-color: #2a78d6; }}
</style>
<figure>
  <svg id="view" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}"></svg>
  <div class="bar">
    <button id="play">&#9654; play</button>
    <button id="prev">&#9664;</button>
    <button id="next">&#9654;</button>
    <input id="scrub" type="range" min="0" max="{last}" value="0" step="1">
    <output id="label"></output>
  </div>
  <figcaption>{sequence}</figcaption>
</figure>
<script>
const F = [{frames}], L = [{labels}], S = [{states}];
const view = document.getElementById('view'), scrub = document.getElementById('scrub'),
      label = document.getElementById('label'), play = document.getElementById('play');
let i = 0, timer = null;
function moveNumber(k) {{
  let done = 0;
  for (const s of S) if (s <= k) done++;
  return done - 1;
}}
function show(k) {{
  i = (k + F.length) % F.length;
  view.innerHTML = F[i][0];
  scrub.value = i;
  label.textContent = L[i] + '  ' + Math.max(moveNumber(i), 0) + '/' + (S.length - 1);
}}
// The arrows jump a whole move; the slider still scrubs frame by frame.
function step(dir) {{
  if (dir > 0) {{
    for (const s of S) if (s > i) return show(s);
    return show(S[0]);
  }}
  for (let j = S.length - 1; j >= 0; j--) if (S[j] < i) return show(S[j]);
  return show(S[S.length - 1]);
}}
function stop() {{ clearTimeout(timer); timer = null; play.innerHTML = '&#9654; play'; }}
function tick() {{
  show(i + 1);
  if (i === F.length - 1) {{ stop(); return; }}
  timer = setTimeout(tick, F[i][1]);
}}
play.onclick = () => {{
  if (timer) return stop();
  if (i === F.length - 1) show(0);
  play.innerHTML = '&#10073;&#10073; pause';
  timer = setTimeout(tick, F[i][1]);
}};
document.getElementById('prev').onclick = () => {{ stop(); step(-1); }};
document.getElementById('next').onclick = () => {{ stop(); step(1); }};
scrub.oninput = () => {{ stop(); show(+scrub.value); }};
show(0);
</script>
"##,
        title = escape(title),
        sequence = escape(sequence),
        size = num(tl.size),
        last = tl.frames.len().saturating_sub(1),
        frames = frames.join(","),
        labels = labels.join(","),
        states = states.join(","),
    )
}

fn escape_js(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('<', "\\x3c")
}

/// Every state of a sequence side by side in one SVG: a step-by-step figure.
#[allow(clippy::too_many_arguments)]
pub fn strip_svg(
    cube: &Cube,
    moves: &[Move],
    cam: &Camera,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
    columns: usize,
    background: Option<&str>,
) -> String {
    let columns = columns.max(1);
    let caption = st.size * 0.12;
    let pad = st.size * 0.04;
    let cell_w = st.size + pad;
    let cell_h = st.size + caption + pad;
    let count = moves.len() + 1;
    let cols = count.min(columns);
    let rows = count.div_ceil(columns);
    let mut s = open(
        cols as f64 * cell_w + pad,
        rows as f64 * cell_h + pad,
        background,
    );
    let mut state = cube.clone();
    for idx in 0..count {
        let (row, col) = (idx / columns, idx % columns);
        let x = pad + col as f64 * cell_w;
        let y = pad + row as f64 * cell_h;
        let label = if idx == 0 {
            "start".to_string()
        } else {
            format!("{idx}. {}", moves[idx - 1].name)
        };
        s.push_str(&format!(
            "<g transform=\"translate({} {})\">{}<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" \
             font-family=\"ui-sans-serif,system-ui,sans-serif\" font-size=\"{}\" fill=\"{}\">{}</text></g>",
            num(x),
            num(y),
            cube3d::inner(&state, cam, None, st, pal, marks),
            num(st.size * 0.5),
            num(st.size + caption * 0.72),
            num(caption * 0.62),
            pal.edge.hex(),
            escape(&label)
        ));
        if idx < moves.len() {
            state.apply(&moves[idx].turn);
        }
    }
    s.push_str("</svg>");
    s
}

/// Markup that embeds the live app, which keeps the figure interactive.
pub fn embed_snippet(base_url: &str, query: &str, height: u32) -> String {
    format!(
        "<iframe src=\"{}?{}\" width=\"100%\" height=\"{height}\" loading=\"lazy\" \
         style=\"border:0;max-width:100%\" title=\"Rubik's cube\"></iframe>",
        escape(base_url),
        escape(query)
    )
}
