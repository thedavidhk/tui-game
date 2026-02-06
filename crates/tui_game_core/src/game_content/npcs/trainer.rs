use crate::content::{DialogueAction, EntityBlueprint};
use crate::game_content::{dialogue_tree, effects, requires};

pub const BLUEPRINT: EntityBlueprint = EntityBlueprint {
    kind: "trainer",
    display_name: "Trainer",
    description: "Friendly sparring NPC (dialogue \"trainer\").",
    default_glyph: 't',
    default_label: "Trainer",
    dialogue_id: Some("trainer"),
    world_item: None,
    is_container: false,
    base_max_hp: 22,
    base_strength: 7,
    base_agility: 6,
    base_speed: 7,
};

dialogue_tree! {
    TREE_TRAINER, "trainer", {
        _intro => {
            text: "(adjusts stance) Want to run drills? We can spar without bruised egos.",
            choices: [],
            continue_to: hub,
        },
        hub => {
            text: "A clean spar is the fastest way to learn timing.",
            choices: [
                {
                    label: "Let's spar.",
                    next: EXIT,
                    action: DialogueAction::StartFriendlyTrainingCombat,
                    requires: requires![],
                    effects: effects![],
                },
                {
                    label: "Not right now.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        }
    }
}

pub const NPC_TRAINER: super::NpcSpec = super::NpcSpec {
    blueprint: BLUEPRINT,
    dialogue: &TREE_TRAINER,
};
