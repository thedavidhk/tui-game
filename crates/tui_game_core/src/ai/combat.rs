use crate::ai::{AiIntent, CombatAiCtx, CombatDecisionPolicy};
use crate::combat::CombatAction;
use crate::entity::EntityId;
use crate::world::plan_path;

/// Rudimentary combat AI: attack adjacent target, else move toward nearest one.
pub struct ChaseNearestPolicy;

impl CombatDecisionPolicy for ChaseNearestPolicy {
    fn decide(&self, actor: EntityId, ctx: &CombatAiCtx<'_>) -> AiIntent {
        let Some(actor_pos) = ctx.entities.pos(actor) else {
            return AiIntent::Wait;
        };
        let mut closest: Option<(EntityId, i32)> = None;
        for target in &ctx.state.initiative {
            if *target == actor {
                continue;
            }
            if !ctx.entities.is_alive(*target) || ctx.entities.stats(*target).is_none_or(|s| s.hp == 0) {
                continue;
            }
            let Some(p) = ctx.entities.pos(*target) else {
                continue;
            };
            let d = (actor_pos.x - p.x).abs() + (actor_pos.y - p.y).abs();
            if closest.is_none_or(|(_, best)| d < best) {
                closest = Some((*target, d));
            }
        }
        let Some((target_id, _)) = closest else {
            return AiIntent::Wait;
        };
        let Some(target_pos) = ctx.entities.pos(target_id) else {
            return AiIntent::Wait;
        };
        let dx = (actor_pos.x - target_pos.x).abs();
        let dy = (actor_pos.y - target_pos.y).abs();
        if dx.max(dy) == 1 {
            return AiIntent::Combat(CombatAction::Attack { target: target_id });
        }
        let plan = plan_path(
            ctx.map,
            ctx.entities,
            actor_pos,
            target_pos,
            Some(actor),
            true,
            u32::MAX,
        );
        let Ok(plan) = plan else {
            return AiIntent::Wait;
        };
        let Some(next) = plan.path.get(1).copied() else {
            return AiIntent::Wait;
        };
        AiIntent::Combat(CombatAction::Move { target: next })
    }
}
