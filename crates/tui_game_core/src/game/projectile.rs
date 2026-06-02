//! In-flight projectiles and melee-flash effects — purely visual, driven by `Game::tick_effects`.
//!
//! A [`Projectile`] is spawned by [`super::mod::Game`] when an attack is committed and removed
//! after `total_ticks` have elapsed.  The associated [`super::super::combat::PendingHit`] fires
//! at `delay_ticks == 0`, which coincides with the projectile reaching its target.

use crate::entity::GridPos;
use crate::render::Color;

/// A flying arrow or melee flash currently animating on the world view.
#[derive(Clone, Debug)]
pub struct Projectile {
    pub from: GridPos,
    pub to: GridPos,
    /// How many ticks have elapsed since spawning.
    pub ticks_elapsed: u8,
    /// Total lifetime; the projectile is removed once `ticks_elapsed >= total_ticks`.
    pub total_ticks: u8,
    pub glyph: char,
    pub color: Color,
    /// True = ranged arrow travelling from `from` to `to`.  False = melee flash at `to`.
    pub is_ranged: bool,
}

impl Projectile {
    /// Current interpolated world position (0..1 fraction along the from→to segment).
    #[must_use]
    pub fn current_pos(&self) -> GridPos {
        if self.total_ticks == 0 || !self.is_ranged {
            return self.to;
        }
        let t = f32::from(self.ticks_elapsed) / f32::from(self.total_ticks);
        let x = self.from.x as f32 + (self.to.x - self.from.x) as f32 * t;
        let y = self.from.y as f32 + (self.to.y - self.from.y) as f32 * t;
        GridPos {
            x: x.round() as i32,
            y: y.round() as i32,
        }
    }

    /// Returns `true` when the projectile should be removed.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.ticks_elapsed >= self.total_ticks
    }
}

/// Arrow color (slightly warm white).
pub const ARROW_COLOR: Color = Color::rgb(230, 220, 160);

/// Melee flash color (orange-white).
pub const MELEE_FLASH_COLOR: Color = Color::rgb(255, 200, 120);

/// Direction-aware arrow glyph from `from` to `to`.
#[must_use]
pub fn arrow_glyph(from: GridPos, to: GridPos) -> char {
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();
    match (dx, dy) {
        (1, 0) => '→',
        (-1, 0) => '←',
        (0, -1) => '↑',
        (0, 1) => '↓',
        (1, -1) => '↗',
        (-1, -1) => '↖',
        (1, 1) => '↘',
        (-1, 1) => '↙',
        _ => '*', // same cell (shouldn't happen in valid combat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i32, y: i32) -> GridPos {
        GridPos { x, y }
    }

    #[test]
    fn arrow_glyph_cardinal_directions() {
        assert_eq!(arrow_glyph(pos(0, 0), pos(3, 0)), '→');
        assert_eq!(arrow_glyph(pos(3, 0), pos(0, 0)), '←');
        assert_eq!(arrow_glyph(pos(0, 3), pos(0, 0)), '↑');
        assert_eq!(arrow_glyph(pos(0, 0), pos(0, 3)), '↓');
    }

    #[test]
    fn arrow_glyph_diagonals() {
        assert_eq!(arrow_glyph(pos(0, 0), pos(3, -3)), '↗');
        assert_eq!(arrow_glyph(pos(0, 0), pos(-3, -3)), '↖');
        assert_eq!(arrow_glyph(pos(0, 0), pos(3, 3)), '↘');
        assert_eq!(arrow_glyph(pos(0, 0), pos(-3, 3)), '↙');
    }

    #[test]
    fn projectile_interpolates_to_midpoint() {
        let p = Projectile {
            from: pos(0, 0),
            to: pos(10, 0),
            ticks_elapsed: 5,
            total_ticks: 10,
            glyph: '→',
            color: ARROW_COLOR,
            is_ranged: true,
        };
        assert_eq!(p.current_pos(), pos(5, 0));
    }

    #[test]
    fn melee_flash_always_at_target() {
        let p = Projectile {
            from: pos(0, 0),
            to: pos(1, 0),
            ticks_elapsed: 2,
            total_ticks: 6,
            glyph: '*',
            color: MELEE_FLASH_COLOR,
            is_ranged: false,
        };
        assert_eq!(p.current_pos(), pos(1, 0));
    }
}
