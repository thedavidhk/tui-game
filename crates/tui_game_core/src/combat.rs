//! Turn-based combat stub (initiative queue, move + end turn).

use serde::{Deserialize, Serialize};

use crate::entity::{EntityArena, EntityId, GridPos};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatAction {
    Move { target: GridPos },
    Pass,
    Flee,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombatState {
    pub initiative: Vec<EntityId>,
    pub turn_index: usize,
    pub grid_w: u16,
    pub grid_h: u16,
}

impl CombatState {
    pub fn from_participants(participants: Vec<EntityId>, w: u16, h: u16) -> Self {
        Self {
            initiative: participants,
            turn_index: 0,
            grid_w: w,
            grid_h: h,
        }
    }

    pub fn current_actor(&self) -> Option<EntityId> {
        self.initiative.get(self.turn_index).copied()
    }

    pub fn end_turn(&mut self) {
        if self.initiative.is_empty() {
            return;
        }
        self.turn_index = (self.turn_index + 1) % self.initiative.len();
    }

    /// Returns true if combat should end (e.g. only one side left — simplified: flee or Pass all).
    pub fn apply_action(
        &mut self,
        action: CombatAction,
        arena: &mut EntityArena,
        map_blocks: impl Fn(i32, i32) -> bool,
    ) -> bool {
        let Some(actor) = self.current_actor() else {
            return true;
        };
        match action {
            CombatAction::Move { target } => {
                if target.x < 0
                    || target.y < 0
                    || target.x >= self.grid_w as i32
                    || target.y >= self.grid_h as i32
                {
                    return false;
                }
                let blocked = map_blocks(target.x, target.y);
                if !arena.can_move_to(blocked, target, Some(actor)) {
                    return false;
                }
                arena.set_pos(actor, target);
                self.end_turn();
                false
            }
            CombatAction::Pass => {
                self.end_turn();
                false
            }
            CombatAction::Flee => true,
        }
    }
}
