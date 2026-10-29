//! Freehand filter and curve-fit tests (roadmap M1-3, findings §5.4).

use scribblecap_core::geometry::{PointFilter, filter_points, freehand_path};
use scribblecap_core::{Path, PathEl, Point};

/// Deterministic noise in [-amp, amp] (a fixed LCG, so tests are reproducible).
fn noise(seed: &mut u64, amp: f64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) * amp
}

fn wave(x: f64) -> f64 {
    200.0 + 40.0 * (x / 30.0).sin()
}

fn noisy_wave() -> Vec<Point> {
    let mut seed = 7;
    (0..=100)
        .map(|i| {
            let x = 10.0 + 3.0 * f64::from(i);
            Point::new(x + noise(&mut seed, 1.0), wave(x) + noise(&mut seed, 1.5))
        })
        .collect()
}

fn circle(cx: f64, cy: f64, r: f64, n: u32) -> Vec<Point> {
    (0..=n)
        .map(|i| {
            let a = f64::from(i) / f64::from(n) * std::f64::consts::TAU;
            Point::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

type Cubic = [Point; 4];

fn cubics(path: &Path) -> Vec<Cubic> {
    let mut at = Point::default();
    let mut out = Vec::new();
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => at = p,
            PathEl::CubicTo(c1, c2, p) => {
                out.push([at, c1, c2, p]);
                at = p;
            }
            PathEl::Close => {}
        }
    }
    out
}

fn eval(c: &Cubic, t: f64) -> Point {
    let s = 1.0 - t;
    let (b0, b1, b2, b3) = (s * s * s, 3.0 * t * s * s, 3.0 * t * t * s, t * t * t);
    Point::new(
        c[0].x * b0 + c[1].x * b1 + c[2].x * b2 + c[3].x * b3,
        c[0].y * b0 + c[1].y * b1 + c[2].y * b2 + c[3].y * b3,
    )
}

/// The curve sampled densely.
fn samples(path: &Path) -> Vec<Point> {
    cubics(path)
        .iter()
        .flat_map(|c| (0..=200).map(move |i| eval(c, f64::from(i) / 200.0)))
        .collect()
}

fn dist(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn dist_to(curve: &[Point], p: Point) -> f64 {
    curve
        .iter()
        .map(|&q| dist(p, q))
        .fold(f64::INFINITY, f64::min)
}

fn first_point(path: &Path) -> Point {
    match path.elements()[0] {
        PathEl::MoveTo(p) => p,
        el => panic!("path starts with {el:?}"),
    }
}

fn last_point(path: &Path) -> Point {
    *cubics(path).last().map(|c| &c[3]).unwrap()
}

// ---- Point filter ----

#[test]
fn filter_keeps_first_and_last_points() {
    let raw = [
        Point::new(10.4, 20.6),
        Point::new(30.0, 25.0),
        Point::new(80.2, 41.7),
    ];
    let out = filter_points(&raw);
    assert_eq!(out.first(), Some(&Point::new(10.0, 21.0)));
    assert_eq!(out.last(), Some(&Point::new(80.0, 42.0)));
}

#[test]
fn filter_outputs_whole_pixels_at_least_two_apart() {
    let mut seed = 1;
    let raw: Vec<Point> = (0..400)
        .map(|i| {
            let t = f64::from(i) * 0.7;
            Point::new(t + noise(&mut seed, 0.8), 0.5 * t + noise(&mut seed, 0.8))
        })
        .collect();
    let out = filter_points(&raw);
    assert!(out.len() > 50 && out.len() < raw.len());
    for p in &out {
        assert_eq!(
            (p.x.fract(), p.y.fract()),
            (0.0, 0.0),
            "{p:?} not on the grid"
        );
    }
    // Every step is at least 2 px (Manhattan), except possibly the final one added by
    // `finish`.
    for w in out[..out.len() - 1].windows(2) {
        assert!(
            (w[1].x - w[0].x).abs() + (w[1].y - w[0].y).abs() >= 2.0,
            "{w:?}"
        );
    }
}

#[test]
fn filter_smooths_jitter() {
    // A straight line with ±2 px jitter across it: the filter is light, but the mean
    // deviation from the line drops by at least a third.
    let mut seed = 3;
    let raw: Vec<Point> = (0..200)
        .map(|i| Point::new(f64::from(i) * 2.0, 100.0 + noise(&mut seed, 2.0)))
        .collect();
    let deviation =
        |pts: &[Point]| pts.iter().map(|p| (p.y - 100.0).abs()).sum::<f64>() / pts.len() as f64;
    let out = filter_points(&raw);
    let (before, after) = (deviation(&raw), deviation(&out[3..out.len() - 3]));
    assert!(after < before * 0.67, "deviation {before:.2} -> {after:.2}");
}

#[test]
fn filter_handles_tiny_and_bad_input() {
    assert!(filter_points(&[]).is_empty());
    assert_eq!(
        filter_points(&[Point::new(3.0, 4.0)]),
        vec![Point::new(3.0, 4.0)]
    );
    let same = [Point::new(5.0, 5.0); 10];
    assert_eq!(filter_points(&same), vec![Point::new(5.0, 5.0)]);
    let mut f = PointFilter::new();
    let mut out = Vec::new();
    f.push(Point::new(f64::NAN, 1.0), &mut out);
    f.push(Point::new(-7.6, -2.4), &mut out);
    f.finish(&mut out);
    assert_eq!(out, vec![Point::new(-8.0, -2.0)]);
}

// ---- Curve fit ----

const WIDTH: f64 = 6.0;

#[test]
fn fit_stays_close_to_every_input_point() {
    let pts = noisy_wave();
    let path = freehand_path(&pts, WIDTH);
    let curve = samples(&path);
    let tolerance = (WIDTH / 5.0).sqrt() + 0.05;
    for &p in &pts {
        let d = dist_to(&curve, p);
        assert!(d <= tolerance, "{p:?} is {d:.2} px from the curve");
    }
}

#[test]
fn fit_follows_the_underlying_shape() {
    let path = freehand_path(&noisy_wave(), WIDTH);
    for p in samples(&path) {
        let off = (p.y - wave(p.x)).abs();
        assert!(off < 3.0, "curve at {p:?} is {off:.2} px off the wave");
    }
}

#[test]
fn fit_is_smooth_and_compact() {
    // The app's pipeline: filter while drawing, fit on pointer-up.
    let pts = filter_points(&noisy_wave());
    let path = freehand_path(&pts, WIDTH);
    let cs = cubics(&path);
    assert!(
        cs.len() * 8 < pts.len(),
        "{} cubics for {} points",
        cs.len(),
        pts.len()
    );
    // No corners in a wave: tangents line up at every join (G1 continuity).
    for w in cs.windows(2) {
        let a = (w[0][3].x - w[0][2].x, w[0][3].y - w[0][2].y);
        let b = (w[1][1].x - w[1][0].x, w[1][1].y - w[1][0].y);
        let cross = a.0 * b.1 - a.1 * b.0;
        let dot = a.0 * b.0 + a.1 * b.1;
        let n = (a.0.hypot(a.1)) * (b.0.hypot(b.1));
        assert!(
            dot > 0.0 && (cross / n).abs() < 1e-9,
            "kink at {:?}",
            w[0][3]
        );
    }
}

#[test]
fn fit_starts_and_ends_on_the_stroke() {
    let pts = noisy_wave();
    let path = freehand_path(&pts, WIDTH);
    assert_eq!(first_point(&path), pts[0]);
    assert_eq!(last_point(&path), *pts.last().unwrap());
}

#[test]
fn sharp_turns_stay_sharp() {
    // An L: right along y = 0, then down at x = 100.
    let mut pts: Vec<Point> = (0..=50)
        .map(|i| Point::new(f64::from(i) * 2.0, 0.0))
        .collect();
    pts.extend((1..=50).map(|i| Point::new(100.0, f64::from(i) * 2.0)));
    let path = freehand_path(&pts, WIDTH);
    let cs = cubics(&path);
    let corner = Point::new(100.0, 0.0);
    let i = cs
        .iter()
        .position(|c| c[3] == corner)
        .expect("a segment ends exactly at the corner");
    let (into, out) = (cs[i], cs[i + 1]);
    assert!(
        (into[3].y - into[2].y).abs() < 1e-9 && into[3].x > into[2].x,
        "arrives along +x"
    );
    assert!(
        (out[1].x - out[0].x).abs() < 1e-9 && out[1].y > out[0].y,
        "leaves along +y"
    );
}

#[test]
fn gentle_turns_are_not_corners() {
    let pts = circle(150.0, 150.0, 80.0, 120);
    let path = freehand_path(&pts[..90], WIDTH);
    let cs = cubics(&path);
    for w in cs.windows(2) {
        let a = (w[0][3].x - w[0][2].x, w[0][3].y - w[0][2].y);
        let b = (w[1][1].x - w[1][0].x, w[1][1].y - w[1][0].y);
        assert!((a.0 * b.1 - a.1 * b.0).abs() / (a.0.hypot(a.1) * b.0.hypot(b.1)) < 1e-9);
    }
}

#[test]
fn closed_loop_closes() {
    let pts = circle(100.0, 100.0, 50.0, 90);
    let path = freehand_path(&pts, WIDTH);
    assert_eq!(first_point(&path), pts[0]);
    assert!(dist(last_point(&path), pts[0]) < 1e-9);
    let curve = samples(&path);
    for p in curve {
        let r = dist(p, Point::new(100.0, 100.0));
        assert!((r - 50.0).abs() < 1.5, "radius {r:.2}");
    }
}

#[test]
fn tiny_strokes() {
    assert!(freehand_path(&[], WIDTH).is_empty());

    let dot = freehand_path(&[Point::new(4.0, 5.0)], WIDTH);
    assert_eq!(
        dot.elements(),
        [
            PathEl::MoveTo(Point::new(4.0, 5.0)),
            PathEl::LineTo(Point::new(4.0, 5.0))
        ]
    );

    let dupes = [Point::new(1.0, 1.0); 3];
    assert_eq!(freehand_path(&dupes, WIDTH).elements().len(), 2);

    let two = freehand_path(&[Point::new(0.0, 0.0), Point::new(30.0, 40.0)], WIDTH);
    let c = cubics(&two);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0][3], Point::new(30.0, 40.0));
    // A straight segment: control points on the line.
    for p in &c[0] {
        assert!((p.x * 40.0 - p.y * 30.0).abs() < 1e-9);
    }
}

#[test]
fn filter_then_fit_a_scribble() {
    // The whole pipeline on raw, jittery input with a sharp zig-zag in it.
    let mut seed = 11;
    let mut raw = Vec::new();
    for i in 0..300 {
        let t = f64::from(i);
        let zig = if (i / 60) % 2 == 0 {
            t % 60.0
        } else {
            60.0 - t % 60.0
        };
        raw.push(Point::new(
            t + noise(&mut seed, 0.7),
            zig * 1.5 + noise(&mut seed, 0.7),
        ));
    }
    let pts = filter_points(&raw);
    let path = freehand_path(&pts, WIDTH);
    assert!(!cubics(&path).is_empty());
    let curve = samples(&path);
    let tolerance = (WIDTH / 5.0).sqrt() + 0.05;
    for &p in &pts {
        assert!(dist_to(&curve, p) <= tolerance);
    }
}
