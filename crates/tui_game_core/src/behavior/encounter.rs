//! Exploration encounter detection (start combat).

use crate::entity::EntityId;

use super::ctx::BehaviorCtx;
use super::threat;

/// First NPC that should start an encounter with the player, as `(player, hostile)`.
#[must_use]
pub fn find_encounter_start(ctx: &BehaviorCtx<'_>) -> Option<(EntityId, EntityId)> {
    let player = ctx.player_id()?;
    for i in 0..ctx.entities.alive.len() {
        if !ctx.entities.alive[i] {
            continue;
        }
        let eid = EntityId(i as u32);
        if eid == player {
            continue;
        }
        if threat::should_start_encounter(ctx, eid) {
            return Some((player, eid));
        }
    }
    None
}
