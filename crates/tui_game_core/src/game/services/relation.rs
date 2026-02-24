use crate::content::{Disposition, Relation};
use crate::entity::EntityId;
use crate::game::Game;

fn relation_from_disposition(disposition: Disposition) -> Relation {
    match disposition {
        Disposition::Friendly => Relation::Friendly,
        Disposition::Neutral => Relation::Neutral,
        Disposition::Hostile => Relation::Hostile,
    }
}

/// Runtime relation lookup between the player and `other`.
///
/// For now this resolves from blueprint defaults; future quest/zone modifiers
/// should be layered here so call sites stay branch-free.
pub fn relation_to_player(game: &Game, other: EntityId) -> Relation {
    let Some(pid) = game.player_id() else {
        return Relation::Neutral;
    };
    if other == pid {
        return Relation::Allied;
    }
    let Some(kind) = game
        .entities
        .npc_kind
        .get(other.0 as usize)
        .and_then(|k| k.as_deref())
    else {
        return Relation::Neutral;
    };
    let Some(bp) = game.content.blueprint(kind) else {
        return Relation::Neutral;
    };
    relation_from_disposition(bp.disposition_to_player)
}

#[must_use]
pub fn is_hostile_to_player(game: &Game, other: EntityId) -> bool {
    matches!(relation_to_player(game, other), Relation::Hostile)
}
