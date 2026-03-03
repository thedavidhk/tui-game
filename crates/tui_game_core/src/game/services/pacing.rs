//! Tick-based visual pacing for discrete steps (exploration auto-walk, NPC combat).
//!
//! Call sites assume the outer loop runs at a stable tick rate (see `tui_game` poll interval,
//! currently ~60 Hz for exploration and combat). Cooldown is in **game ticks** after a
//! successful move; the next move is allowed when it reaches 0, so the gap between moves is
//! `(cooldown + 1)` ticks.

/// Cooldown ticks after a successful move or attack so multi-AP turns do not resolve in one frame.
pub(crate) fn visual_step_cooldown_ticks_from_speed(speed: u16) -> u16 {
    // Tuned to match the former on-screen pace when the main loop was ~30 Hz (cooldown 2/1/0
    // there) after moving to ~60 Hz: multiply the old gap `(old_cd + 1)` by two and subtract 1.
    match speed {
        0..=3 => 5,
        4..=7 => 3,
        _ => 1,
    }
}
