//! Full-screen overlay column geometry for the shipped game (margins only; no widget content).
//!
//! Horizontal extent follows [`crate::ui::layout::OverlayBandLayout`] (default **50%** of the
//! terminal, centered). Tune fraction constants there.

use crate::rect::Rect;
use crate::ui::layout::{split_horizontal_columns, OverlayBandLayout};

const MARGIN_X: u16 = 2;
const COLUMN_GAP: u16 = 2;
const BOTTOM_MARGIN: u16 = 10;
const MARGIN_TOP_RELAXED: u16 = 3;
const MARGIN_TOP_TIGHT: u16 = 2;

#[must_use]
pub(crate) fn two_column_relaxed(full_w: u16, full_h: u16) -> (Rect, Rect) {
    let band_w = OverlayBandLayout::band_width(full_w);
    let ix = OverlayBandLayout::band_left_x(full_w, band_w);
    let v = split_horizontal_columns(
        band_w,
        full_h,
        MARGIN_X,
        MARGIN_TOP_RELAXED,
        BOTTOM_MARGIN,
        COLUMN_GAP,
        2,
    );
    (
        shift_rect_x(v[0], ix),
        shift_rect_x(v[1], ix),
    )
}

#[must_use]
pub(crate) fn two_column_tight(full_w: u16, full_h: u16) -> (Rect, Rect) {
    let band_w = OverlayBandLayout::band_width(full_w);
    let ix = OverlayBandLayout::band_left_x(full_w, band_w);
    let v = split_horizontal_columns(
        band_w,
        full_h,
        MARGIN_X,
        MARGIN_TOP_TIGHT,
        BOTTOM_MARGIN,
        COLUMN_GAP,
        2,
    );
    (
        shift_rect_x(v[0], ix),
        shift_rect_x(v[1], ix),
    )
}

#[must_use]
fn shift_rect_x(r: Rect, dx: u16) -> Rect {
    Rect::new(r.x.saturating_add(dx), r.y, r.w, r.h)
}

/// Inventory screen: ~35% / 30% / 35% of the overlay band (`docs/ui_design.md` §7).
#[must_use]
pub(crate) fn three_column_inventory(full_w: u16, full_h: u16) -> (Rect, Rect, Rect) {
    let band_w = OverlayBandLayout::band_width(full_w);
    let ix = OverlayBandLayout::band_left_x(full_w, band_w);
    let inner_h = full_h
        .saturating_sub(MARGIN_TOP_RELAXED)
        .saturating_sub(BOTTOM_MARGIN)
        .max(8);
    let inner_w = band_w.saturating_sub(MARGIN_X.saturating_mul(2));
    let gaps = COLUMN_GAP.saturating_mul(2);
    let sum = inner_w.saturating_sub(gaps).max(24);
    let mut w0 = (u32::from(sum) * 35 / 100) as u16;
    let mut w2 = (u32::from(sum) * 35 / 100) as u16;
    let mut w1 = sum.saturating_sub(w0).saturating_sub(w2);
    const MIN_COL: u16 = 8;
    while w0 < MIN_COL && w1 > MIN_COL {
        w0 += 1;
        w1 -= 1;
    }
    while w2 < MIN_COL && w1 > MIN_COL {
        w2 += 1;
        w1 -= 1;
    }
    while w1 < MIN_COL && w0 > MIN_COL {
        w0 -= 1;
        w1 += 1;
    }
    let x0 = MARGIN_X.saturating_add(ix);
    let x1 = x0.saturating_add(w0).saturating_add(COLUMN_GAP);
    let x2 = x1.saturating_add(w1).saturating_add(COLUMN_GAP);
    (
        Rect::new(x0, MARGIN_TOP_RELAXED, w0, inner_h),
        Rect::new(x1, MARGIN_TOP_RELAXED, w1, inner_h),
        Rect::new(x2, MARGIN_TOP_RELAXED, w2, inner_h),
    )
}
