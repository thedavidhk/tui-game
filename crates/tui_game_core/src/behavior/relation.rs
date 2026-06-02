//! Social relation lookup between entities (faction-ready seam).

use crate::content::{ContentPack, Disposition, Relation};
use crate::entity::{EntityArena, EntityId};

/// Resolves relation between two entities without depending on [`crate::game::Game`].
pub trait RelationResolver {
    fn relation(&self, a: EntityId, b: EntityId) -> Relation;
}

#[must_use]
fn relation_from_disposition(disposition: Disposition) -> Relation {
    match disposition {
        Disposition::Friendly => Relation::Friendly,
        Disposition::Neutral => Relation::Neutral,
        Disposition::Hostile => Relation::Hostile,
    }
}

/// Default resolver: blueprint `disposition_to_player` vs the player; other pairs neutral.
pub struct BlueprintRelationResolver<'a> {
    pub player: EntityId,
    pub content: &'a ContentPack,
    pub entities: &'a EntityArena,
}

impl RelationResolver for BlueprintRelationResolver<'_> {
    fn relation(&self, a: EntityId, b: EntityId) -> Relation {
        if a == b {
            return Relation::Allied;
        }
        if a == self.player {
            return self.relation_to_player(b);
        }
        if b == self.player {
            return self.relation_to_player(a);
        }
        Relation::Neutral
    }
}

impl BlueprintRelationResolver<'_> {
    fn relation_to_player(&self, other: EntityId) -> Relation {
        if other == self.player {
            return Relation::Allied;
        }
        let Some(kind) = self
            .entities
            .npc_kind
            .get(other.0 as usize)
            .and_then(|k| k.as_deref())
        else {
            return Relation::Neutral;
        };
        let Some(bp) = self.content.blueprint(kind) else {
            return Relation::Neutral;
        };
        relation_from_disposition(bp.disposition_to_player)
    }
}

#[must_use]
pub fn is_hostile_to_player(resolver: &dyn RelationResolver, player: EntityId, other: EntityId) -> bool {
    matches!(resolver.relation(player, other), Relation::Hostile)
}
