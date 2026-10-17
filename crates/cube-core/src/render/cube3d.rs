//! The 3D view: a projected, depth sorted SVG of every visible cubie face.
//!
//! Vector output keeps the drawing crisp at any size and makes the on screen
//! view and the exported file the same thing.

use super::Marks;
use super::svg::{P2, num, open, rounded_path};
use crate::cube::Cube;
use crate::geom::{Axis, Face, V3, facelet_at, layer_index};
use crate::moves::Turn;
use crate::palette::{Palette, Rgb};

/// Camera distance, in units where the cube spans `[-1, 1]`.
const DIST: f64 = 7.0;
/// Radius of the sphere the cube turns inside. Framing uses it so that the
/// view neither breathes while dragging nor clips a mid-turn layer.
const SPHERE: f64 = 1.732_050_807_568_877_2;

/// Where the cube is seen from. Angles are degrees; `zoom` is a free multiplier.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Camera {
    /// Rotation about the vertical axis. Positive brings the R face forward.
    pub yaw: f64,
    /// Elevation. Positive looks down on the U face.
    pub pitch: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 38.0,
            pitch: 24.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub fn clamped(mut self) -> Self {
        self.pitch = self.pitch.clamp(-89.5, 89.5);
        self.yaw = self.yaw.rem_euclid(360.0);
        self.zoom = self.zoom.clamp(0.4, 3.0);
        self
    }
}

/// Named viewpoints, for reproducible figures.
pub const CAMERA_PRESETS: &[(&str, f64, f64)] = &[
    ("U F R", 38.0, 24.0),
    ("Isometric", 45.0, 35.26),
    ("U F L", -38.0, 24.0),
    ("D F R", 38.0, -24.0),
    ("Front", 0.0, 0.0),
    ("Right", 90.0, 0.0),
    ("Back", 180.0, 0.0),
    ("Left", 270.0, 0.0),
    ("Up", 0.0, 89.5),
    ("Down", 0.0, -89.5),
];

/// A turn caught part way through, for the animation.
#[derive(Clone, Copy, Debug)]
pub struct Anim {
    pub turn: Turn,
    /// Signed angle in radians, clockwise seen from `+turn.axis`.
    pub angle: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Style3d {
    /// Side of the square viewBox, in user units.
    pub size: f64,
    /// Fraction of the viewBox the cube may fill.
    pub fit: f64,
    /// Space left between cubies, as a fraction of a cubie.
    pub gap: f64,
    /// Sticker size, as a fraction of the cubie face.
    pub sticker: f64,
    /// Corner rounding, as a fraction of the edge length.
    pub round: f64,
    /// Strength of the directional shading; 0 gives flat notebook colors.
    pub shade: f64,
    /// Sticker outline width, in user units.
    pub stroke: f64,
}

impl Default for Style3d {
    fn default() -> Self {
        Self {
            size: 300.0,
            fit: 0.98,
            gap: 0.07,
            sticker: 0.84,
            round: 0.16,
            shade: 0.22,
            stroke: 0.9,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
    /// Cubie body, including the inner faces a turn exposes.
    Plastic,
    Sticker,
    /// Ring around a highlighted facelet.
    Ring,
}

/// One quad, projected and ready to write out.
#[derive(Clone, Copy, Debug)]
pub struct Facet {
    pub pts: [P2; 4],
    pub color: Rgb,
    pub paint: Paint,
    /// Distance from the camera; larger is further away.
    pub depth: f64,
    /// Corner rounding in user units.
    pub radius: f64,
    /// Part of the dark body behind the cubies, drawn first.
    pub behind: bool,
    pub facelet: Option<(Face, u8, u8)>,
}

fn view(p: [f64; 3], cam: &Camera) -> [f64; 3] {
    let yawed = Axis::Y.rotate_f64(p, cam.yaw.to_radians());
    Axis::X.rotate_f64(yawed, -cam.pitch.to_radians())
}

fn scale_for(st: &Style3d, cam: &Camera) -> f64 {
    let projected = SPHERE / (DIST * DIST - SPHERE * SPHERE).sqrt();
    st.size * 0.5 * st.fit / projected * cam.zoom
}

fn project(v: [f64; 3], scale: f64, size: f64) -> P2 {
    let w = (DIST - v[2]).max(0.1);
    [size * 0.5 + scale * v[0] / w, size * 0.5 - scale * v[1] / w]
}

fn centroid(pts: &[[f64; 3]]) -> [f64; 3] {
    let k = pts.len() as f64;
    let mut c = [0.0; 3];
    for p in pts {
        for i in 0..3 {
            c[i] += p[i] / k;
        }
    }
    c
}

/// Every visible quad of the cube, far to near.
pub fn facets(
    cube: &Cube,
    cam: &Camera,
    anim: Option<&Anim>,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
) -> Vec<Facet> {
    let n = cube.n();
    let nf = n as f64;
    let m = n as i32 - 1;
    let half = (1.0 - st.gap) / nf;
    let scale = scale_for(st, cam);
    // Light fixed to the camera, from above and slightly left and in front.
    let light = {
        let l: [f64; 3] = [-0.30, 0.85, 0.55];
        let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / len, l[1] / len, l[2] / len]
    };
    let mut out = Vec::with_capacity(6 * n as usize * n as usize * 2);

    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let inner = |v: u8| v != 0 && v != n - 1;
                if inner(i) && inner(j) && inner(k) {
                    continue;
                }
                let cell = V3::new(2 * i as i32 - m, 2 * j as i32 - m, 2 * k as i32 - m);
                let center = [cell.x as f64 / nf, cell.y as f64 / nf, cell.z as f64 / nf];
                let spin = anim.filter(|a| a.turn.covers(layer_index(n, a.turn.axis, cell)));

                for dir in Face::ALL {
                    // Inner faces are never drawn: the body box behind the
                    // cubies fills both the grooves and the cut a turn opens.
                    let (axis, positive) = dir.axis();
                    if layer_index(n, axis, cell) != if positive { n - 1 } else { 0 } {
                        continue;
                    }
                    let normal = dir.normal().as_f64();
                    let right = dir.right().as_f64();
                    let down = dir.down().as_f64();
                    let corner = |sr: f64, sd: f64, shrink: f64| {
                        let mut p = [0.0; 3];
                        for c in 0..3 {
                            p[c] = center[c]
                                + normal[c] * half
                                + (right[c] * sr + down[c] * sd) * half * shrink;
                        }
                        p
                    };
                    let turn = |p: [f64; 3]| match spin {
                        Some(a) => a.turn.axis.rotate_f64(p, a.angle),
                        None => p,
                    };

                    // The body is drawn a touch oversize so that the three faces
                    // of a cubie meet without a notch at the corner.
                    let world: Vec<[f64; 3]> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                        .iter()
                        .map(|&(sr, sd)| view(turn(corner(sr, sd, 1.03)), cam))
                        .collect();
                    let vnormal = view(turn(normal), cam);
                    let mid = centroid(&world);
                    let to_eye = [-mid[0], -mid[1], DIST - mid[2]];
                    let depth =
                        to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2];
                    if vnormal[0] * to_eye[0] + vnormal[1] * to_eye[1] + vnormal[2] * to_eye[2]
                        <= 0.0
                    {
                        continue;
                    }
                    // Half Lambert: a soft gradient that keeps white stickers white.
                    let lambert = 0.5
                        + 0.5
                            * (vnormal[0] * light[0]
                                + vnormal[1] * light[1]
                                + vnormal[2] * light[2]);

                    let quad = |ps: &[[f64; 3]]| -> [P2; 4] {
                        [
                            project(ps[0], scale, st.size),
                            project(ps[1], scale, st.size),
                            project(ps[2], scale, st.size),
                            project(ps[3], scale, st.size),
                        ]
                    };
                    let body = quad(&world);
                    out.push(Facet {
                        pts: body,
                        color: pal.plastic.shade(1.0 + st.shade * (1.6 * lambert - 0.6)),
                        paint: Paint::Plastic,
                        depth,
                        radius: st.round * 0.35 * edge_length(&body),
                        behind: false,
                        facelet: None,
                    });

                    let Some(facelet) = facelet_at(n, cell, dir.normal()) else {
                        continue;
                    };
                    let sticker: Vec<[f64; 3]> =
                        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                            .iter()
                            .map(|&(sr, sd)| view(turn(corner(sr, sd, st.sticker)), cam))
                            .collect();
                    let pts = quad(&sticker);
                    let own = marks
                        .fill
                        .get(&facelet)
                        .copied()
                        .unwrap_or_else(|| pal.face(cube.get(facelet.0, facelet.1, facelet.2)));
                    let color = own.shade(1.0 - st.shade * (1.0 - lambert));
                    let radius = st.round * edge_length(&pts);
                    out.push(Facet {
                        pts,
                        color,
                        paint: Paint::Sticker,
                        depth,
                        radius,
                        behind: false,
                        facelet: Some(facelet),
                    });
                    if let Some(ring) = marks.ring.get(&facelet) {
                        out.push(Facet {
                            pts,
                            color: *ring,
                            paint: Paint::Ring,
                            depth,
                            radius,
                            behind: false,
                            facelet: Some(facelet),
                        });
                    }
                }
            }
        }
    }
    out.extend(body_boxes(n, cam, anim, st, pal, light));
    // The body goes down first, then the cubies from the back forwards.
    out.sort_by(|a, b| b.behind.cmp(&a.behind).then(b.depth.total_cmp(&a.depth)));
    out
}

/// The rigid groups a turn splits the cube into: the layers that move, and the
/// one or two blocks that stay put.
fn groups(n: u8, anim: Option<&Anim>) -> Vec<(Axis, u8, u8, bool)> {
    match anim {
        None => vec![(Axis::Y, 0, n - 1, false)],
        Some(a) => {
            let axis = a.turn.axis;
            let mut out = vec![(axis, a.turn.lo, a.turn.hi, true)];
            if a.turn.lo > 0 {
                out.push((axis, 0, a.turn.lo - 1, false));
            }
            if a.turn.hi < n - 1 {
                out.push((axis, a.turn.hi + 1, n - 1, false));
            }
            out
        }
    }
}

/// A dark box filling each group: what shows through the grooves between the
/// cubies, and across the face a turn cuts open.
fn body_boxes(
    n: u8,
    cam: &Camera,
    anim: Option<&Anim>,
    st: &Style3d,
    pal: &Palette,
    light: [f64; 3],
) -> Vec<Facet> {
    let nf = n as f64;
    let half = (1.0 - st.gap) / nf;
    let scale = scale_for(st, cam);
    let outer = (nf - st.gap) / nf;
    let center = |i: u8| (2.0 * i as f64 - (nf - 1.0)) / nf;
    let mut out = Vec::new();

    for (axis, lo, hi, moving) in groups(n, anim) {
        let mut min = [-outer; 3];
        let mut max = [outer; 3];
        min[axis as usize] = center(lo) - half;
        max[axis as usize] = center(hi) + half;
        let span = |a: usize| if a == axis as usize { hi - lo + 1 } else { n };

        for dir in Face::ALL {
            let normal = dir.normal().as_f64();
            let right = dir.right().as_f64();
            let down = dir.down().as_f64();
            let pick = |v: [f64; 3], sr: f64, sd: f64| {
                let mut p = [0.0; 3];
                for c in 0..3 {
                    let along = normal[c] * normal[c];
                    let side = right[c] * sr + down[c] * sd;
                    p[c] = along * v[c] + (1.0 - along) * if side > 0.0 { max[c] } else { min[c] };
                }
                p
            };
            // The face sits at the box bound the normal points to.
            let plane: [f64; 3] = core::array::from_fn(|c| {
                if normal[c] > 0.0 {
                    max[c]
                } else if normal[c] < 0.0 {
                    min[c]
                } else {
                    0.0
                }
            });
            let turn = |p: [f64; 3]| match anim.filter(|_| moving) {
                Some(a) => a.turn.axis.rotate_f64(p, a.angle),
                None => p,
            };
            let world: Vec<[f64; 3]> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .iter()
                .map(|&(sr, sd)| view(turn(pick(plane, sr, sd)), cam))
                .collect();
            let vnormal = view(turn(normal), cam);
            let mid = centroid(&world);
            let to_eye = [-mid[0], -mid[1], DIST - mid[2]];
            let depth = to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2];
            let lambert =
                0.5 + 0.5 * (vnormal[0] * light[0] + vnormal[1] * light[1] + vnormal[2] * light[2]);
            let pts = [
                project(world[0], scale, st.size),
                project(world[1], scale, st.size),
                project(world[2], scale, st.size),
                project(world[3], scale, st.size),
            ];
            // Round the box corners like a cubie, so the silhouette matches.
            let cubies = (0..3)
                .filter(|c| normal[*c] == 0.0)
                .map(span)
                .min()
                .unwrap_or(n) as f64;
            out.push(Facet {
                pts,
                color: pal.plastic.shade(1.0 + st.shade * (1.6 * lambert - 0.75)),
                paint: Paint::Plastic,
                depth,
                radius: st.round * 0.35 * edge_length(&pts) / cubies,
                behind: true,
                facelet: None,
            });
        }
    }
    out
}

fn edge_length(pts: &[P2; 4]) -> f64 {
    let mut total = 0.0;
    for i in 0..4 {
        let a = pts[i];
        let b = pts[(i + 1) % 4];
        total += ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    }
    total / 4.0
}

/// SVG markup for the facets, without the enclosing `<svg>` element.
pub fn markup(facets: &[Facet], st: &Style3d, pal: &Palette) -> String {
    let mut s = format!(
        "<g stroke-linejoin=\"round\" stroke=\"{}\" stroke-width=\"{}\">",
        pal.edge.hex(),
        num(st.stroke)
    );
    for f in facets {
        let d = rounded_path(&f.pts, f.radius);
        match f.paint {
            Paint::Plastic => {
                s.push_str(&format!("<path d=\"{d}\" fill=\"{}\"/>", f.color.hex()));
            }
            Paint::Sticker => {
                let tag = f
                    .facelet
                    .map(|(face, r, c)| format!(" data-facelet=\"{}-{r}-{c}\"", face.letter()))
                    .unwrap_or_default();
                s.push_str(&format!(
                    "<path d=\"{d}\" fill=\"{}\"{tag}/>",
                    f.color.hex()
                ));
            }
            Paint::Ring => {
                s.push_str(&format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    f.color.hex(),
                    num(st.stroke * 3.6)
                ));
            }
        }
    }
    s.push_str("</g>");
    s
}

/// Convenience: markup for a cube state, ready to drop into an `<svg>`.
pub fn inner(
    cube: &Cube,
    cam: &Camera,
    anim: Option<&Anim>,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
) -> String {
    markup(&facets(cube, cam, anim, st, pal, marks), st, pal)
}

/// A standalone SVG document.
pub fn document(
    cube: &Cube,
    cam: &Camera,
    anim: Option<&Anim>,
    st: &Style3d,
    pal: &Palette,
    marks: &Marks,
    background: Option<&str>,
) -> String {
    let mut s = open(st.size, st.size, background);
    s.push_str(&inner(cube, cam, anim, st, pal, marks));
    s.push_str("</svg>");
    s
}
