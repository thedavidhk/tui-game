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
    let each = inner_w
        .saturating_sub(mid_gap)
        / 2
        .max(min_col_w);
    let h = full_h
        .saturating_sub(margin_top + margin_bottom)
        .max(8);
    let left = Rect::new(margin_x, margin_top, each, h);
    let right = Rect::new(
        margin_x.saturating_add(each).saturating_add(mid_gap),
        margin_top,
        each,
        h,
    );
    (left, right)
}
