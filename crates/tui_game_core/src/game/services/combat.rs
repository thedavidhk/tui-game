use crate::combat::CombatState;
use crate::entity::{EntityArena, EntityId, GridPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackTargetOutcome {
    Target(EntityId),
    NoAdjacentTarget,
}

pub fn find_adjacent_target(
    arena: &EntityArena,
    state: &CombatState,
    actor: EntityId,
    actor_pos: GridPos,
) -> AttackTargetOutcome {
    let dirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    for (dx, dy) in dirs {
        for oid in arena.occupants_at(actor_pos.x + dx, actor_pos.y + dy) {
            if oid == actor || !state.contains_actor(oid) {
                continue;
            }
            if arena.stats(oid).is_some_and(|s| s.hp > 0) {
                return AttackTargetOutcome::Target(oid);
            }
        }
    }
    AttackTargetOutcome::NoAdjacentTarget
}
