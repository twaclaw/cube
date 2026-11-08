//! A tree of cube states, drawn as a node-link diagram.
//!
//! Every node carries the state it stands for, the number a table would store
//! for it, and whatever else the figure is about. The cubes are all the same
//! size, so a deeper tree costs width rather than detail, and each node opens a
//! panel with the cube at full size and its numbers written out. The panel is
//! CSS: hovering or clicking a node reveals it, and nothing in the figure needs
//! a script.
//!
//! Each cube's geometry is written once into a `<symbol>` and placed twice with
//! `<use>`, small in the tree and large in the panel, so the second copy costs
//! one tag rather than another cube.

use cube_core::Cube;
use cube_core::palette::{self, Palette};
use cube_core::render::Marks;
use cube_core::render::cube3d::{self, Camera, Style3d};
use cube_core::render::svg::{P2, num, open};

/// User units each cube is drawn in before `<use>` scales it to a box.
pub(crate) const CUBE_UNITS: f64 = 300.0;

/// One row of the panel a node opens.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Detail {
    pub key: String,
    pub value: String,
}

impl Detail {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// What a node means for the figure it is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mark {
    /// A value seen here for the first time: the table gains an entry.
    #[default]
    Fresh,
    /// A value the table already holds, reached again by a longer way.
    Seen,
    /// What the figure was looking for.
    Goal,
    /// A branch the search cut.
    Pruned,
}

impl Mark {
    fn color(self) -> &'static str {
        match self {
            Mark::Fresh => "#2a78d6",
            Mark::Seen => "#8a8a85",
            Mark::Goal => "#0ca30c",
            Mark::Pruned => "#8a8a85",
        }
    }

    fn dash(self) -> &'static str {
        match self {
            Mark::Fresh | Mark::Goal => "",
            Mark::Seen => " stroke-dasharray=\"5 4\"",
            Mark::Pruned => " stroke-dasharray=\"2 4\"",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mark::Fresh => "new to the table",
            Mark::Seen => "already in the table",
            Mark::Goal => "the goal",
            Mark::Pruned => "cut by the bound",
        }
    }
}

/// One node: a state, the number stored for it, and the rest of the story.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node {
    /// The move that reached it, as written. Empty at the root.
    pub label: String,
    pub cube: Cube,
    /// The number a table would keep for this node.
    pub value: String,
    pub detail: Vec<Detail>,
    pub mark: Mark,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(label: impl Into<String>, cube: Cube, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            cube,
            value: value.into(),
            detail: Vec::new(),
            mark: Mark::Fresh,
            children: Vec::new(),
        }
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.detail.push(Detail::new(key, value));
        self
    }

    pub fn marked(mut self, mark: Mark) -> Self {
        self.mark = mark;
        self
    }

    /// Nodes in the whole subtree, this one included.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    fn leaves(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            self.children.iter().map(Node::leaves).sum()
        }
    }
}

/// Sizes and the camera, all in user units.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Side of a node's box in the tree.
    pub node: f64,
    /// The panel's cube, as a multiple of `node`.
    pub zoom: f64,
    /// Space between one level and the next.
    pub gap_x: f64,
    /// Space between one row and the next.
    pub gap_y: f64,
    pub font: f64,
    pub camera: Camera,
    /// Flat notebook colors rather than shaded ones.
    pub flat: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            node: 46.0,
            zoom: 3.4,
            gap_x: 62.0,
            gap_y: 18.0,
            font: 11.0,
            camera: Camera {
                yaw: 34.0,
                pitch: 26.0,
                zoom: 1.0,
            },
            flat: false,
        }
    }
}

impl Style {
    fn row(&self) -> f64 {
        self.node + self.gap_y + 2.4 * self.font
    }

    fn step(&self) -> f64 {
        self.node + self.gap_x
    }

    fn style3d(&self) -> Style3d {
        Style3d {
            size: CUBE_UNITS,
            // Thicker than the default, because the small copy is a sixth of
            // the size the stroke was chosen for.
            stroke: 2.4,
            shade: if self.flat {
                0.0
            } else {
                Style3d::default().shade
            },
            ..Style3d::default()
        }
    }

    fn palette(&self) -> Palette {
        palette::STANDARD
    }
}

/// A tree, ready to draw.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tree {
    pub root: Node,
    /// A line under the figure saying what it is.
    pub caption: String,
    /// The marks this figure uses and what to call them, in listing order.
    pub legend: Vec<(Mark, String)>,
}

impl Tree {
    pub fn new(root: Node, caption: impl Into<String>) -> Self {
        let mut marks = Vec::new();
        collect_marks(&root, &mut marks);
        Self {
            root,
            caption: caption.into(),
            legend: marks
                .into_iter()
                .map(|m| (m, m.name().to_string()))
                .collect(),
        }
    }

    /// Call one of the marks something else, because a figure about a search
    /// does not mean by "new" what a figure about a table means.
    pub fn legend_as(mut self, mark: Mark, name: &str) -> Self {
        for entry in &mut self.legend {
            if entry.0 == mark {
                entry.1 = name.to_string();
            }
        }
        self
    }

    /// Nodes in the tree.
    pub fn len(&self) -> usize {
        self.root.count()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Where every node sits, in drawing order.
    fn layout(&self, st: &Style) -> Vec<Placed<'_>> {
        let mut out = Vec::with_capacity(self.len());
        let mut row = 0.0;
        place(&self.root, 0, None, st, &mut row, &mut out);
        out
    }

    /// One SVG document: the whole figure, panels and all, with no script.
    pub fn svg(&self, st: &Style) -> String {
        self.draw(st, None).0
    }

    /// The same figure in a little HTML, with a draggable cube when `app` is
    /// the base URL of the published app.
    ///
    /// The app reads a state out of its query string, so opening a node can
    /// point one lazily loaded iframe at that state. Nothing is fetched until a
    /// reader clicks.
    pub fn html(&self, st: &Style, app: Option<&str>) -> String {
        let (svg, id) = self.draw(st, app);
        let Some(base) = app else {
            return format!("<div class=\"cube-tree\">{svg}</div>");
        };
        let join = if base.contains('?') { "&" } else { "?" };
        format!(
            "<div class=\"cube-tree\" id=\"{id}-wrap\">{svg}\
<div style=\"margin:.5rem auto 0;max-width:30rem;text-align:center\">\
<div style=\"font-size:.8rem;opacity:.7;margin-bottom:.25rem\" id=\"{id}-say\">\
Click a node to turn its cube.</div>\
<iframe id=\"{id}-app\" title=\"the cube at that node\" loading=\"lazy\" \
style=\"width:100%;height:320px;border:0;display:none\"></iframe></div></div>\
<script>(function(){{\
var w=document.getElementById(\"{id}-wrap\"),f=document.getElementById(\"{id}-app\"),\
s=document.getElementById(\"{id}-say\");\
function open(t){{var g=t.closest(\"[data-state]\");if(!g)return;\
f.src=\"{base}{join}embed=1&views=3d&controls=1&state=\"+g.getAttribute(\"data-state\");\
f.style.display=\"block\";\
s.textContent=\"Drag to turn \"+(g.getAttribute(\"data-path\")||\"the solved cube\")+\".\";}}\
w.addEventListener(\"click\",function(e){{open(e.target);}});\
w.addEventListener(\"focusin\",function(e){{open(e.target);}});\
}})();</script>"
        )
    }

    /// The figure, and the id its CSS is scoped to.
    fn draw(&self, st: &Style, app: Option<&str>) -> (String, String) {
        let placed = self.layout(st);
        let rows = self.root.leaves() as f64;
        let deep = placed.iter().map(|p| p.depth).max().unwrap_or(0) as f64;
        let pad = 1.6 * st.font;
        // A value sits under its node, centred, and can be wider than the node
        // box; at the first and last level there is no neighbour to lend room,
        // so the margins have to.
        let spill = |at: usize| {
            placed
                .iter()
                .filter(|p| p.depth == at)
                .map(|p| (text_w(&p.node.value, st.font) - st.node) / 2.0)
                .fold(0.0, f64::max)
                .max(0.0)
        };
        let (left, right) = (spill(0), spill(deep as usize));
        let content = left + deep * st.step() + st.node + right;
        let legend_w = self.legend_width(st);
        let caption_w = text_w(&self.caption, st.font * 1.05);
        let width = (content + 2.0 * pad)
            .max(caption_w + 2.0 * pad)
            .max(legend_w + 2.0 * pad);
        let x0 = (width - content) / 2.0 + left;
        let caption_h = if self.caption.is_empty() {
            0.0
        } else {
            2.2 * st.font
        };
        let legend_h = if self.legend.len() > 1 {
            2.0 * st.font
        } else {
            0.0
        };
        let height = rows * st.row() + pad + caption_h + legend_h;
        let id = format!("ct{:x}", hash(&self.caption, self.len(), placed.len()));

        let mut defs = String::from("<defs>");
        let mut nodes = String::new();
        let mut edges = String::new();
        let mut panels = String::new();
        let mut rules = String::new();
        let style3d = st.style3d();
        let pal = st.palette();
        let marks = Marks::new();

        for p in &placed {
            let node = p.node;
            defs.push_str(&format!(
                "<symbol id=\"{id}-c{i}\" viewBox=\"0 0 {u} {u}\">{body}</symbol>",
                i = p.index,
                u = num(CUBE_UNITS),
                body = cube3d::inner(&node.cube, &st.camera, None, &style3d, &pal, &marks),
            ));
            let (x, y) = (x0 + p.depth as f64 * st.step(), pad + p.y);
            if let Some(parent) = p.parent {
                let from = &placed[parent];
                let (fx, fy) = (x0 + from.depth as f64 * st.step() + st.node, pad + from.y);
                edges.push_str(&format!(
                    "<path class=\"e\" d=\"M{} {}C{} {},{} {},{} {}\" fill=\"none\" \
                     stroke=\"{}\" stroke-width=\"2\" opacity=\".7\"{}/>",
                    num(fx),
                    num(fy),
                    num(fx + st.gap_x / 2.0),
                    num(fy),
                    num(x - st.gap_x / 2.0),
                    num(y),
                    num(x),
                    num(y),
                    node.mark.color(),
                    node.mark.dash(),
                ));
            }
            nodes.push_str(&self.node_markup(p, &id, [x, y], st, app));
            panels.push_str(&self.panel_markup(p, &id, [x, y], st, [width, height]));
            rules.push_str(&format!(
                "#{id}:hover .n{i}:hover~.h,#{id} .n{i}:focus~.h{{opacity:0}}\
                 #{id}:has(.n{i}:hover) .p{i},#{id}:has(.n{i}:focus) .p{i}{{opacity:1}}",
                i = p.index
            ));
        }
        defs.push_str("</defs>");

        let mut out = open(width, height, None);
        // Every word is grey: black disappears into a dark page, and white
        // into a light one. The card behind a panel is the one surface that
        // has to pick a side, so it follows the reader's theme.
        out.push_str(&format!(
            "<style>#{id} text{{font-family:ui-sans-serif,system-ui,-apple-system,sans-serif}}\
             #{id} .v{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}}\
             #{id} .t{{fill:#6f6f6a}}#{id} .s{{fill:#8e8e88}}\
             #{id} .card{{fill:#f7f7f5;stroke:#8a8a8566;stroke-width:1}}\
             #{id} .n{{cursor:zoom-in}}#{id} .n:focus{{outline:none;cursor:zoom-out}}\
             #{id} .p{{opacity:0;transition:opacity .12s ease-out;pointer-events:none}}\
             @media (prefers-color-scheme:dark){{\
             #{id} .t{{fill:#c3c2b7}}#{id} .s{{fill:#9a9a93}}\
             #{id} .card{{fill:#23231f;stroke:#8a8a8577}}}}\
             {rules}</style>"
        ));
        out.push_str(&defs);
        out.push_str(&format!("<g id=\"{id}\">"));
        out.push_str(&edges);
        out.push_str(&nodes);
        out.push_str(&panels);
        if !self.caption.is_empty() || !self.legend.is_empty() {
            out.push_str(&self.footer(st, width, height, caption_h, legend_h));
        }
        out.push_str("</g></svg>");
        (out, id)
    }

    fn node_markup(&self, p: &Placed, id: &str, at: P2, st: &Style, app: Option<&str>) -> String {
        let [x, y] = at;
        let node = p.node;
        let top = y - st.node / 2.0;
        let data = if app.is_some() {
            format!(
                " data-state=\"{}\" data-path=\"{}\"",
                node.cube.to_letters(),
                escape(&p.path)
            )
        } else {
            String::new()
        };
        let mut s = format!(
            "<g class=\"n n{i}\" tabindex=\"0\"{data}><title>{title}</title>",
            i = p.index,
            title = escape(&self.tooltip(node, &p.path)),
        );
        // A transparent box over the cube, so the whole node is one hit target.
        s.push_str(&format!(
            "<use href=\"#{id}-c{i}\" x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{w}\"/>\
             <rect x=\"{x}\" y=\"{top}\" width=\"{w}\" height=\"{w}\" fill=\"transparent\"/>",
            i = p.index,
            x = num(x),
            top = num(top),
            w = num(st.node),
        ));
        if !node.label.is_empty() {
            s.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"middle\">{}</text>",
                num(x + st.node / 2.0),
                num(top - 0.45 * st.font),
                num(st.font),
                escape(&node.label),
            ));
        }
        if !node.value.is_empty() {
            s.push_str(&format!(
                "<text class=\"v {}\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"middle\"{}>{}</text>",
                if node.mark == Mark::Fresh { "t" } else { "s" },
                num(x + st.node / 2.0),
                num(top + st.node + 1.15 * st.font),
                num(st.font),
                match node.mark {
                    Mark::Fresh | Mark::Seen => String::new(),
                    other => format!(" fill=\"{}\"", other.color()),
                },
                escape(&node.value),
            ));
        }
        s.push_str("</g>");
        s
    }

    /// The panel a node opens: the cube at full size, then its numbers.
    fn panel_markup(&self, p: &Placed, id: &str, at: P2, st: &Style, canvas: P2) -> String {
        let ([x, y], [width, height]) = (at, canvas);
        let node = p.node;
        let cube = st.node * st.zoom;
        let line = 1.55 * st.font;
        let text_w = node
            .detail
            .iter()
            .map(|d| 0.56 * st.font * (d.key.chars().count() + d.value.chars().count() + 3) as f64)
            .fold(cube, f64::max)
            .min(26.0 * st.font);
        let pw = text_w.max(cube) + 2.0 * st.font;
        let ph = cube + line * (node.detail.len() as f64 + 1.6) + 1.4 * st.font;
        // Beside the node if there is room, otherwise on its other side, and
        // never past an edge of the figure.
        let px = if x + st.node + 12.0 + pw <= width {
            x + st.node + 12.0
        } else {
            (x - 12.0 - pw).max(4.0)
        };
        let py = (y - ph / 2.0).clamp(4.0, (height - ph - 4.0).max(4.0));

        let mut s = format!("<g class=\"p p{}\" opacity=\"0\">", p.index);
        s.push_str(&format!(
            "<rect class=\"card\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>",
            num(px),
            num(py),
            num(pw),
            num(ph),
        ));
        s.push_str(&format!(
            "<use href=\"#{id}-c{i}\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{w}\"/>",
            i = p.index,
            x = num(px + (pw - cube) / 2.0),
            y = num(py + 0.7 * st.font),
            w = num(cube),
        ));
        let mut ty = py + 0.7 * st.font + cube + line;
        let head = if p.path.is_empty() {
            "solved".to_string()
        } else {
            p.path.clone()
        };
        s.push_str(&format!(
            "<text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
             text-anchor=\"middle\">{}</text>",
            num(px + pw / 2.0),
            num(ty),
            num(st.font * 1.05),
            escape(&head),
        ));
        ty += line * 0.5;
        for d in &node.detail {
            ty += line;
            s.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>\
                 <text class=\"v t\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"end\">{}</text>",
                num(px + st.font),
                num(ty),
                num(st.font),
                escape(&d.key),
                num(px + pw - st.font),
                num(ty),
                num(st.font),
                escape(&d.value),
            ));
        }
        s.push_str("</g>");
        s
    }

    fn footer(&self, st: &Style, width: f64, height: f64, caption_h: f64, legend_h: f64) -> String {
        let mut s = String::new();
        let mut y = height - caption_h - legend_h + 1.4 * st.font;
        if self.legend.len() > 1 {
            let mut x = (width - self.legend_width(st)) / 2.0;
            for (mark, name) in &self.legend {
                s.push_str(&format!(
                    "<path d=\"M{} {}h{}\" stroke=\"{}\" stroke-width=\"2\"{}/>\
                     <text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
                    num(x),
                    num(y - 0.3 * st.font),
                    num(1.8 * st.font),
                    mark.color(),
                    mark.dash(),
                    num(x + 2.4 * st.font),
                    num(y),
                    num(st.font),
                    escape(name),
                ));
                x += 2.4 * st.font + text_w(name, st.font) + 1.8 * st.font;
            }
            y += 1.9 * st.font;
        }
        if !self.caption.is_empty() {
            s.push_str(&format!(
                "<text class=\"s\" x=\"{}\" y=\"{}\" font-size=\"{}\" \
                 text-anchor=\"middle\">{}</text>",
                num(width / 2.0),
                num(y),
                num(st.font * 1.05),
                escape(&self.caption),
            ));
        }
        s
    }

    /// How wide the legend needs to be, from the names it prints.
    fn legend_width(&self, st: &Style) -> f64 {
        if self.legend.len() < 2 {
            return 0.0;
        }
        self.legend
            .iter()
            .map(|(_, name)| 2.4 * st.font + text_w(name, st.font) + 1.8 * st.font)
            .sum::<f64>()
            - 1.8 * st.font
    }

    fn tooltip(&self, node: &Node, path: &str) -> String {
        let mut s = if path.is_empty() {
            "solved".to_string()
        } else {
            path.to_string()
        };
        if !node.value.is_empty() {
            s.push_str(&format!("  {}", node.value));
        }
        for d in &node.detail {
            s.push_str(&format!("\n{}: {}", d.key, d.value));
        }
        s
    }
}

/// A node, once the layout knows where it goes.
struct Placed<'a> {
    node: &'a Node,
    depth: usize,
    /// Centre of the node box, before the figure's padding.
    y: f64,
    index: usize,
    parent: Option<usize>,
    /// The moves that reach it, written out.
    path: String,
}

fn place<'a>(
    node: &'a Node,
    depth: usize,
    parent: Option<usize>,
    st: &Style,
    row: &mut f64,
    out: &mut Vec<Placed<'a>>,
) -> usize {
    let index = out.len();
    let path = match parent {
        Some(p) if out[p].path.is_empty() => node.label.clone(),
        Some(p) => format!("{} {}", out[p].path, node.label),
        None => String::new(),
    };
    out.push(Placed {
        node,
        depth,
        y: 0.0,
        index,
        parent,
        path,
    });
    if node.children.is_empty() {
        let y = *row * st.row() + st.row() / 2.0;
        *row += 1.0;
        out[index].y = y;
        return index;
    }
    let kids: Vec<usize> = node
        .children
        .iter()
        .map(|kid| place(kid, depth + 1, Some(index), st, row, out))
        .collect();
    let first = out[kids[0]].y;
    let last = out[*kids.last().expect("a child")].y;
    out[index].y = (first + last) / 2.0;
    index
}

fn collect_marks(node: &Node, into: &mut Vec<Mark>) {
    if !into.contains(&node.mark) {
        into.push(node.mark);
    }
    for kid in &node.children {
        collect_marks(kid, into);
    }
}

/// Enough of a hash to keep two figures on one page from sharing a CSS scope.
pub(crate) fn hash(caption: &str, nodes: usize, placed: usize) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in caption.as_bytes() {
        h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
    }
    for v in [nodes as u64, placed as u64] {
        h = (h ^ v).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A rough width for a run of text: the figure only needs to know how much
/// room to leave, so this errs generous.
pub(crate) fn text_w(s: &str, font: f64) -> f64 {
    0.58 * font * s.chars().count() as f64
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(label: &str, value: &str) -> Node {
        Node::new(label, Cube::solved(3), value)
    }

    fn sample() -> Tree {
        let mut root = leaf("", "0");
        let mut first = leaf("U", "1");
        first.children = vec![leaf("U", "2"), leaf("R2", "3").marked(Mark::Seen)];
        root.children = vec![first, leaf("R2", "4")];
        Tree::new(root, "a sample")
    }

    #[test]
    fn counts_its_nodes() {
        assert_eq!(sample().len(), 5);
    }

    #[test]
    fn every_node_is_placed_once_and_parents_centre_on_children() {
        let st = Style::default();
        let tree = sample();
        let placed = tree.layout(&st);
        assert_eq!(placed.len(), tree.len());
        for (i, p) in placed.iter().enumerate() {
            assert_eq!(p.index, i);
        }
        // Three leaves, each on its own row, none overlapping.
        let mut leaves: Vec<f64> = placed
            .iter()
            .filter(|p| p.node.children.is_empty())
            .map(|p| p.y)
            .collect();
        leaves.sort_by(f64::total_cmp);
        assert_eq!(leaves.len(), 3);
        for pair in leaves.windows(2) {
            assert!(pair[1] - pair[0] >= st.node, "rows overlap: {pair:?}");
        }
        for p in &placed {
            if p.node.children.is_empty() {
                continue;
            }
            let kids: Vec<f64> = placed
                .iter()
                .filter(|k| k.parent == Some(p.index))
                .map(|k| k.y)
                .collect();
            let mid = (kids.iter().cloned().fold(f64::MAX, f64::min)
                + kids.iter().cloned().fold(f64::MIN, f64::max))
                / 2.0;
            assert!((p.y - mid).abs() < 1e-9, "parent off its children");
        }
    }

    #[test]
    fn paths_accumulate_the_moves() {
        let tree = sample();
        let placed = tree.layout(&Style::default());
        let paths: Vec<&str> = placed.iter().map(|p| p.path.as_str()).collect();
        assert!(paths.contains(&""), "the root has no path");
        assert!(paths.contains(&"U U"));
        assert!(paths.contains(&"U R2"));
    }

    #[test]
    fn svg_has_one_cube_and_one_panel_per_node() {
        let tree = sample();
        let svg = tree.svg(&Style::default());
        assert!(svg.starts_with("<svg "), "an svg document");
        assert!(svg.ends_with("</svg>"));
        assert_eq!(svg.matches("<symbol ").count(), tree.len());
        // One <use> in the tree and one in the panel, per node.
        assert_eq!(svg.matches("<use ").count(), 2 * tree.len());
        assert_eq!(svg.matches("class=\"p p").count(), tree.len());
        // One edge per node but the root.
        assert_eq!(svg.matches("<path class=\"e\"").count(), tree.len() - 1);
        assert!(svg.contains("stroke-dasharray"), "the seen node is dashed");
        assert!(svg.contains(":has("), "panels open on hover or focus");
        assert!(!svg.contains("<script"), "the svg carries no script");
    }

    #[test]
    fn html_adds_a_player_only_when_an_app_is_given() {
        let tree = sample();
        let st = Style::default();
        let plain = tree.html(&st, None);
        assert!(!plain.contains("<iframe"));
        assert!(!plain.contains("<script"));
        let live = tree.html(&st, Some("https://example.com/cube/"));
        assert!(live.contains("<iframe"));
        assert!(live.contains("data-state=\""));
        assert!(
            live.contains("loading=\"lazy\""),
            "nothing fetched up front"
        );
        assert_eq!(live.matches("data-state=\"").count(), tree.len());
    }

    #[test]
    fn two_figures_do_not_share_a_css_scope() {
        let st = Style::default();
        let one = Tree::new(leaf("", "0"), "first").svg(&st);
        let two = Tree::new(leaf("", "0"), "second").svg(&st);
        let id = |s: &str| s[s.find("#ct").unwrap()..][..8].to_string();
        assert_ne!(id(&one), id(&two));
    }
}
