//! Unified NPC action intents (exploration and turn-based).

use crate::combat::AttackStyle;
use crate::entity::{EntityId, GridPos};

/// How urgently an NPC is moving on the exploration grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPace {
    /// Flee, chase, investigate.
    Urgent,
    /// Roam or patrol.
    Leisurely,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcAction {
    Step {
        target: GridPos,
        pace: StepPace,
    },
    Attack {
        target: EntityId,
        style: AttackStyle,
    },
    Pass,
    Idle,
}

impl NpcAction {
    #[must_use]
    pub const fn step(target: GridPos) -> Self {
        Self::Step {
            target,
            pace: StepPace::Urgent,
        }
    }

    #[must_use]
    pub const fn roam_step(target: GridPos) -> Self {
        Self::Step {
            target,
            pace: StepPace::Leisurely,
        }
    }

    #[must_use]
    pub const fn step_target(self) -> Option<GridPos> {
        match self {
            Self::Step { target, .. } => Some(target),
            Self::Attack { .. } | Self::Pass | Self::Idle => None,
        }
    }

    #[must_use]
    pub const fn step_pace(self) -> Option<StepPace> {
        match self {
            Self::Step { pace, .. } => Some(pace),
            Self::Attack { .. } | Self::Pass | Self::Idle => None,
        }
    }

    #[must_use]
    pub const fn is_leisurely_step(self) -> bool {
        matches!(
            self,
            Self::Step {
                pace: StepPace::Leisurely,
                ..
            }
        )
    }

    /// Exploration tick cooldown after a successful step, or `None` for non-move actions.
    #[must_use]
    pub fn explore_step_cooldown(self, speed: u16, dx: i32, dy: i32) -> Option<u16> {
        self.step_target()?;
        Some(crate::step_pacing::explore_step_cooldown(
            self.is_leisurely_step(),
            speed,
            dx,
            dy,
        ))
    }
}
