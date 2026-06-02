//! Screen-space post-processing effects applied over an already-composed region of a
//! [`FrameBuffer`].
//!
//! Each [`ScreenEffect`] variant is **self-describing**: it carries everything needed to
//! draw it (colors, strength), so the policy deciding *when* an effect is active lives in
//! the game layer, not here. To add a new effect (magic corruption, poison tint, damage
//! flash, …) add a variant here and one arm in [`apply_screen_effects`]; the compose
//! pipeline and the policy layer are the only other touch points. To remove one, delete
//! the variant and its arm.

use crate::rect::Rect;
use crate::render::{Color, FrameBuffer};

/// A single screen-space effect to blend over a region of the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScreenEffect {
    /// Edge-weighted tint toward `color`. `strength` is the peak blend weight reached at
    /// the region edges/corners (`0` = invisible, `255` = fully `color`); the center is
    /// left untouched and the tint ramps in smoothly toward the border.
    Vignette { color: Color, strength: u8 },
}

/// Normalized distance from the region center (`0` center, `1` edge midpoint) below which
/// a vignette contributes nothing; the tint ramps smoothly from here outward. Lower =
/// tint reaches further toward the middle. Adjust to taste.
const VIGNETTE_INNER_RADIUS: f32 = 0.35;

/// How much of the vignette strength also tints glyph foregrounds (`0.0` = backgrounds
/// only, `1.0` = glyphs tinted as strongly as backgrounds). Keeps text readable.
const VIGNETTE_FG_TINT_RATIO: f32 = 0.5;

/// Apply each effect in order over `area` (clipped to the frame).
pub fn apply_screen_effects(fb: &mut FrameBuffer, area: Rect, effects: &[ScreenEffect]) {
    for effect in effects {
        match *effect {
            ScreenEffect::Vignette { color, strength } => apply_vignette(fb, area, color, strength),
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn apply_vignette(fb: &mut FrameBuffer, area: Rect, color: Color, strength: u8) {
    if strength == 0 || area.w == 0 || area.h == 0 {
        return;
    }
    let half_w = f32::from(area.w) / 2.0;
    let half_h = f32::from(area.h) / 2.0;
    let peak = f32::from(strength);
    for row in 0..area.h {
        for col in 0..area.w {
            // Cell center in coordinates normalized so the edge midpoints sit at +-1.
            let nx = (f32::from(col) + 0.5 - half_w) / half_w;
            let ny = (f32::from(row) + 0.5 - half_h) / half_h;
            let dist = nx.mul_add(nx, ny * ny).sqrt();
            let ramp =
                ((dist - VIGNETTE_INNER_RADIUS) / (1.0 - VIGNETTE_INNER_RADIUS)).clamp(0.0, 1.0);
            // Smoothstep for a soft edge instead of a hard ring.
            let eased = ramp * ramp * (3.0 - 2.0 * ramp);
            let weight = (eased * peak).round().clamp(0.0, 255.0) as u8;
            if weight == 0 {
                continue;
            }
            let x = area.x + col;
            let y = area.y + row;
            let Some(existing) = fb.get(x, y) else {
                continue;
            };
            let mut cell = existing.clone();
            let fg_weight = (f32::from(weight) * VIGNETTE_FG_TINT_RATIO)
                .round()
                .clamp(0.0, 255.0) as u8;
            cell.bg = cell.bg.blend_weight(color, weight);
            cell.fg = cell.fg.blend_weight(color, fg_weight);
            fb.set(x, y, cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_screen_effects, ScreenEffect};
    use crate::rect::Rect;
    use crate::render::{Cell, Color, FrameBuffer};

    fn solid_buffer(w: u16, h: u16, bg: Color) -> FrameBuffer {
        let mut fb = FrameBuffer::new(w, h);
        fb.fill_rect(
            Rect::new(0, 0, w, h),
            Cell {
                bg,
                ..Cell::default()
            },
        );
        fb
    }

    #[test]
    fn zero_strength_is_a_no_op() {
        let base = Color::rgb(10, 20, 30);
        let mut fb = solid_buffer(8, 6, base);
        apply_screen_effects(
            &mut fb,
            Rect::new(0, 0, 8, 6),
            &[ScreenEffect::Vignette {
                color: Color::rgb(255, 0, 0),
                strength: 0,
            }],
        );
        assert!(fb.cells().iter().all(|c| c.bg == base));
    }

    #[test]
    fn corner_is_tinted_more_than_center() {
        let base = Color::rgb(0, 0, 0);
        let tint = Color::rgb(200, 0, 0);
        let area = Rect::new(0, 0, 9, 9);
        let mut fb = solid_buffer(9, 9, base);
        apply_screen_effects(
            &mut fb,
            area,
            &[ScreenEffect::Vignette {
                color: tint,
                strength: 200,
            }],
        );
        let center = fb.get(4, 4).unwrap().bg;
        let corner = fb.get(0, 0).unwrap().bg;
        assert_eq!(center, base, "center should be left untouched");
        assert!(corner.r > center.r, "corner should be tinted toward red");
    }
}
