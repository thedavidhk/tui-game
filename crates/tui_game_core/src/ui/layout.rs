//! Shared rectangle math for bordered panels and split overlays.
//!
//! ## Where to tune layout
//! - **Explorer shell** (world + HUD + log): [`GameShellLayout`]
//! - **Full-screen two-column overlays** (inventory, journal, transfer): [`OverlaySplitConfig`]
//! - **Floating panels** (main menu, dialogue strip, combat HUD, debug): [`FloatingPanelLayout`]

use crate::rect::Rect;

// --- compile-time chrome (edit these structs' constants) --------------------

/// Three-pane layout used in exploration and any mode that shares the same shell.
///
/// Used by the binary main loop and by [`crate::game::Game::world_rect_for_viewport`]
/// so hit-testing and rendering stay aligned.
pub struct GameShellLayout;

impl GameShellLayout {
    /// Width of the right-hand status column (clamped so the map keeps [`Self::MIN_WORLD_WIDTH`]).
    pub const HUD_WIDTH: u16 = 32;
    /// Minimum number of columns left for the world view when clamping HUD width.
    pub const MIN_WORLD_WIDTH: u16 = 10;

    /// Height of the bottom log strip in terminal rows (full [`Rect::h`], including borders).
    pub const LOG_HEIGHT: u16 = 8;
    /// Minimum number of rows left for the world view above the log when clamping log height.
    pub const MIN_WORLD_HEIGHT: u16 = 3;

    /// `(world, hud, log)` rectangles for a terminal of size `(viewport_w, viewport_h)`.
    #[must_use]
    pub fn root_panels(viewport_w: u16, viewport_h: u16) -> (Rect, Rect, Rect) {
        let hud_w = Self::HUD_WIDTH.min(viewport_w.saturating_sub(Self::MIN_WORLD_WIDTH));
        let log_h = Self::LOG_HEIGHT.min(viewport_h.saturating_sub(Self::MIN_WORLD_HEIGHT));
        let world = Rect::new(
            0,
            0,
            viewport_w.saturating_sub(hud_w),
            viewport_h.saturating_sub(log_h),
        );
        let hud = Rect::new(world.w, 0, hud_w, viewport_h.saturating_sub(log_h));
        let log = Rect::new(0, world.h, viewport_w, log_h);
        (world, hud, log)
    }
}

/// Margins passed to [`split_horizontal_outer`] for inventory, journal, and chest UIs.
pub struct OverlaySplitConfig;

impl OverlaySplitConfig {
    pub const MARGIN_X: u16 = 2;
    pub const MID_GAP: u16 = 2;
    pub const MIN_COL_WIDTH: u16 = 18;
    pub const BOTTOM_MARGIN: u16 = 3;

    /// Top margin for journal and inventory overlays.
    pub const JOURNAL_INVENTORY_TOP: u16 = 3;
    /// Top margin for the item-transfer overlay (slightly tighter).
    pub const TRANSFER_TOP: u16 = 2;

    #[must_use]
    pub fn journal_or_inventory(fb_w: u16, fb_h: u16) -> (Rect, Rect) {
        split_horizontal_outer(
            fb_w,
            fb_h,
            Self::MARGIN_X,
            Self::JOURNAL_INVENTORY_TOP,
            Self::BOTTOM_MARGIN,
            Self::MID_GAP,
            Self::MIN_COL_WIDTH,
        )
    }

    #[must_use]
    pub fn item_transfer(fb_w: u16, fb_h: u16) -> (Rect, Rect) {
        split_horizontal_outer(
            fb_w,
            fb_h,
            Self::MARGIN_X,
            Self::TRANSFER_TOP,
            Self::BOTTOM_MARGIN,
            Self::MID_GAP,
            Self::MIN_COL_WIDTH,
        )
    }
}

/// Small anchored panels drawn on top of the world in `game::view::compose`.
pub struct FloatingPanelLayout;

impl FloatingPanelLayout {
    pub const MAIN_MENU_X: u16 = 2;
    pub const MAIN_MENU_Y: u16 = 2;
    pub const MAIN_MENU_W: u16 = 30;
    pub const MAIN_MENU_H: u16 = 10;

    /// Horizontal inset from each screen edge for the dialogue band.
    pub const DIALOGUE_MARGIN_X: u16 = 2;
    /// Distance from the **top** of the dialogue [`Rect`] to the bottom of the framebuffer (`y = h - value`).
    pub const DIALOGUE_FROM_BOTTOM: u16 = 12;
    pub const DIALOGUE_HEIGHT: u16 = 10;

    pub const COMBAT_X: u16 = 2;
    pub const COMBAT_Y: u16 = 10;
    pub const COMBAT_W: u16 = 40;
    pub const COMBAT_H: u16 = 6;

    pub const DEBUG_MARGIN_X: u16 = 2;
    /// Same convention as [`Self::DIALOGUE_FROM_BOTTOM`]: `y = fb_h - value`.
    pub const DEBUG_FROM_BOTTOM: u16 = 8;
    pub const DEBUG_HEIGHT: u16 = 6;

    pub const GAME_OVER_W: u16 = 38;
    pub const GAME_OVER_H: u16 = 8;

    #[must_use]
    pub fn main_menu() -> Rect {
        Rect::new(
            Self::MAIN_MENU_X,
            Self::MAIN_MENU_Y,
            Self::MAIN_MENU_W,
            Self::MAIN_MENU_H,
        )
    }

    #[must_use]
    pub fn dialogue_band(fb_w: u16, fb_h: u16) -> Rect {
        let w = fb_w.saturating_sub(Self::DIALOGUE_MARGIN_X.saturating_mul(2));
        Rect::new(
            Self::DIALOGUE_MARGIN_X,
            fb_h.saturating_sub(Self::DIALOGUE_FROM_BOTTOM),
            w,
            Self::DIALOGUE_HEIGHT,
        )
    }

    #[must_use]
    pub fn combat_hud() -> Rect {
        Rect::new(Self::COMBAT_X, Self::COMBAT_Y, Self::COMBAT_W, Self::COMBAT_H)
    }

    #[must_use]
    pub fn debug_panel(fb_w: u16, fb_h: u16) -> Rect {
        let w = fb_w.saturating_sub(Self::DEBUG_MARGIN_X.saturating_mul(2));
        Rect::new(
            Self::DEBUG_MARGIN_X,
            fb_h.saturating_sub(Self::DEBUG_FROM_BOTTOM),
            w,
            Self::DEBUG_HEIGHT,
        )
    }

    /// Centered panel for the game-over overlay.
    #[must_use]
    pub fn game_over(fb_w: u16, fb_h: u16) -> Rect {
        let w = Self::GAME_OVER_W.min(fb_w);
        let h = Self::GAME_OVER_H.min(fb_h);
        let x = fb_w.saturating_sub(w) / 2;
        let y = fb_h.saturating_sub(h) / 2;
        Rect::new(x, y, w, h)
    }
}

// --- geometry helpers -------------------------------------------------------

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
    use super::{split_horizontal_outer, GameShellLayout, Rect};

    #[test]
    fn split_horizontal_outer_uses_available_width() {
        let (left, right) = split_horizontal_outer(80, 30, 2, 3, 3, 2, 18);
        assert_eq!(left.w, 37);
        assert_eq!(right.w, 37);
        assert_eq!(left.x, 2);
        assert_eq!(right.x, 41);
    }

    #[test]
    fn game_shell_root_panels_matches_formula() {
        let w = 100u16;
        let h = 40u16;
        let (world, hud, log) = GameShellLayout::root_panels(w, h);
        let hud_w = GameShellLayout::HUD_WIDTH.min(w.saturating_sub(GameShellLayout::MIN_WORLD_WIDTH));
        let log_h =
            GameShellLayout::LOG_HEIGHT.min(h.saturating_sub(GameShellLayout::MIN_WORLD_HEIGHT));
        assert_eq!(hud.w, hud_w);
        assert_eq!(log.h, log_h);
        assert_eq!(
            world,
            Rect::new(0, 0, w.saturating_sub(hud_w), h.saturating_sub(log_h))
        );
        assert_eq!(hud, Rect::new(world.w, 0, hud_w, h.saturating_sub(log_h)));
        assert_eq!(log, Rect::new(0, world.h, w, log_h));
    }
}
