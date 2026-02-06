use crate::entity::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplorationIntent {
    Idle,
    Roam,
    Follow(EntityId),
}
