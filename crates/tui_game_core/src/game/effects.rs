//! Screen-effect **policy**: derives which [`ScreenEffect`]s are active from current game
//! state, each frame.
//!
//! This is the single bridge from simulation state (player HP today; magic proximity,
//! poison, recent damage later) to the presentation-only renderer in
//! [`crate::render::effects`]. Keeping the mapping here means an effect can be tuned,
//! added, or removed without touching the compose pipeline:
//!
//! - **Tune** the low-health vignette via the `LOW_HEALTH_*` constants below.
//! - **Add** an effect: write a `fn maybe_<effect>(game) -> Option<ScreenEffect>` and push
//!   it in [`active_screen_effects`].
//! - **Remove** an effect: delete its helper and its push; nothing else depends on it.
//!
//! Effects are recomputed from scratch each frame, so time-based variants (e.g. a brief
//! red flash on taking damage) only need a countdown on [`Game`] plus one branch here.

use crate::game::Game;
use crate::render::effects::ScreenEffect;
use crate::render::Color;

/// Player HP fraction (`0.0..=1.0`) at or below which the low-health vignette appears.
pub const LOW_HEALTH_THRESHOLD: f32 = 0.30;
/// Color the screen edges blend toward when near death.
pub const LOW_HEALTH_TINT: Color = Color::rgb(170, 20, 20);
/// Vignette strength right at the threshold (just barely visible).
pub const LOW_HEALTH_MIN_STRENGTH: u8 = 35;
/// Vignette strength at `0` HP (most intense).
pub const LOW_HEALTH_MAX_STRENGTH: u8 = 200;

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
