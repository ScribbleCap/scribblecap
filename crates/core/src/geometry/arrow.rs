//! Arrow outline (findings §5.3).

use crate::geom::{Point, is_positive};
use crate::path::Path;

/// Control-point distance that makes one cubic approximate a half circle.
const HALF_CIRCLE: f64 = 4.0 / 3.0;

/// Filled outline of an arrow from `start` to its tip at `end`, `size` pixels wide
/// (an [`Arrow`](crate::element::Arrow)'s `width`).
///
/// One closed subpath: a shaft that widens from a round tail to the head, and a
/// triangular head with swept-back barbs. Arrows shorter than `5 × size` shrink their
/// size so the head never swallows the arrow. With `head_at_start` both ends get a
/// head and a straight shaft. Returns an empty path when `start == end`.
///
/// Uses only `+ − × ÷ √`, which IEEE 754 rounds identically on every platform, so the
/// output is bit-for-bit reproducible.
pub fn arrow_outline(start: Point, end: Point, size: f64, head_at_start: bool) -> Path {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let len = (dx * dx + dy * dy).sqrt();
    if !is_positive(len) || !is_positive(size) {
        return Path::new();
    }

    let s = if len < 5.0 * size { len / 5.0 } else { size };
    let h = (s - 1.5) * 0.7 + 6.0;
    let head = (2.0 * h).max(6.0).min(0.4 * len);
    let tail = (0.08 * s).max(1.5);
    let shaft = (0.5 * h).min(0.25 * head).max(tail);
    let barb_y = 0.55 * head;

    // Built along +x from the origin, then rotated onto start→end.
    let (ux, uy) = (dx / len, dy / len);
    let place = |x: f64, y: f64| Point::new(start.x + x * ux - y * uy, start.y + x * uy + y * ux);

    let m1 = (tail + shaft) / 2.0;
    let mut path = Path::new();
    if head_at_start {
        // Straight shaft, as wide as the middle of a single-headed arrow.
        path.move_to(place(0.0, 0.0))
            .line_to(place(1.1 * head, -barb_y))
            .line_to(place(head, -m1))
            .line_to(place(len - head, -m1))
            .line_to(place(len - 1.1 * head, -barb_y))
            .line_to(place(len, 0.0))
            .line_to(place(len - 1.1 * head, barb_y))
            .line_to(place(len - head, m1))
            .line_to(place(head, m1))
            .line_to(place(1.1 * head, barb_y))
            .close();
        return path;
    }

    let base = len - head;
    let barb_x = len - 1.1 * head;
    let m2 = (tail + m1) / 2.0;
    let mid = (len / 2.0 + base) / 2.0;
    path.move_to(place(0.0, -tail))
        .cubic_to(place(base / 2.0, -m2), place(mid, -m1), place(base, -shaft))
        .line_to(place(barb_x, -barb_y))
        .line_to(place(len, 0.0))
        .line_to(place(barb_x, barb_y))
        .line_to(place(base, shaft))
        .cubic_to(place(mid, m1), place(base / 2.0, m2), place(0.0, tail))
        // Round tail: a half circle of radius `tail` behind the start point.
        .cubic_to(
            place(-HALF_CIRCLE * tail, tail),
            place(-HALF_CIRCLE * tail, -tail),
            place(0.0, -tail),
        )
        .close();
    path
}
