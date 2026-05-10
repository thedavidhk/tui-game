//! Game UI color tokens and typography defaults.
//!
//! Values follow the palette described in `docs/ui_design.md` (§3 / §13): UI chrome is **separate**
//! from terrain colors so the HUD reads as parchment/ink over the world rather than competing
//! with map fog and tile art.

use crate::render::Color;

/// Canonical RGB palette for game chrome (HUD, log, dialogue, modals).
///
/// Use [`GameUiPalette::DEFAULT`] for shipped visuals; tests may construct custom values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameUiPalette {
    pub panel_bg: Color,
    pub panel_bg_soft: Color,
    pub border_dim: Color,
    pub border_normal: Color,
    pub border_active: Color,
    pub text: Color,
    pub text_dim: Color,
    pub title: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub warning: Color,
    pub good: Color,
    pub magic: Color,
    /// Semitone wash over the world when a full-screen modal is open (still lets map read through).
    pub modal_scrim_bg: Color,
}

impl GameUiPalette {
    /// Palette from `docs/ui_design.md` §3.
    pub const DEFAULT: Self = Self {
        panel_bg: Color::rgb(20, 18, 25),
        panel_bg_soft: Color::rgb(26, 23, 32),
        border_dim: Color::rgb(116, 111, 99),
        border_normal: Color::rgb(185, 170, 130),
        border_active: Color::rgb(229, 211, 155),
        text: Color::rgb(216, 210, 192),
        text_dim: Color::rgb(143, 138, 125),
        title: Color::rgb(240, 217, 138),
        selected_fg: Color::rgb(244, 235, 208),
        selected_bg: Color::rgb(42, 38, 52),
        warning: Color::rgb(212, 106, 90),
        good: Color::rgb(155, 203, 138),
        magic: Color::rgb(185, 140, 255),
        modal_scrim_bg: Color::rgb(23, 21, 29),
    };
}

impl Default for GameUiPalette {
    fn default() -> Self {
        Self::DEFAULT
    }
}
