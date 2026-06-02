//! Effect **policy**: derives which [`ScreenEffect`]s and [`ActiveAreaEffect`]s are active
//! from current game state, each frame.
//!
//! This is the single bridge from simulation state to the presentation-only renderers in
//! [`crate::render::effects`] and [`crate::render::area_effects`].
//!
//! ## Screen effects (full-viewport post-processing)
//! - **Tune** the low-health vignette via the `LOW_HEALTH_*` constants below.
//! - **Add**: write `fn maybe_<effect>(game) -> Option<ScreenEffect>` and push it in
//!   [`active_screen_effects`].
//! - **Remove**: delete the helper and its push.
//!
//! ## Area effects (world-space radius tints)
//! - Stored as [`ActiveAreaEffect`] on [`Game`]; managed by [`Game::trigger_area_effect`]
//!   and ticked down in [`Game::tick_effects`].
//! - To query the list for rendering, use [`Game::active_area_effects`].
//!
//! Effects are recomputed from scratch each frame, so time-based variants only need a
//! countdown on [`Game`] plus one branch here.

use crate::game::Game;
use crate::render::area_effects::AreaEffect;
use crate::render::effects::ScreenEffect;
use crate::render::Color;

// ── Area-effect policy ────────────────────────────────────────────────────────

/// A persistent world-space area effect stored on [`Game`] between frames.
///
/// When `remaining_ticks` reaches 0 the effect is removed; set to `u32::MAX` for permanent
/// effects (e.g. level-defined burning braziers).
#[derive(Clone, Debug)]
pub struct ActiveAreaEffect {
    /// Rendering descriptor forwarded to [`crate::render::area_effects`] each frame.
    pub effect: AreaEffect,
    /// How many more game ticks this effect should live (`u32::MAX` = permanent).
    pub remaining_ticks: u32,
}

/// Player HP fraction (`0.0..=1.0`) at or below which the low-health vignette appears.
pub const LOW_HEALTH_THRESHOLD: f32 = 0.30;
/// Color the screen edges blend toward when near death.
pub const LOW_HEALTH_TINT: Color = Color::rgb(170, 20, 20);
/// Vignette strength right at the threshold (just barely visible).
pub const LOW_HEALTH_MIN_STRENGTH: u8 = 35;
/// Vignette strength at `0` HP (most intense).
pub const LOW_HEALTH_MAX_STRENGTH: u8 = 200;

/// Build the per-frame [`AreaEffect`] slice from the game's active area effects, advancing
/// the animation `phase` with `surface_tick`.
#[must_use]
pub(crate) fn frame_area_effects(game: &Game) -> Vec<AreaEffect> {
    game.active_area_effects
        .iter()
        .map(|ae| {
            let mut e = ae.effect;
            e.phase = (game.surface_tick & 0xFF) as u8;
            e
        })
        .collect()
}

/// All screen-space effects active for the current frame, in draw order.
#[must_use]
pub(crate) fn active_screen_effects(game: &Game) -> Vec<ScreenEffect> {
    let mut effects = Vec::new();
    if let Some(strength) = player_hp_ratio(game).and_then(low_health_vignette_strength) {
        effects.push(ScreenEffect::Vignette {
            color: LOW_HEALTH_TINT,
            strength,
        });
    }
    effects
}

/// Player current-HP fraction in `0.0..=1.0`, or `None` if there is no living player.
fn player_hp_ratio(game: &Game) -> Option<f32> {
    let stats = game.player_stats()?;
    if stats.max_hp == 0 {
        return None;
    }
    Some(f32::from(stats.hp) / f32::from(stats.max_hp))
}

/// Map an HP fraction to a vignette strength, or `None` above the threshold.
///
/// Linear from [`LOW_HEALTH_MIN_STRENGTH`] at the threshold to [`LOW_HEALTH_MAX_STRENGTH`]
/// at `0` HP. Kept pure so the curve can be unit-tested and tweaked in isolation.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn low_health_vignette_strength(ratio: f32) -> Option<u8> {
    if ratio > LOW_HEALTH_THRESHOLD {
        return None;
    }
    let severity = ((LOW_HEALTH_THRESHOLD - ratio) / LOW_HEALTH_THRESHOLD).clamp(0.0, 1.0);
    let span = f32::from(LOW_HEALTH_MAX_STRENGTH - LOW_HEALTH_MIN_STRENGTH);
    let strength = severity.mul_add(span, f32::from(LOW_HEALTH_MIN_STRENGTH));
    Some(strength.round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::{
        low_health_vignette_strength, LOW_HEALTH_MAX_STRENGTH, LOW_HEALTH_MIN_STRENGTH,
        LOW_HEALTH_THRESHOLD,
    };

    #[test]
    fn no_vignette_above_threshold() {
        assert_eq!(low_health_vignette_strength(1.0), None);
        assert_eq!(
            low_health_vignette_strength(LOW_HEALTH_THRESHOLD + 0.01),
            None
        );
    }

    #[test]
    fn strength_grows_as_health_drops() {
        let at_threshold = low_health_vignette_strength(LOW_HEALTH_THRESHOLD).unwrap();
        let near_death = low_health_vignette_strength(0.0).unwrap();
        assert_eq!(at_threshold, LOW_HEALTH_MIN_STRENGTH);
        assert_eq!(near_death, LOW_HEALTH_MAX_STRENGTH);
        assert!(low_health_vignette_strength(LOW_HEALTH_THRESHOLD / 2.0).unwrap() > at_threshold);
    }
}
