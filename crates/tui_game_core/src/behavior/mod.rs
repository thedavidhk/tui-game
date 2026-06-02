//! NPC behavior: priority reaction stack, shared across exploration and turn-based time.
//!
//! ## Adding behavior
//!
//! 1. Add a [`crate::content::ReactionDef`] variant if needed.
//! 2. Implement it in [`reactions::try_reaction`].
//! 3. Compose a `&'static [ReactionDef]` preset on the blueprint in `game_content/blueprints.rs`.
//!
//! Game code calls only [`decide::decide_actor_action`] via [`crate::game::services::behavior`].
//!
//! ## Time vs encounter
//!
//! [`constraints::ActionConstraints`] carries realtime vs turn pacing. Overworld turn-based uses
//! [`crate::game::Game::turn_clock`]; lethal encounters use [`GameMode::Combat`] with the same
//! decision pipeline.

pub mod action;
pub mod constraints;
pub mod ctx;
pub mod decide;
pub mod encounter;
pub mod events;
pub mod exploration;
pub mod navigation;
pub mod reactions;
pub mod relation;
pub mod threat;

#[cfg(test)]
mod tests;

pub use action::NpcAction;
pub use constraints::{ActionConstraints, ActionPhase};
pub use ctx::BehaviorCtx;
pub use decide::decide_actor_action;
pub use encounter::find_encounter_start;
pub use events::{force_flee, on_actor_damaged, on_combat_hit_target};
pub use exploration::ExplorationIntent;
pub use relation::{BlueprintRelationResolver, RelationResolver};
pub use threat::is_actively_hostile_to_player_with;

/// Map NPC intent to combat action for the game adapter.
#[must_use]
pub fn npc_action_to_combat(action: NpcAction) -> crate::combat::CombatAction {
    match action {
        NpcAction::Step(target) => crate::combat::CombatAction::Move { target },
        NpcAction::Attack { target, style } => {
            crate::combat::CombatAction::Attack { target, style }
        }
        NpcAction::Pass | NpcAction::Idle => crate::combat::CombatAction::Pass,
    }
}
