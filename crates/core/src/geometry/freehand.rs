//! Freehand strokes (findings §5.4): a point filter applied while drawing, and a
//! curve fit that turns the filtered points into smooth cubic Béziers.
//!
//! The fit is Schneider's algorithm ("An Algorithm for Automatically Fitting Digitized
//! Curves", Graphics Gems, 1990) with corner detection. Like the arrow outline it uses
//! only `+ − × ÷ √`, so results are identical on every platform.

use crate::geom::Point;
use crate::path::Path;

/// Fixed-point scale of the point filter.
const FIXED: i64 = 9;
/// Filtered points closer than this (Manhattan distance, pixels) to the previous
/// output are dropped.
const MIN_STEP: i64 = 2;

/// Smooths raw pointer input and drops near-duplicates, one point at a time.
///
/// Each new input is approached in three sub-steps; every sub-step moves the smoothed
/// position a third of the way to its target, so jitter is damped. Output points lie
/// on the integer pixel grid. Call [`finish`](Self::finish) at pointer-up so the
/// stroke ends where the pointer did.
#[derive(Clone, Debug, Default)]
pub struct PointFilter {
    last_input: Option<(i64, i64)>,
    smoothed: (i64, i64),
    last_output: Option<(i64, i64)>,
}

impl PointFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one input point; appends any resulting output points to `out`.
    pub fn push(&mut self, p: Point, out: &mut Vec<Point>) {
        if !p.is_finite() {
            return;
        }
        let input = (to_fixed(p.x), to_fixed(p.y));
        let Some(last) = self.last_input else {
            self.last_input = Some(input);
            self.smoothed = input;
            self.emit(input, out);
            return;
        };
        for i in 1..=3 {
            let target = (
                last.0 + (input.0 - last.0) * i / 3,
                last.1 + (input.1 - last.1) * i / 3,
            );
            self.smoothed.0 += (target.0 - self.smoothed.0) / 3;
            self.smoothed.1 += (target.1 - self.smoothed.1) / 3;
            self.emit(self.smoothed, out);
        }
        self.last_input = Some(input);
    }

    /// End the stroke: emits the last input point if the output hasn't reached it.
    pub fn finish(&mut self, out: &mut Vec<Point>) {
        if let Some(last) = self.last_input {
            let p = pixel(last);
            if self.last_output != Some(p) {
                self.last_output = Some(p);
                out.push(Point::new(p.0 as f64, p.1 as f64));
            }
        }
    }

    fn emit(&mut self, v: (i64, i64), out: &mut Vec<Point>) {
        let p = pixel(v);
        if let Some(q) = self.last_output
            && (p.0 - q.0).abs() + (p.1 - q.1).abs() < MIN_STEP
        {
            return;
        }
        self.last_output = Some(p);
        out.push(Point::new(p.0 as f64, p.1 as f64));
    }
}

fn to_fixed(v: f64) -> i64 {
    (v * FIXED as f64).round() as i64
}

/// Fixed point to the nearest whole pixel.
fn pixel(v: (i64, i64)) -> (i64, i64) {
    let r = |c: i64| (c + FIXED / 2).div_euclid(FIXED);
    (r(v.0), r(v.1))
}

/// Run a whole stroke through [`PointFilter`].
pub fn filter_points(points: &[Point]) -> Vec<Point> {
    let mut filter = PointFilter::new();
    let mut out = Vec::new();
    for &p in points {
        filter.push(p, &mut out);
    }
    filter.finish(&mut out);
    out
}

/// Smooth centre line of a freehand stroke drawn `width` pixels wide (stroke it with
/// round caps and joins). `points` are the filtered points stored in the document.
///
/// Fitting tolerances scale with the width: corners are points where the stroke turns
/// by 90° or more over `width / 2` pixels on each side; elsewhere the curve stays
/// within `√(width / 5)` pixels of every point. A single point gives a zero-length
/// line (a dot once stroked).
pub fn freehand_path(points: &[Point], width: f64) -> Path {
    let mut pts: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points.iter().filter(|p| p.is_finite()) {
        if pts.last() != Some(&p) {
            pts.push(p);
        }
    }
    let mut path = Path::new();
    let Some(&first) = pts.first() else {
        return path;
    };
    path.move_to(first);
    if pts.len() == 1 {
        path.line_to(first);
        return path;
    }

    let width = if width.is_finite() && width > 0.0 {
        width
    } else {
        1.0
    };
    let corner_len = 0.5 * width;
    let max_error = (width / 5.0).sqrt();
    let corners = find_corners(&pts, corner_len);
    for pair in corners.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let t1 = end_tangent(&pts[a..=b], corner_len);
        let t2 = end_tangent_rev(&pts[a..=b], corner_len);
        fit_cubic(&pts[a..=b], t1, t2, max_error * max_error, &mut path);
    }
    path
}

/// Indices that split the stroke: both ends plus every corner.
fn find_corners(pts: &[Point], len: f64) -> Vec<usize> {
    let last = pts.len() - 1;
    // Cosine of the turn at each point; ≤ 0 means a turn of 90° or more.
    let turn = |i: usize| {
        let back = walk(pts, i, len, false);
        let fwd = walk(pts, i, len, true);
        let vin = sub(pts[i], back);
        let vout = sub(fwd, pts[i]);
        let n = (dot(vin, vin) * dot(vout, vout)).sqrt();
        if n == 0.0 { 1.0 } else { dot(vin, vout) / n }
    };
    let mut corners = vec![0];
    let mut i = 1;
    while i < last {
        if turn(i) <= 0.0 {
            // Keep only the sharpest point of a run of candidates.
            let mut best = (i, turn(i));
            let mut j = i + 1;
            while j < last && turn(j) <= 0.0 {
                let c = turn(j);
                if c < best.1 {
                    best = (j, c);
                }
                j += 1;
            }
            corners.push(best.0);
            i = j;
        } else {
            i += 1;
        }
    }
    corners.push(last);
    corners
}

/// The first point at least `len` along the polyline from `pts[i]` (or the end).
fn walk(pts: &[Point], i: usize, len: f64, forward: bool) -> Point {
    let mut dist = 0.0;
    let mut j = i;
    loop {
        let next = if forward {
            if j + 1 >= pts.len() {
                return pts[j];
            }
            j + 1
        } else {
            if j == 0 {
                return pts[0];
            }
            j - 1
        };
        dist += distance(pts[j], pts[next]);
        j = next;
        if dist >= len {
            return pts[j];
        }
    }
}

/// Unit tangent at the start, pointing into the curve.
fn end_tangent(pts: &[Point], len: f64) -> Point {
    normalize(sub(walk(pts, 0, len, true), pts[0]))
}

/// Unit tangent at the end, pointing back into the curve.
fn end_tangent_rev(pts: &[Point], len: f64) -> Point {
    let last = pts.len() - 1;
    normalize(sub(walk(pts, last, len, false), pts[last]))
}

const MAX_ITERATIONS: usize = 4;

/// Fit `pts` with cubics, appending them to `path` (which already ends at `pts[0]`).
/// `t1`/`t2` are unit tangents at the ends, pointing inwards; `err2` is the squared
/// error allowed.
fn fit_cubic(pts: &[Point], t1: Point, t2: Point, err2: f64, path: &mut Path) {
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    if pts.len() == 2 {
        let d = distance(first, last) / 3.0;
        path.cubic_to(add(first, scale(t1, d)), add(last, scale(t2, d)), last);
        return;
    }

    let mut u = chord_lengths(pts);
    let mut bez = generate_bezier(pts, &u, t1, t2);
    let (mut err, mut split) = max_error(pts, &bez, &u);
    if err < err2 {
        path.cubic_to(bez[1], bez[2], bez[3]);
        return;
    }
    // Close enough to be worth improving the parameters before splitting.
    if err < err2 * 4.0 {
        for _ in 0..MAX_ITERATIONS {
            for (ui, &p) in u.iter_mut().zip(pts) {
                *ui = newton_step(&bez, p, *ui);
            }
            bez = generate_bezier(pts, &u, t1, t2);
            (err, split) = max_error(pts, &bez, &u);
            if err < err2 {
                path.cubic_to(bez[1], bez[2], bez[3]);
                return;
            }
        }
    }

    let mut center = normalize(sub(pts[split - 1], pts[split + 1]));
    if center == Point::default() {
        center = normalize(sub(pts[split - 1], pts[split]));
    }
    fit_cubic(&pts[..=split], t1, center, err2, path);
    fit_cubic(&pts[split..], scale(center, -1.0), t2, err2, path);
}

/// Parameter of each point by cumulative chord length, 0…1.
fn chord_lengths(pts: &[Point]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    let mut total = 0.0;
    u.push(0.0);
    for w in pts.windows(2) {
        total += distance(w[0], w[1]);
        u.push(total);
    }
    let total = total.max(f64::MIN_POSITIVE);
    u.iter_mut().for_each(|v| *v /= total);
    u
}

/// Least-squares cubic through the ends with the given end tangents.
fn generate_bezier(pts: &[Point], u: &[f64], t1: Point, t2: Point) -> [Point; 4] {
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (&p, &t) in pts.iter().zip(u) {
        let [b0, b1, b2, b3] = bernstein(t);
        let a0 = scale(t1, b1);
        let a1 = scale(t2, b2);
        c00 += dot(a0, a0);
        c01 += dot(a0, a1);
        c11 += dot(a1, a1);
        let rest = sub(p, add(scale(first, b0 + b1), scale(last, b2 + b3)));
        x0 += dot(a0, rest);
        x1 += dot(a1, rest);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = distance(first, last);
    let eps = 1e-6 * seg;
    let (mut al, mut ar) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (0.0, 0.0)
    };
    // Degenerate or backwards handles: fall back to a third of the chord.
    if al < eps || ar < eps {
        al = seg / 3.0;
        ar = al;
    }
    [
        first,
        add(first, scale(t1, al)),
        add(last, scale(t2, ar)),
        last,
    ]
}

/// Largest squared distance between a point and its place on the curve, and where.
fn max_error(pts: &[Point], bez: &[Point; 4], u: &[f64]) -> (f64, usize) {
    let mut worst = (0.0, pts.len() / 2);
    for i in 1..pts.len() - 1 {
        let d = sub(eval(bez, u[i]), pts[i]);
        let d2 = dot(d, d);
        if d2 >= worst.0 {
            worst = (d2, i);
        }
    }
    worst
}

/// One Newton–Raphson step towards the parameter of the curve point nearest `p`.
fn newton_step(bez: &[Point; 4], p: Point, u: f64) -> f64 {
    let d1 = [
        scale(sub(bez[1], bez[0]), 3.0),
        scale(sub(bez[2], bez[1]), 3.0),
        scale(sub(bez[3], bez[2]), 3.0),
    ];
    let d2 = [scale(sub(d1[1], d1[0]), 2.0), scale(sub(d1[2], d1[1]), 2.0)];
    let q = sub(eval(bez, u), p);
    let q1 = quad(&d1, u);
    let q2 = add(scale(d2[0], 1.0 - u), scale(d2[1], u));
    let denom = dot(q1, q1) + dot(q, q2);
    if denom == 0.0 {
        u
    } else {
        (u - dot(q, q1) / denom).clamp(0.0, 1.0)
    }
}

fn bernstein(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * t * s * s, 3.0 * t * t * s, t * t * t]
}

fn eval(bez: &[Point; 4], t: f64) -> Point {
    let [b0, b1, b2, b3] = bernstein(t);
    add(
        add(scale(bez[0], b0), scale(bez[1], b1)),
        add(scale(bez[2], b2), scale(bez[3], b3)),
    )
}

fn quad(q: &[Point; 3], t: f64) -> Point {
    let s = 1.0 - t;
    add(
        add(scale(q[0], s * s), scale(q[1], 2.0 * s * t)),
        scale(q[2], t * t),
    )
}

fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn scale(a: Point, k: f64) -> Point {
    Point::new(a.x * k, a.y * k)
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn distance(a: Point, b: Point) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

fn normalize(a: Point) -> Point {
    let n = dot(a, a).sqrt();
    if n == 0.0 { a } else { scale(a, 1.0 / n) }
}
