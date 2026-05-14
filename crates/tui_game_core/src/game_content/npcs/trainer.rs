use crate::content::{DialogueAction, Disposition, EntityBlueprint, NpcBehaviorDef, Rgb24};
use crate::game_content::{dialogue_tree, effects, requires};

pub const BLUEPRINT: EntityBlueprint = EntityBlueprint {
    kind: "trainer",
    display_name: "Trainer",
    description: "Friendly sparring NPC (dialogue \"trainer\").",
    default_glyph: '🨾',
    default_fg: Rgb24::new(240, 227, 227),
    default_label: "Trainer",
    is_actor: true,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: Some("trainer"),
    world_item: None,
    is_container: false,
    faction_id: "town",
    disposition_to_player: Disposition::Friendly,
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
        post_spar_yield => {
            text: "Enough — I yield. You win this round.",
            choices: [],
            continue_to: EXIT,
        },
        post_spar_help_up => {
            text: "Let me help you up. That was solid work.",
            choices: [],
            continue_to: EXIT,
        },
        post_spar_even => {
            text: "Good round. We'll call it there.",
            choices: [],
            continue_to: EXIT,
        },
        hub => {
            text: "A clean spar is the fastest way to learn timing.",
            choices: [
                {
                    label: "Let's spar.",
                    next: EXIT,
                    action: DialogueAction::StartTrainingSpar,
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

#[must_use]
pub fn spar_epilogue_node(player_hp: u16, trainer_hp: u16) -> &'static str {
    if trainer_hp < player_hp {
        "post_spar_yield"
    } else if player_hp < trainer_hp {
        "post_spar_help_up"
    } else {
        "post_spar_even"
    }
}
