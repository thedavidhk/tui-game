//! Mouse helpers: map screen cells to local grids and common brush / fill shapes.

use crate::input::MouseCell;
use crate::math::chebyshev_dist;
use crate::rect::Rect;

/// If `cell` lies inside `rect`, return coordinates relative to `rect`'s top-left.
#[must_use]
pub fn cell_local_in_rect(cell: MouseCell, rect: Rect) -> Option<(i32, i32)> {
    if !rect.contains(cell.x, cell.y) {
        return None;
    }
    Some((
        i32::from(cell.x) - i32::from(rect.x),
        i32::from(cell.y) - i32::from(rect.y),
    ))
}

/// Visible map extent when the map is drawn at the top-left of the framebuffer.
#[must_use]
pub fn map_view_rect(level_w: u16, level_h: u16, fb_w: u16, fb_h: u16) -> Rect {
    let w = level_w.min(fb_w);
    let h = level_h.min(fb_h);
    Rect::new(0, 0, w, h)
}

/// True if `(tx, ty)` lies in the square **Chebyshev** brush around `(cx, cy)`.
#[must_use]
pub fn cell_in_brush(tx: i32, ty: i32, cx: i32, cy: i32, radius: u8) -> bool {
    chebyshev_dist(tx - cx, ty - cy) <= i32::from(radius)
}

/// True if `(tx, ty)` lies in the inclusive axis-aligned rectangle between the two corners.
#[must_use]
pub fn cell_in_axis_rect(tx: i32, ty: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let xa = x0.min(x1);
    let xb = x0.max(x1);
    let ya = y0.min(y1);
    let yb = y0.max(y1);
    tx >= xa && tx <= xb && ty >= ya && ty <= yb
}

/// Visit every cell in a square **Chebyshev** brush (L∞ radius) centered at `(cx, cy)`.
pub fn for_each_in_brush(cx: i32, cy: i32, radius: u8, mut visit: impl FnMut(i32, i32)) {
    let r = i32::from(radius);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx.abs().max(dy.abs()) <= r {
                visit(cx + dx, cy + dy);
            }
        }
    }
}

/// Visit every cell in the axis-aligned rectangle between the two corners (inclusive).
pub fn for_each_in_rect(x0: i32, y0: i32, x1: i32, y1: i32, mut visit: impl FnMut(i32, i32)) {
    let (xa, xb) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (ya, yb) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    for y in ya..=yb {
        for x in xa..=xb {
            visit(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_radius_zero_is_center_only() {
        let mut v = Vec::new();
        for_each_in_brush(5, 5, 0, |x, y| v.push((x, y)));
        assert_eq!(v, vec![(5, 5)]);
    }

    #[test]
    fn brush_radius_one_is_three_by_three() {
        let mut v = Vec::new();
        for_each_in_brush(1, 1, 1, |x, y| v.push((x, y)));
        v.sort();
        assert_eq!(v.len(), 9);
    }

    #[test]
    fn cell_in_brush_matches_for_each() {
        for_each_in_brush(3, -2, 2, |x, y| {
            assert!(cell_in_brush(x, y, 3, -2, 2));
        });
        assert!(!cell_in_brush(10, 10, 3, -2, 2));
    }

    #[test]
    fn cell_in_axis_rect_corners() {
        assert!(cell_in_axis_rect(2, 2, 0, 0, 2, 2));
        assert!(!cell_in_axis_rect(3, 2, 0, 0, 2, 2));
    }
}
