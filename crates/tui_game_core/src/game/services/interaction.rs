use crate::content::ContentPack;
use crate::entity::{EntityArena, EntityId, GridPos};

use super::hover;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcome {
    Dialogue(EntityId),
    Container(EntityId),
    None,
}

/// Prefer an adjacent chest, then the closest talkable NPC within [`hover::TALK_RANGE_MANHATTAN`].
pub fn probe_interaction(
    entities: &EntityArena,
    player_pos: GridPos,
    content: &ContentPack,
) -> InteractionOutcome {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = player_pos.x + dx;
            let ny = player_pos.y + dy;
            if let Some(chest) = entities.first_container_at(nx, ny) {
                return InteractionOutcome::Container(chest);
            }
        }
    }

    let mut best: Option<(i32, EntityId)> = None;
    for i in 0..entities.alive.len() {
        if !entities.alive[i] {
            continue;
        }
        let eid = EntityId(i as u32);
        let Some(kind) = entities.npc_kind[i].as_deref() else {
            continue;
        };
        let Some(bp) = content.blueprint(kind) else {
            continue;
        };
        if bp.dialogue_id.is_none() {
            continue;
        }
        let Some(pos) = entities.pos(eid) else {
            continue;
        };
        let d = hover::manhattan(player_pos, pos);
        if d > hover::TALK_RANGE_MANHATTAN {
            continue;
        }
        match best {
            None => best = Some((d, eid)),
            Some((bd, _)) if d < bd => best = Some((d, eid)),
            _ => {}
        }
    }
    best.map(|(_, id)| InteractionOutcome::Dialogue(id))
        .unwrap_or(InteractionOutcome::None)
}
