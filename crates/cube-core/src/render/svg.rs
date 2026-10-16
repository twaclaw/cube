//! Small SVG writing helpers: number formatting and rounded polygons.

/// A point in SVG user units.
pub type P2 = [f64; 2];

/// Compact number: at most two decimals, no trailing zeros.
pub fn num(v: f64) -> String {
    let mut s = format!("{v:.2}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" { "0".to_string() } else { s }
}

fn dist(a: P2, b: P2) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn toward(from: P2, to: P2, d: f64) -> P2 {
    let len = dist(from, to).max(1e-9);
    let t = (d / len).clamp(0.0, 0.5);
    [
        from[0] + (to[0] - from[0]) * t,
        from[1] + (to[1] - from[1]) * t,
    ]
}

/// Closed path through `pts` with the corners rounded by `radius` user units.
/// Each corner is clamped so that neighboring arcs never overlap.
pub fn rounded_path(pts: &[P2], radius: f64) -> String {
    let k = pts.len();
    if k < 3 {
        return String::new();
    }
    if radius <= 0.0 {
        let mut d = String::with_capacity(k * 12);
        for (i, p) in pts.iter().enumerate() {
            d.push(if i == 0 { 'M' } else { 'L' });
            d.push_str(&num(p[0]));
            d.push(' ');
            d.push_str(&num(p[1]));
        }
        d.push('Z');
        return d;
    }
    let mut enter = Vec::with_capacity(k);
    let mut leave = Vec::with_capacity(k);
    for i in 0..k {
        let prev = pts[(i + k - 1) % k];
        let next = pts[(i + 1) % k];
        let r = radius
            .min(dist(pts[i], prev) * 0.5)
            .min(dist(pts[i], next) * 0.5);
        enter.push(toward(pts[i], prev, r));
        leave.push(toward(pts[i], next, r));
    }
    let mut d = format!("M{} {}", num(leave[0][0]), num(leave[0][1]));
    for i in 1..=k {
        let j = i % k;
        d.push_str(&format!(
            "L{} {}Q{} {} {} {}",
            num(enter[j][0]),
            num(enter[j][1]),
            num(pts[j][0]),
            num(pts[j][1]),
            num(leave[j][0]),
            num(leave[j][1])
        ));
    }
    d.push('Z');
    d
}

/// Opening tag of a standalone SVG document.
pub fn open(width: f64, height: f64, background: Option<&str>) -> String {
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">",
        num(width),
        num(height),
        num(width),
        num(height)
    );
    if let Some(bg) = background {
        s.push_str(&format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"{bg}\"/>",
            num(width),
            num(height)
        ));
    }
    s
}
