//! Cube geometry: faces, the lattice facelets live on, and layer rotations.
//!
//! Everything is parameterised by the cube size `n`, so the same code draws and
//! turns a 2x2, a 3x3 or a 7x7.

/// A point or direction in cube space.
///
/// Facelet centers live on the doubled, centered lattice: cell index `i` of an
/// `n`-cube sits at `2*i - (n - 1)`, which keeps every quarter turn integral.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct V3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl V3 {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn dot(self, o: V3) -> i32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub const fn scale(self, k: i32) -> Self {
        Self::new(self.x * k, self.y * k, self.z * k)
    }

    pub const fn cross(self, o: V3) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub const fn add(self, o: V3) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    pub fn as_f64(self) -> [f64; 3] {
        [self.x as f64, self.y as f64, self.z as f64]
    }
}

/// The three cube axes. Rotations are clockwise seen from the positive end.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub const fn component(self, v: V3) -> i32 {
        match self {
            Axis::X => v.x,
            Axis::Y => v.y,
            Axis::Z => v.z,
        }
    }

    /// The two axes spanning the plane of rotation, in the order that makes
    /// `(a, b) -> (a*cos + b*sin, -a*sin + b*cos)` a clockwise turn seen from
    /// the positive end of `self`.
    pub const fn plane(self) -> (Axis, Axis) {
        match self {
            Axis::X => (Axis::Y, Axis::Z),
            Axis::Y => (Axis::Z, Axis::X),
            Axis::Z => (Axis::X, Axis::Y),
        }
    }

    /// `q` quarter turns clockwise seen from the positive end of the axis.
    pub fn rotate(self, v: V3, q: i32) -> V3 {
        let mut out = v;
        for _ in 0..q.rem_euclid(4) {
            out = match self {
                Axis::X => V3::new(out.x, out.z, -out.y),
                Axis::Y => V3::new(-out.z, out.y, out.x),
                Axis::Z => V3::new(out.y, -out.x, out.z),
            };
        }
        out
    }

    /// The same rotation by a free angle, on floating point coordinates.
    pub fn rotate_f64(self, v: [f64; 3], angle: f64) -> [f64; 3] {
        let (c, s) = (angle.cos(), angle.sin());
        let (a, b) = self.plane();
        let (va, vb) = (a.pick(v), b.pick(v));
        let mut out = v;
        a.put(&mut out, va * c + vb * s);
        b.put(&mut out, -va * s + vb * c);
        out
    }

    fn pick(self, v: [f64; 3]) -> f64 {
        v[self as usize]
    }

    fn put(self, v: &mut [f64; 3], value: f64) {
        v[self as usize] = value;
    }
}

/// The six faces, in the order used for facelet numbering (U, L, F, R, B, D),
/// which is the order of the Singmaster indices 1..48 of a 3x3.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Face {
    U = 0,
    L = 1,
    F = 2,
    R = 3,
    B = 4,
    D = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::U, Face::L, Face::F, Face::R, Face::B, Face::D];

    pub const fn letter(self) -> char {
        match self {
            Face::U => 'U',
            Face::L => 'L',
            Face::F => 'F',
            Face::R => 'R',
            Face::B => 'B',
            Face::D => 'D',
        }
    }

    pub fn from_letter(c: char) -> Option<Face> {
        match c.to_ascii_uppercase() {
            'U' => Some(Face::U),
            'L' => Some(Face::L),
            'F' => Some(Face::F),
            'R' => Some(Face::R),
            'B' => Some(Face::B),
            'D' => Some(Face::D),
            _ => None,
        }
    }

    /// Outward normal.
    pub const fn normal(self) -> V3 {
        match self {
            Face::U => V3::new(0, 1, 0),
            Face::L => V3::new(-1, 0, 0),
            Face::F => V3::new(0, 0, 1),
            Face::R => V3::new(1, 0, 0),
            Face::B => V3::new(0, 0, -1),
            Face::D => V3::new(0, -1, 0),
        }
    }

    /// Direction of increasing column, laid out as the standard unfolded net
    /// (U on top, the L F R B strip around the middle, D underneath).
    pub const fn right(self) -> V3 {
        match self {
            Face::U => V3::new(1, 0, 0),
            Face::L => V3::new(0, 0, 1),
            Face::F => V3::new(1, 0, 0),
            Face::R => V3::new(0, 0, -1),
            Face::B => V3::new(-1, 0, 0),
            Face::D => V3::new(1, 0, 0),
        }
    }

    /// Direction of increasing row.
    pub const fn down(self) -> V3 {
        match self {
            Face::U => V3::new(0, 0, 1),
            Face::L => V3::new(0, -1, 0),
            Face::F => V3::new(0, -1, 0),
            Face::R => V3::new(0, -1, 0),
            Face::B => V3::new(0, -1, 0),
            Face::D => V3::new(0, 0, -1),
        }
    }

    /// Axis of the face normal, and whether the normal points along `+axis`.
    pub const fn axis(self) -> (Axis, bool) {
        match self {
            Face::U => (Axis::Y, true),
            Face::D => (Axis::Y, false),
            Face::R => (Axis::X, true),
            Face::L => (Axis::X, false),
            Face::F => (Axis::Z, true),
            Face::B => (Axis::Z, false),
        }
    }

    pub fn from_normal(n: V3) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.normal() == n)
    }
}

/// The face drawn above this one in the net, which is also the face that
/// should be uppermost when this one is held square to a camera.
///
/// The net puts a fixed neighbour above every face -- U above the side
/// faces, B above U, F above D -- so naming it is all anyone needs to hold a
/// cube the way the rest of the program reads it.
pub fn above(f: Face) -> Face {
    let up = f.down().scale(-1);
    Face::ALL
        .into_iter()
        .find(|g| g.normal() == up)
        .expect("every face has one above it")
}

/// Which face's color ends up at each position when the cube is turned to
/// hold `face` square to the viewer with [`above`] uppermost.
///
/// Indexed by position, in `Face` order. This is the cube as it has to be
/// held for one shot of a capture: the camera stays where it is and the cube
/// turns, so a picture of this is a picture of what to do.
pub fn facing(face: Face) -> [Face; 6] {
    let front = face.normal();
    let up = above(face).normal();
    let right = up.cross(front);
    let mut at = [Face::U; 6];
    for color in Face::ALL {
        let n = color.normal();
        let turned = V3::new(n.dot(right), n.dot(up), n.dot(front));
        let position = Face::from_normal(turned).expect("a turn maps faces to faces");
        at[position as usize] = color;
    }
    at
}

/// Index of a facelet inside a flat `6 * n * n` sticker array.
pub const fn facelet_index(n: u8, f: Face, row: u8, col: u8) -> usize {
    let n = n as usize;
    (f as usize) * n * n + (row as usize) * n + col as usize
}

/// Lattice position of the center of a facelet.
pub fn facelet_position(n: u8, f: Face, row: u8, col: u8) -> V3 {
    let m = n as i32 - 1;
    f.normal()
        .scale(m)
        .add(f.right().scale(2 * col as i32 - m))
        .add(f.down().scale(2 * row as i32 - m))
}

/// Inverse of [`facelet_position`]: which facelet sits at `p` facing `normal`.
pub fn facelet_at(n: u8, p: V3, normal: V3) -> Option<(Face, u8, u8)> {
    let m = n as i32 - 1;
    let f = Face::from_normal(normal)?;
    if p.dot(normal) != m {
        return None;
    }
    let col = p.dot(f.right()) + m;
    let row = p.dot(f.down()) + m;
    if col < 0 || row < 0 || col > 2 * m || row > 2 * m || col % 2 != 0 || row % 2 != 0 {
        return None;
    }
    Some((f, (row / 2) as u8, (col / 2) as u8))
}

/// Layer index (0 at the negative end of the axis) of a lattice point.
pub const fn layer_index(n: u8, axis: Axis, p: V3) -> u8 {
    ((axis.component(p) + n as i32 - 1) / 2) as u8
}

/// Singmaster-style number of a facelet: 1..=48 on a 3x3, counting faces in the
/// order U, L, F, R, B, D and skipping the fixed center of each face.
pub fn facelet_label(n: u8, f: Face, row: u8, col: u8) -> Option<u32> {
    let center = (n % 2 == 1).then_some(n / 2);
    if center == Some(row) && center == Some(col) {
        return None;
    }
    let per_face = n as u32 * n as u32 - u32::from(center.is_some());
    let mut within = row as u32 * n as u32 + col as u32;
    if let Some(c) = center {
        let center_at = c as u32 * n as u32 + c as u32;
        if within > center_at {
            within -= 1;
        }
    }
    Some(f as u32 * per_face + within + 1)
}

/// Inverse of [`facelet_label`].
pub fn facelet_from_label(n: u8, label: u32) -> Option<(Face, u8, u8)> {
    Face::ALL.into_iter().find_map(|f| {
        (0..n).find_map(|row| {
            (0..n).find_map(|col| {
                (facelet_label(n, f, row, col) == Some(label)).then_some((f, row, col))
            })
        })
    })
}
