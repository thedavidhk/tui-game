//! Tick-based visual pacing for discrete steps (exploration auto-walk, NPC combat).
//!
//! Call sites assume the outer loop runs at a stable tick rate (see `tui_game` poll interval).

/// Cooldown ticks after a successful move or attack so multi-AP turns do not resolve in one frame.
pub(crate) fn visual_step_cooldown_ticks_from_speed(speed: u16) -> u16 {
    // Tuned for smoothness at ~30 Hz:
    // speed 1-3  => every ~99ms (2 cooldown ticks)
    // speed 4-7  => every ~66ms (1 cooldown tick)
    // speed 8+   => every ~33ms (0 cooldown ticks)
    match speed {
        0..=3 => 2,
        4..=7 => 1,
        _ => 0,
    }
}
