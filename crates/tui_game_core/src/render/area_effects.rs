//! World-space area effects: color and glyph modifications applied to a radius of world
//! cells projected onto the [`FrameBuffer`].
//!
//! The policy (what effects exist, where, how long) lives in
//! [`crate::game::effects`]; this module is pure presentation.
//!
//! ## Adding a new effect
//! 1. Add a variant to [`AreaEffectKind`] with whatever parameters the renderer needs.
//! 2. Add a match arm in [`apply_one_area_effect`] that reads those parameters and calls
//!    [`blend_cell`] (or does custom cell manipulation).
//! 3. Write the policy side in `game/effects.rs` (an `ActiveAreaEffect` entry).
//!
//! ## Removing an effect
//! Delete the [`AreaEffectKind`] variant and its arm in [`apply_one_area_effect`]; nothing
//! else refers to it.

use crate::entity::GridPos;
use crate::render::{Cell, Color, FrameBuffer};

// ── Public types ──────────────────────────────────────────────────────────────

/// Descriptor for a single world-space area effect.
///
/// The renderer needs only what's in this struct; it must not access game state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaEffect {
    /// World-space center of the effect.
    pub center: GridPos,
    /// Chebyshev radius (in tiles) over which the effect applies. Strength falls off linearly
    /// from 1 at the center tile to 0 at `radius + 1`.
    pub radius: u8,
    /// Peak blend strength at the center cell (`0..=255`). Cells at the edge receive
    /// `strength * (1 - dist/radius)`.
    pub strength: u8,
    /// Which visual treatment to apply.
    pub kind: AreaEffectKind,
    /// Animation phase (`surface_tick & 0xFF` recommended): some effects use this to flicker.
    pub phase: u8,
}

/// The visual treatment applied to cells within an [`AreaEffect`]'s radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaEffectKind {
    /// Warm orange-red tint; intense center cells get glyph-replaced with fire glyphs
    /// that cycle with `phase`.
    Fire,
    /// Soft green-yellow tint; can represent poison clouds, swamp gas, corrupted air.
    PoisonCloud,
    /// Configurable-color magical aura. A faint tint over the area, strongest at center.
    MagicAura {
        color: Color,
    },
    /// Generic color tint (background blend only). Useful for quick experiments.
    Tint {
        color: Color,
    },
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Apply all `effects` to `fb` within `world_rect`.
///
/// `world_origin` is the world coordinate `(wx, wy)` that maps to screen cell
/// `(world_rect.x, world_rect.y)` — i.e. the top-left world tile currently visible.
pub fn apply_area_effects(
    fb: &mut FrameBuffer,
    world_origin: (i32, i32),
    world_rect: crate::rect::Rect,
    effects: &[AreaEffect],
) {
    for effect in effects {
        apply_one_area_effect(fb, world_origin, world_rect, effect);
    }
}

// ── Per-kind rendering ────────────────────────────────────────────────────────

/// Glyphs cycled at the center of a fire effect (most intense cells), indexed by `phase`.
const FIRE_GLYPHS: &[char] = &['▓', '▒', '░', '*', '▒', '▓', '*', '░'];

/// Base orange-red fire tint color.
const FIRE_TINT: Color = Color::rgb(220, 110, 20);

/// Poison-cloud tint.
const POISON_TINT: Color = Color::rgb(80, 200, 60);

// wx_lo/wx_hi/wy_lo/wy_hi are inherently similar axis-pair names in 2-D iteration.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names
)]
fn apply_one_area_effect(
    fb: &mut FrameBuffer,
    (ox, oy): (i32, i32),
    world_rect: crate::rect::Rect,
    effect: &AreaEffect,
) {
    if effect.strength == 0 || effect.radius == 0 {
        return;
    }
    let r = i32::from(effect.radius);
    let cx = effect.center.x;
    let cy = effect.center.y;

    // Iterate only the bounding box of the effect, clamped to the viewport.
    let wx_lo = cx - r;
    let wx_hi = cx + r;
    let wy_lo = cy - r;
    let wy_hi = cy + r;

    for wy in wy_lo..=wy_hi {
        for wx in wx_lo..=wx_hi {
            let dist_sq = crate::math::euclidean_dist_sq(wx - cx, wy - cy);
            let r_sq = i64::from(r) * i64::from(r);
            if dist_sq > r_sq {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let dist = (dist_sq as f32).sqrt().round() as i32;
            // World → screen.
            let sx = wx - ox;
            let sy = wy - oy;
            if sx < 0
                || sy < 0
                || sx >= i32::from(world_rect.w)
                || sy >= i32::from(world_rect.h)
            {
                continue;
            }
            let screen_x = world_rect.x + sx as u16;
            let screen_y = world_rect.y + sy as u16;
            let Some(existing) = fb.get(screen_x, screen_y) else {
                continue;
            };
            // Linear falloff: 1.0 at center, approaching 0 at dist == radius.
            // Coordinates fit comfortably in f32 (map coords are small i32 values).
            #[allow(clippy::cast_precision_loss)]
            let falloff = 1.0 - dist as f32 / (r as f32 + 1.0);
            let cell_strength =
                (f32::from(effect.strength) * falloff).round().clamp(0.0, 255.0) as u8;
            if cell_strength == 0 {
                continue;
            }

            let mut cell = existing.clone();
            render_kind(
                &mut cell,
                effect.kind,
                cell_strength,
                dist,
                effect.phase,
            );
            fb.set(screen_x, screen_y, cell);
        }
    }
}

fn render_kind(cell: &mut Cell, kind: AreaEffectKind, strength: u8, dist: i32, phase: u8) {
    match kind {
        AreaEffectKind::Fire => {
            cell.bg = blend_cell(cell.bg, FIRE_TINT, strength);
            // Replace glyphs at high intensity (center cells) with animated fire.
            if dist == 0 && strength > 160 {
                let idx = (phase as usize / 2) % FIRE_GLYPHS.len();
                cell.ch = FIRE_GLYPHS[idx];
                // Brighten fg so fire glyphs stand out.
                cell.fg = blend_cell(cell.fg, Color::rgb(255, 200, 100), strength / 2);
            }
        }
        AreaEffectKind::PoisonCloud => {
            cell.bg = blend_cell(cell.bg, POISON_TINT, strength / 2);
            cell.fg = blend_cell(cell.fg, POISON_TINT, strength / 4);
        }
        AreaEffectKind::MagicAura { color } | AreaEffectKind::Tint { color } => {
            cell.bg = blend_cell(cell.bg, color, strength);
        }
    }
}

/// Blend `base` toward `tint` by `weight/255`.
#[inline]
fn blend_cell(base: Color, tint: Color, weight: u8) -> Color {
    base.blend_weight(tint, weight)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::GridPos;
    use crate::rect::Rect;
    use crate::render::{Cell, Color, FrameBuffer};

    fn black_fb(w: u16, h: u16) -> FrameBuffer {
        let mut fb = FrameBuffer::new(w, h);
        fb.fill_rect(Rect::new(0, 0, w, h), Cell::default());
        fb
    }

    fn pos(x: i32, y: i32) -> GridPos {
        GridPos { x, y }
    }

    #[test]
    fn zero_strength_is_noop() {
        let mut fb = black_fb(10, 10);
        apply_area_effects(
            &mut fb,
            (0, 0),
            Rect::new(0, 0, 10, 10),
            &[AreaEffect {
                center: pos(5, 5),
                radius: 3,
                strength: 0,
                kind: AreaEffectKind::Fire,
                phase: 0,
            }],
        );
        assert!(fb.cells().iter().all(|c| c.bg == Color::rgb(0, 0, 0)));
    }

    #[test]
    fn fire_tints_center_cell() {
        let mut fb = black_fb(10, 10);
        apply_area_effects(
            &mut fb,
            (0, 0),
            Rect::new(0, 0, 10, 10),
            &[AreaEffect {
                center: pos(5, 5),
                radius: 2,
                strength: 200,
                kind: AreaEffectKind::Fire,
                phase: 0,
            }],
        );
        let center = fb.get(5, 5).unwrap().bg;
        // Center should be tinted toward orange-red (r component raised).
        assert!(center.r > 0, "fire should raise red channel at center");
    }

    #[test]
    fn cells_outside_radius_are_untouched() {
        let mut fb = black_fb(20, 10);
        apply_area_effects(
            &mut fb,
            (0, 0),
            Rect::new(0, 0, 20, 10),
            &[AreaEffect {
                center: pos(5, 5),
                radius: 2,
                strength: 200,
                kind: AreaEffectKind::Fire,
                phase: 0,
            }],
        );
        // Cell at Euclidean dist 3 from center should be untouched (radius 2).
        let outside = fb.get(8, 5).unwrap().bg;
        assert_eq!(outside, Color::rgb(0, 0, 0), "outside radius should be black");
    }

    #[test]
    fn strength_falls_off_from_center() {
        let mut fb = black_fb(20, 10);
        apply_area_effects(
            &mut fb,
            (0, 0),
            Rect::new(0, 0, 20, 10),
            &[AreaEffect {
                center: pos(5, 5),
                radius: 4,
                strength: 200,
                kind: AreaEffectKind::Tint {
                    color: Color::rgb(255, 0, 0),
                },
                phase: 0,
            }],
        );
        let center_r = fb.get(5, 5).unwrap().bg.r;
        let edge_r = fb.get(9, 5).unwrap().bg.r; // Euclidean dist ≈ 4 (the edge)
        assert!(center_r > edge_r, "tint should be stronger at center than edge");
    }

    #[test]
    fn world_origin_offset_maps_correctly() {
        // Place effect at world (10, 10), viewport origin at (8, 8), so effect center
        // maps to screen (2, 2).
        let mut fb = black_fb(10, 10);
        apply_area_effects(
            &mut fb,
            (8, 8),
            Rect::new(0, 0, 10, 10),
            &[AreaEffect {
                center: pos(10, 10),
                radius: 1,
                strength: 200,
                kind: AreaEffectKind::Tint {
                    color: Color::rgb(0, 0, 255),
                },
                phase: 0,
            }],
        );
        let center = fb.get(2, 2).unwrap().bg;
        assert!(center.b > 0, "blue tint should appear at the mapped screen position");
        let unaffected = fb.get(0, 0).unwrap().bg;
        assert_eq!(unaffected, Color::rgb(0, 0, 0), "far cell should be unaffected");
    }
}
