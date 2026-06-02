//! Tick-based visual pacing for discrete steps (exploration auto-walk, NPC combat).
//!
//! Call sites assume the outer loop runs at a stable tick rate (see `tui_game` poll interval,
//! currently ~60 Hz for exploration and combat). Cooldown is in **game ticks** after a
//! successful move; the next move is allowed when it reaches 0, so the gap between moves is
//! `(cooldown + 1)` ticks.
//!
//! When [`crate::math::tile_aspect_enabled`], vertical steps use a longer cooldown so
//! on-screen motion matches horizontal speed (see [`scaled_step_cooldown`]).

use crate::math;

/// Base cooldown ticks after a successful move (before tile-aspect scaling).
#[must_use]
pub(crate) fn visual_step_cooldown_ticks_from_speed(speed: u16) -> u16 {
    // Tuned to match the former on-screen pace when the main loop was ~30 Hz (cooldown 2/1/0
    // there) after moving to ~60 Hz: multiply the old gap `(old_cd + 1)` by two and subtract 1.
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

/// Player / NPC combat pacing from actor speed and step direction.
#[must_use]
pub(crate) fn visual_step_cooldown_for_move(speed: u16, dx: i32, dy: i32) -> u16 {
    scaled_step_cooldown(visual_step_cooldown_ticks_from_speed(speed), dx, dy)
}
