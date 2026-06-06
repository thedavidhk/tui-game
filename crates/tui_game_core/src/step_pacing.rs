//! Tick-based visual pacing for discrete grid steps (exploration NPCs, combat).

use crate::math;

/// Base exploration cooldown for casual roam/patrol steps (before aspect scaling).
pub(crate) const EXPLORE_ROAM_BASE_TICKS: u16 = 6;

/// Roam/patrol moves at half the urgent step rate.
pub(crate) const ROAM_PACE_MULTIPLIER: u16 = 2;

/// Base cooldown ticks after a successful move (before tile-aspect scaling).
#[must_use]
pub(crate) fn visual_step_cooldown_ticks_from_speed(speed: u16) -> u16 {
    match speed {
        0..=3 => 5,
        4..=7 => 3,
        _ => 1,
    }
}

/// Cooldown after a step in direction `(dx, dy)`, scaled for terminal tile aspect.
#[must_use]
pub(crate) fn scaled_step_cooldown(base_ticks: u16, dx: i32, dy: i32) -> u16 {
    if !math::tile_aspect_enabled() {
        return base_ticks;
    }
    let mult = math::grid_step_pace_multiplier(dx, dy);
    base_ticks.saturating_mul(mult)
}

/// Exploration pacing from actor speed, step direction, and urgency.
#[must_use]
pub(crate) fn explore_step_cooldown(leisurely: bool, speed: u16, dx: i32, dy: i32) -> u16 {
    let base = if leisurely {
        EXPLORE_ROAM_BASE_TICKS
    } else {
        visual_step_cooldown_ticks_from_speed(speed)
    };
    let mut cooldown = scaled_step_cooldown(base, dx, dy);
    if leisurely {
        cooldown = cooldown.saturating_mul(ROAM_PACE_MULTIPLIER);
    }
    cooldown
}

/// Player / NPC combat pacing from actor speed and step direction.
#[must_use]
pub(crate) fn visual_step_cooldown_for_move(speed: u16, dx: i32, dy: i32) -> u16 {
    scaled_step_cooldown(visual_step_cooldown_ticks_from_speed(speed), dx, dy)
}
