//! Threat detection for flee and encounter triggers.

use crate::content::{EncounterTriggerDef, Relation};
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::math::chebyshev;

use super::ctx::BehaviorCtx;
use super::relation::{BlueprintRelationResolver, RelationResolver};

/// Nearest non-allied actor within Chebyshev `range` of `actor`, if any.
#[must_use]
pub fn nearest_non_allied_threat(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    range: u16,
) -> Option<(EntityId, GridPos)> {
    let Some(actor_pos) = ctx.entities.pos(actor) else {
        return None;
    };
    let r = i32::from(range);
    let mut best: Option<(EntityId, GridPos, i32)> = None;
    for i in 0..ctx.entities.alive.len() {
        if !ctx.entities.alive[i] {
            continue;
        }
        let other = EntityId(i as u32);
        if other == actor {
            continue;
        }
        if !is_non_allied(ctx, actor, other) {
            continue;
        }
        let Some(op) = ctx.entities.pos(other) else {
            continue;
        };
        let d = chebyshev(actor_pos, op);
        if d > r {
            continue;
        }
        if best.is_none_or(|(_, _, best_d)| d < best_d) {
            best = Some((other, op, d));
        }
    }
    best.map(|(id, pos, _)| (id, pos))
}

#[must_use]
pub fn is_non_allied(ctx: &BehaviorCtx<'_>, actor: EntityId, other: EntityId) -> bool {
    !matches!(ctx.relation(actor, other), Relation::Allied | Relation::Friendly)
}

#[must_use]
pub fn evaluate_encounter_trigger(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    rule: EncounterTriggerDef,
) -> bool {
    let Some(player) = ctx.player_id() else {
        return false;
    };
    if actor == player {
        return false;
    }
    if !ctx.is_hostile_to_player(actor) {
        return false;
    }
    let Some(pp) = ctx.entities.pos(player) else {
        return false;
    };
    let Some(ep) = ctx.entities.pos(actor) else {
        return false;
    };
    match rule {
        EncounterTriggerDef::PlayerWithinChebyshev { range } => {
            chebyshev(pp, ep) <= i32::from(range)
        }
    }
}

/// Whether `actor` should start a combat encounter with the player.
#[must_use]
pub fn should_start_encounter(ctx: &BehaviorCtx<'_>, actor: EntityId) -> bool {
    let Some(bp) = ctx.blueprint_for(actor) else {
        return false;
    };
    let Some(trigger) = bp.behavior.encounter else {
        return false;
    };
    evaluate_encounter_trigger(ctx, actor, trigger)
}

/// Read-only check for combat AI (hostile + in aggro range).
#[must_use]
pub fn is_actively_hostile_to_player_with(
    entities: &EntityArena,
    content: &crate::content::ContentPack,
    player: Option<EntityId>,
    actor: EntityId,
) -> bool {
    let Some(player) = player else {
        return false;
    };
    if !entities.is_alive(actor) {
        return false;
    }
    let resolver = BlueprintRelationResolver {
        player,
        content,
        entities,
    };
    if !matches!(
        resolver.relation(player, actor),
        crate::content::Relation::Hostile
    ) {
        return false;
    }
    let Some(kind) = entities.npc_kind.get(actor.0 as usize).and_then(|k| k.as_deref()) else {
        return false;
    };
    let Some(bp) = content.blueprint(kind) else {
        return false;
    };
    let Some(trigger) = bp.behavior.encounter else {
        return false;
    };
    let Some(pp) = entities.pos(player) else {
        return false;
    };
    let Some(ep) = entities.pos(actor) else {
        return false;
    };
    match trigger {
        EncounterTriggerDef::PlayerWithinChebyshev { range } => {
            chebyshev(pp, ep) <= i32::from(range)
        }
    }
}
