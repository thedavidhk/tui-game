//! Shared rectangle math for bordered panels and split overlays.
//!
//! ## Where to tune layout
//! - **Explorer shell** (world + HUD + log): [`GameShellLayout`]
//! - **Canvas + side panel** (level editor): [`PrimarySidePanelLayout`]
//! - **Multi-column full-screen bands**: [`split_horizontal_columns`] (callers choose margins and column count)
//! - **Floating panels** (main menu, dialogue strip, combat HUD, debug): [`FloatingPanelLayout`]
//! - **Centered modal panels**: [`centered_rect`] / [`centered_rect_dims`]

use crate::rect::Rect;
use crate::render::FrameBuffer;

// --- compile-time chrome (edit these structs' constants) --------------------

/// Three-pane layout used in exploration and any mode that shares the same shell.
///
/// Used by the binary main loop and by [`crate::game::Game::world_rect_for_viewport`]
/// so hit-testing and rendering stay aligned.
pub struct GameShellLayout;

impl GameShellLayout {
    /// Width of the right-hand status column (clamped so the map keeps [`Self::MIN_WORLD_WIDTH`]).
    pub const HUD_WIDTH: u16 = 26;
    /// Minimum number of columns left for the world view when clamping HUD width.
    pub const MIN_WORLD_WIDTH: u16 = 10;

    /// Height of the bottom log strip in terminal rows (full [`Rect::h`], including borders).
    /// Includes borders; body fits roughly 3–5 log lines plus an optional footer row.
    pub const LOG_HEIGHT: u16 = 7;
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

/// Primary content area (left) plus a fixed-width side panel (right): the level editor's
/// map + tool column, and a sibling to [`GameShellLayout`] for any "canvas + tools" screen.
pub struct PrimarySidePanelLayout;

impl PrimarySidePanelLayout {
    /// Columns the primary view always keeps when clamping the side-panel width.
    pub const MIN_PRIMARY_WIDTH: u16 = 1;

    /// `(primary, side)` rectangles for a `(viewport_w, viewport_h)` terminal with a side panel of
    /// at most `side_width` columns (clamped so the primary keeps [`Self::MIN_PRIMARY_WIDTH`]).
    #[must_use]
    pub fn root_panels(viewport_w: u16, viewport_h: u16, side_width: u16) -> (Rect, Rect) {
        let side_w = side_width.min(viewport_w.saturating_sub(Self::MIN_PRIMARY_WIDTH));
        let primary_w = viewport_w.saturating_sub(side_w).max(1);
        let primary = Rect::new(0, 0, primary_w, viewport_h);
        let side = Rect::new(
            primary.right(),
            0,
            viewport_w.saturating_sub(primary_w).max(1),
            viewport_h,
        );
        (primary, side)
    }
}

/// Width of centered overlay bands (dialogue, inventory, journal, chest transfer).
///
/// Tune **`WIDTH_FRAC_NUM`** / **`WIDTH_FRAC_DEN`** for fraction of the terminal (default **1/2**).
/// [`MIN_WIDTH`] prevents unreadably narrow panels on small terminals.
#[derive(Clone, Copy, Debug)]
pub struct OverlayBandLayout;

impl OverlayBandLayout {
    pub const WIDTH_FRAC_NUM: u32 = 1;
    pub const WIDTH_FRAC_DEN: u32 = 2;
    pub const MIN_WIDTH: u16 = 28;

    #[must_use]
    pub fn band_width(terminal_w: u16) -> u16 {
        let raw = u32::from(terminal_w).saturating_mul(Self::WIDTH_FRAC_NUM) / Self::WIDTH_FRAC_DEN;
        let w = (raw as u16).max(Self::MIN_WIDTH).min(terminal_w);
        w.max(1)
    }

    /// Left column for a [`band_width`] panel centered in `terminal_w`.
    #[must_use]
    pub fn band_left_x(terminal_w: u16, band_w: u16) -> u16 {
        terminal_w.saturating_sub(band_w) / 2
    }
}

/// Small anchored panels drawn on top of the world in `game::view::compose`.
pub struct FloatingPanelLayout;

impl FloatingPanelLayout {
    pub const MAIN_MENU_X: u16 = 2;
    pub const MAIN_MENU_Y: u16 = 2;
    pub const MAIN_MENU_W: u16 = 30;
    pub const MAIN_MENU_H: u16 = 10;

    /// Distance from the **top** of the dialogue [`Rect`] to the bottom of the framebuffer (`y = h - value`).
    pub const DIALOGUE_FROM_BOTTOM: u16 = 18;
    pub const DIALOGUE_HEIGHT: u16 = 10;

    pub const COMBAT_X: u16 = 2;
    pub const COMBAT_Y: u16 = 10;
    pub const COMBAT_W: u16 = 40;
    pub const COMBAT_H: u16 = 6;

    pub const DEBUG_WIDTH: u16 = 64;
    pub const DEBUG_FROM_TOP: u16 = 0;
    pub const DEBUG_HEIGHT: u16 = 10;

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
        let w = OverlayBandLayout::band_width(fb_w);
        let x = OverlayBandLayout::band_left_x(fb_w, w);
        Rect::new(
            x,
            fb_h.saturating_sub(Self::DIALOGUE_FROM_BOTTOM),
            w.max(1),
            Self::DIALOGUE_HEIGHT,
        )
    }

    #[must_use]
    pub fn combat_hud() -> Rect {
        Rect::new(
            Self::COMBAT_X,
            Self::COMBAT_Y,
            Self::COMBAT_W,
            Self::COMBAT_H,
        )
    }

    #[must_use]
    pub fn debug_panel(fb_w: u16) -> Rect {
        let w = Self::DEBUG_WIDTH;
        let x = fb_w.saturating_sub(Self::DEBUG_WIDTH) / 2;
        Rect::new(x, Self::DEBUG_FROM_TOP, w, Self::DEBUG_HEIGHT)
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

/// Centered rectangle within a `screen_w` × `screen_h` cell grid (clipped to the grid).
#[must_use]
pub fn centered_rect_dims(screen_w: u16, screen_h: u16, w: u16, h: u16) -> Rect {
    let w = w.min(screen_w);
    let h = h.min(screen_h);
    let x = screen_w.saturating_sub(w) / 2;
    let y = screen_h.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

/// Centered modal panel rect (clipped to the framebuffer).
#[must_use]
pub fn centered_rect(fb: &FrameBuffer, w: u16, h: u16) -> Rect {
    centered_rect_dims(fb.width, fb.height, w, h)
}

/// Inner content area below a single-line title row inside a two-line-bordered panel.
#[must_use]
pub fn panel_inner(panel: Rect) -> Rect {
    Rect::new(
        panel.x + 1,
        panel.y + 2,
        panel.w.saturating_sub(2),
        panel.h.saturating_sub(3),
    )
}

/// Divides a horizontal band (full framebuffer width by `full_h`) into `column_count` columns.
///
/// Margins shrink the band; `column_gap` is applied between adjacent columns. Column widths are
/// equal with remainder pixels assigned from the left. `column_count` must be ≥ 1 (panics in
/// debug if zero).
#[must_use]
pub fn split_horizontal_columns(
    full_w: u16,
    full_h: u16,
    margin_x: u16,
    margin_top: u16,
    margin_bottom: u16,
    column_gap: u16,
    column_count: u16,
) -> Vec<Rect> {
    debug_assert!(column_count >= 1, "column_count must be at least 1");
    let column_count = column_count.max(1);
    let inner_w = full_w.saturating_sub(margin_x.saturating_mul(2));
    let gaps_total = column_gap.saturating_mul(column_count.saturating_sub(1));
    let cols_w = inner_w.saturating_sub(gaps_total);
    let h = full_h
        .saturating_sub(margin_top.saturating_add(margin_bottom))
        .max(8);

    let base = cols_w / column_count;
    let rem = cols_w % column_count;
    let mut x = margin_x;
    let mut out = Vec::with_capacity(column_count as usize);
    for i in 0..column_count {
        let wcol = base + u16::from(i < rem);
        out.push(Rect::new(x, margin_top, wcol, h));
        x = x.saturating_add(wcol).saturating_add(column_gap);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{split_horizontal_columns, GameShellLayout, OverlayBandLayout, Rect};

    #[test]
    fn overlay_band_default_is_half_width_centered() {
        assert_eq!(OverlayBandLayout::band_width(80), 40);
        assert_eq!(OverlayBandLayout::band_left_x(80, 40), 20);
        assert_eq!(OverlayBandLayout::band_width(100), 50);
        assert_eq!(OverlayBandLayout::band_left_x(100, 50), 25);
    }

    #[test]
    fn split_horizontal_columns_two_matches_legacy_outer_split() {
        let v = split_horizontal_columns(80, 30, 2, 3, 3, 2, 2);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], Rect::new(2, 3, 37, 24));
        assert_eq!(v[1], Rect::new(41, 3, 37, 24));
    }

    #[test]
    fn split_horizontal_columns_three_distributes_width() {
        let v = split_horizontal_columns(80, 30, 2, 3, 3, 2, 3);
        assert_eq!(v.len(), 3);
        let sum_w: u32 = v.iter().map(|r| u32::from(r.w)).sum();
        // inner 76 - 2 gaps of 2 => 72 for columns
        assert_eq!(sum_w, 72);
        assert_eq!(v[0].x, 2);
        assert!(v[1].x > v[0].x);
        assert!(v[2].x > v[1].x);
    }

    #[test]
    fn game_shell_root_panels_matches_formula() {
        let w = 100u16;
        let h = 40u16;
        let (world, hud, log) = GameShellLayout::root_panels(w, h);
        let hud_w =
            GameShellLayout::HUD_WIDTH.min(w.saturating_sub(GameShellLayout::MIN_WORLD_WIDTH));
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
