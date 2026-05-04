//! Full-screen overlay column geometry for the shipped game (margins only; no widget content).
//!
//! Generic splitting lives in [`crate::ui::layout::split_horizontal_columns`]. Adjust these
//! constants when tuning how far overlays sit from the screen edges.

use crate::rect::Rect;
use crate::ui::layout::split_horizontal_columns;

const MARGIN_X: u16 = 2;
const COLUMN_GAP: u16 = 2;
const BOTTOM_MARGIN: u16 = 3;
const MARGIN_TOP_RELAXED: u16 = 3;
const MARGIN_TOP_TIGHT: u16 = 2;

#[must_use]
pub(crate) fn two_column_relaxed(full_w: u16, full_h: u16) -> (Rect, Rect) {
    let v = split_horizontal_columns(
        full_w,
        full_h,
        MARGIN_X,
        MARGIN_TOP_RELAXED,
        BOTTOM_MARGIN,
        COLUMN_GAP,
        2,
    );
    (v[0], v[1])
}

#[must_use]
pub(crate) fn two_column_tight(full_w: u16, full_h: u16) -> (Rect, Rect) {
    let v = split_horizontal_columns(
        full_w,
        full_h,
        MARGIN_X,
        MARGIN_TOP_TIGHT,
        BOTTOM_MARGIN,
        COLUMN_GAP,
        2,
    );
    (v[0], v[1])
}

#[must_use]
pub(crate) fn three_column_relaxed(full_w: u16, full_h: u16) -> (Rect, Rect, Rect) {
    let v = split_horizontal_columns(
        full_w,
        full_h,
        MARGIN_X,
        MARGIN_TOP_RELAXED,
        BOTTOM_MARGIN,
        COLUMN_GAP,
        3,
    );
    (v[0], v[1], v[2])
}
