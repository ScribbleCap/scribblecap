//! Arrow outline tests (roadmap M1-2, findings §5.3).
//!
//! Golden file: `tests/golden/arrows.txt`. After an intended change to the arrow
//! shape, regenerate it with `SCRIBBLECAP_UPDATE_GOLDEN=1 cargo test -p
//! scribblecap-core --test arrow_geometry`, look at the preview
//! (`cargo run -p scribblecap-core --example preview`) and commit both.

use scribblecap_core::geometry::arrow_outline;
use scribblecap_core::{Path, Point};

/// The five tool sizes (relative 0.2…1.0 × 20 px).
const SIZES: [f64; 5] = [4.0, 8.0, 12.0, 16.0, 20.0];
const LENGTHS: [(&str, f64); 3] = [("short", 40.0), ("normal", 120.0), ("long", 600.0)];

fn golden_cases() -> Vec<(String, Path)> {
    // A 3-4-5 direction keeps the rotated coordinates exact enough to read.
    let start = Point::new(100.0, 50.0);
    let towards = |len: f64| Point::new(start.x + 0.6 * len, start.y + 0.8 * len);
    let mut cases = Vec::new();
    for (name, len) in LENGTHS {
        for size in SIZES {
            let path = arrow_outline(start, towards(len), size, false);
            cases.push((format!("{name}-{size}"), path));
        }
    }
    for size in SIZES {
        let path = arrow_outline(start, towards(120.0), size, true);
        cases.push((format!("both-heads-{size}"), path));
    }
    cases
}

#[test]
fn matches_golden_outlines() {
    let actual: String = golden_cases()
        .iter()
        .map(|(name, path)| format!("{name}\t{}\n", path.to_svg()))
        .collect();
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/arrows.txt");
    if std::env::var_os("SCRIBBLECAP_UPDATE_GOLDEN").is_some() {
        std::fs::write(file, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(file).unwrap().replace("\r\n", "\n");
    for (a, e) in actual.lines().zip(expected.lines()) {
        assert_eq!(
            a, e,
            "arrow outline changed (see the note at the top of this file)"
        );
    }
    assert_eq!(actual.lines().count(), expected.lines().count());
}

const EPS: f64 = 1e-9;

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS
}

/// Axis-aligned arrow from the origin along +x.
fn horizontal(len: f64, size: f64, both: bool) -> Vec<Point> {
    arrow_outline(Point::new(0.0, 0.0), Point::new(len, 0.0), size, both)
        .points()
        .collect()
}

#[test]
fn tip_is_exactly_at_end() {
    for (_, len) in LENGTHS {
        for size in SIZES {
            for both in [false, true] {
                let pts = horizontal(len, size, both);
                assert!(pts.iter().any(|p| close(*p, Point::new(len, 0.0))));
                assert!(
                    pts.iter().all(|p| p.x <= len + EPS),
                    "nothing beyond the tip"
                );
                if both {
                    assert!(pts.iter().any(|p| close(*p, Point::new(0.0, 0.0))));
                    assert!(
                        pts.iter().all(|p| p.x >= -EPS),
                        "nothing beyond the start tip"
                    );
                }
            }
        }
    }
}

#[test]
fn is_mirror_symmetric() {
    for (_, len) in LENGTHS {
        for size in SIZES {
            for both in [false, true] {
                let pts = horizontal(len, size, both);
                for p in &pts {
                    let mirrored = Point::new(p.x, -p.y);
                    assert!(
                        pts.iter().any(|q| close(*q, mirrored)),
                        "{p:?} has no mirror image (len {len}, size {size})"
                    );
                }
            }
        }
    }
}

#[test]
fn head_is_the_widest_part_and_grows_with_size() {
    let mut last = 0.0;
    for size in SIZES {
        let pts = horizontal(600.0, size, false);
        let half_width = pts.iter().map(|p| p.y.abs()).fold(0.0, f64::max);
        let widest = pts.iter().find(|p| p.y.abs() == half_width).unwrap();
        assert!(
            widest.x > 600.0 * 0.8,
            "widest point {widest:?} is not in the head"
        );
        assert!(
            half_width > last,
            "size {size} is not wider than the previous size"
        );
        last = half_width;
    }
}

#[test]
fn short_arrows_shrink() {
    // Below 5 × size the arrow behaves as if drawn with size len / 5.
    let (len, size) = (40.0, 20.0);
    assert_eq!(
        horizontal(len, size, false),
        horizontal(len, len / 5.0, false)
    );
}

#[test]
fn follows_the_direction() {
    // Rotating start→end by 90° rotates the outline by 90°.
    let start = Point::new(30.0, 40.0);
    let right = arrow_outline(start, Point::new(130.0, 40.0), 12.0, false);
    let down = arrow_outline(start, Point::new(30.0, 140.0), 12.0, false);
    let rotate = |p: Point| Point::new(start.x - (p.y - start.y), start.y + (p.x - start.x));
    for (r, d) in right.points().zip(down.points()) {
        assert!(close(rotate(r), d), "{r:?} rotated is not {d:?}");
    }
}

#[test]
fn degenerate_arrows_are_empty() {
    let p = Point::new(5.0, 5.0);
    assert!(arrow_outline(p, p, 12.0, false).is_empty());
    assert!(arrow_outline(p, Point::new(50.0, 5.0), 0.0, false).is_empty());
    assert!(arrow_outline(p, Point::new(f64::NAN, 5.0), 12.0, false).is_empty());
}
