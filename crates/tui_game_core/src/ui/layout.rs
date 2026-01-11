//! Shared rectangle math for bordered panels and split overlays.

use crate::rect::Rect;

/// Inner content area below a single-line title (matches [`super::draw_bordered_panel`]).
#[must_use]
pub fn panel_inner(panel: Rect) -> Rect {
    Rect::new(
        panel.x + 1,
        panel.y + 2,
        panel.w.saturating_sub(2),
        panel.h.saturating_sub(3),
    )
}

/// Two equal-ish columns for full-screen overlays (inventory, transfer).
#[must_use]
pub fn split_horizontal_outer(
    full_w: u16,
    full_h: u16,
    margin_x: u16,
    margin_top: u16,
    margin_bottom: u16,
    mid_gap: u16,
    min_col_w: u16,
) -> (Rect, Rect) {
    let inner_w = full_w.saturating_sub(margin_x * 2);
    let cols_w = inner_w.saturating_sub(mid_gap);
    let mut left_w = cols_w / 2;
    let mut right_w = cols_w.saturating_sub(left_w);
    if cols_w >= min_col_w.saturating_mul(2) {
        left_w = left_w.max(min_col_w);
        right_w = cols_w.saturating_sub(left_w);
    }
    let h = full_h.saturating_sub(margin_top + margin_bottom).max(8);
    let left = Rect::new(margin_x, margin_top, left_w, h);
    let right = Rect::new(
        margin_x.saturating_add(left_w).saturating_add(mid_gap),
        margin_top,
        right_w,
        h,
    );
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::split_horizontal_outer;

    #[test]
    fn split_horizontal_outer_uses_available_width() {
        let (left, right) = split_horizontal_outer(80, 30, 2, 3, 3, 2, 18);
        assert_eq!(left.w, 37);
        assert_eq!(right.w, 37);
        assert_eq!(left.x, 2);
        assert_eq!(right.x, 41);
    }
}
