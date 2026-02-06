use crate::entity::{EntityArena, EntityId, GridPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcome {
    Dialogue(EntityId),
    Container(EntityId),
    None,
}

pub fn probe_adjacent(entities: &EntityArena, player_pos: GridPos) -> InteractionOutcome {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = player_pos.x + dx;
            let ny = player_pos.y + dy;
            if let Some(occ) = entities.first_npc_at(nx, ny) {
                return InteractionOutcome::Dialogue(occ);
            }
            if let Some(chest) = entities.first_container_at(nx, ny) {
                return InteractionOutcome::Container(chest);
            }
        }
    }
    InteractionOutcome::None
}
