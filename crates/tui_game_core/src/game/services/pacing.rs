//! Re-exports for game-layer call sites (see [`crate::step_pacing`] for implementation).

pub(crate) use crate::step_pacing::{
    explore_step_cooldown, visual_step_cooldown_for_move, visual_step_cooldown_ticks_from_speed,
};
