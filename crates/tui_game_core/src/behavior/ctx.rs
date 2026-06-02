//! Shared read/write slice of the world passed into behavior code (no [`crate::game::Game`]).

use crate::content::{ContentPack, Relation};
use crate::entity::{EntityArena, EntityId};
use crate::world::MapGrid;

use super::relation::{BlueprintRelationResolver, RelationResolver};

/// Inputs required to decide NPC exploration/combat actions.
pub struct BehaviorCtx<'a> {
    pub map: &'a MapGrid,
    pub entities: &'a mut EntityArena,
    pub content: &'a ContentPack,
    pub rng: &'a mut u64,
    pub player: Option<EntityId>,
}

impl<'a> BehaviorCtx<'a> {
    #[must_use]
    pub fn player_id(&self) -> Option<EntityId> {
        self.player
    }

    #[must_use]
    pub fn blueprint_for(&self, actor: EntityId) -> Option<&'static crate::content::EntityBlueprint> {
        let kind = self
            .entities
            .npc_kind
            .get(actor.0 as usize)?
            .as_deref()?;
        self.content.blueprint(kind)
    }

    /// Relation between two entities (reborrows `entities` immutably for lookup).
    #[must_use]
    pub fn relation(&self, a: EntityId, b: EntityId) -> Relation {
        let player = self.player.unwrap_or(EntityId(0));
        let entities = &*self.entities;
        let resolver = BlueprintRelationResolver {
            player,
            content: self.content,
            entities,
        };
        resolver.relation(a, b)
    }

    #[must_use]
    pub fn is_hostile_to_player(&self, other: EntityId) -> bool {
        let Some(player) = self.player_id() else {
            return false;
        };
        matches!(self.relation(player, other), Relation::Hostile)
    }
}
