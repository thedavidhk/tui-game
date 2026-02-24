use crate::combat::EncounterProfile;
use crate::entity::{EntityId, GridPos};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionInitiator {
    Player,
    Npc(EntityId),
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionCommand {
    Talk { target: EntityId },
    OpenContainer { target: EntityId },
    EngageCombat {
        target: EntityId,
        profile: EncounterProfile,
    },
    Attack { target: EntityId },
    MoveTo { target: GridPos },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeRequirement {
    Adjacent,
    TalkRadius,
    OccupyTile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRequirement {
    None,
    EntityAlive,
    Container,
    HostileToPlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionRequirements {
    pub range: RangeRequirement,
    pub target: TargetRequirement,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStopReason {
    ReachedRange,
    NoPath,
    Blocked,
    Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionRequest {
    pub initiator: ActionInitiator,
    pub actor: EntityId,
    pub command: ActionCommand,
}

#[must_use]
pub fn requirements_for(command: ActionCommand) -> ActionRequirements {
    match command {
        ActionCommand::Talk { .. } => ActionRequirements {
            range: RangeRequirement::TalkRadius,
            target: TargetRequirement::EntityAlive,
        },
        ActionCommand::OpenContainer { .. } => ActionRequirements {
            range: RangeRequirement::Adjacent,
            target: TargetRequirement::Container,
        },
        ActionCommand::EngageCombat { .. } => ActionRequirements {
            range: RangeRequirement::Adjacent,
            target: TargetRequirement::HostileToPlayer,
        },
        ActionCommand::Attack { .. } => ActionRequirements {
            range: RangeRequirement::Adjacent,
            target: TargetRequirement::EntityAlive,
        },
        ActionCommand::MoveTo { .. } => ActionRequirements {
            range: RangeRequirement::OccupyTile,
            target: TargetRequirement::None,
        },
    }
}
