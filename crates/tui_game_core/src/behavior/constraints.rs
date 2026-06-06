//! Whether an actor may act this tick (realtime pacing vs turn clock).

use crate::combat::CombatState;
use crate::entity::EntityId;

/// Inputs from the game layer about time and turn rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionPhase {
    /// Overworld realtime — per-actor step cooldown on [`crate::entity::NpcBrainState`].
    RealtimeExplore,
    /// Turn session active — only `current_actor` may act, AP applies.
    Turn {
        turn_index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionConstraints {
    pub phase: ActionPhase,
    pub actor: EntityId,
    /// Lethal/training encounter rules are active (combat HUD), not overworld turn clock only.
    pub encounter_active: bool,
}

impl ActionConstraints {
    #[must_use]
    pub fn realtime(actor: EntityId) -> Self {
        Self {
            phase: ActionPhase::RealtimeExplore,
            actor,
            encounter_active: false,
        }
    }

    #[must_use]
    pub fn for_turn(clock: &CombatState, actor: EntityId, encounter_active: bool) -> Self {
        Self {
            phase: ActionPhase::Turn {
                turn_index: clock.turn_index,
            },
            actor,
            encounter_active,
        }
    }

    #[must_use]
    pub fn is_current_turn(&self, clock: &CombatState) -> bool {
        match self.phase {
            ActionPhase::RealtimeExplore => true,
            ActionPhase::Turn { turn_index } => clock
                .initiative
                .get(turn_index)
                .copied()
                == Some(self.actor),
        }
    }

    #[must_use]
    pub fn in_turn_session(&self) -> bool {
        matches!(self.phase, ActionPhase::Turn { .. })
    }
}
