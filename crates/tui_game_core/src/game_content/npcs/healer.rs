use crate::content::{
    Condition, Disposition, Effect, EntityBlueprint, NpcBehaviorDef, QuestJournalStatus, Rgb24,
};
use crate::game_content::{dialogue_tree, effects, glyphs, quests, requires};
use crate::narrative::{NarrativeApplyError, NarrativeState};

pub const BLUEPRINT: EntityBlueprint = EntityBlueprint {
    kind: "healer",
    display_name: "Healer",
    description: "Simple tonic fetch quest (dialogue \"healer\").",
    default_glyph: glyphs::HUMANOID_FRIENDLY,
    default_fg: Rgb24::new(220, 243, 230),
    default_label: "Healer",
    is_actor: true,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: Some("healer"),
    world_item: None,
    is_container: false,
    faction_id: "town",
    disposition_to_player: Disposition::Friendly,
    base_max_hp: 14,
    base_strength: 4,
    base_agility: 5,
    base_speed: 4,
};

pub fn on_item_picked(
    item_id: &str,
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    if item_id != "health_tonic" {
        return Ok(());
    }
    if !narrative.quest_status_is(
        quests::QUEST_HEALER_DELIVERY,
        QuestJournalStatus::InProgress,
    ) {
        return Ok(());
    }
    narrative.apply_effects(
        log,
        &[Effect::JournalAppend {
            quest: quests::QUEST_HEALER_DELIVERY,
            title_if_new: Some(quests::QUEST_HEALER_DELIVERY_TITLE),
            text: "I found the red tonic Mara asked for.",
        }],
    )
}

dialogue_tree! {
    TREE_HEALER, "healer", {
        _intro => {
            text: "(wiping mortar dust from her wrists) Sit if you like—the benches are dry. We've gone through every soothing draft I had on hand.",
            choices: [],
            continue_to: hub,
        },
        hub => {
            text: "",
            text_fn: |n| {
                if n.quest_is_completed(quests::QUEST_HEALER_DELIVERY) {
                    "You already pulled us out of the worst of it—thanks again. If you need salves later, you know where to find me.".to_string()
                } else if n.quest_is_in_progress(quests::QUEST_HEALER_DELIVERY) {
                    "Still after that red tonic from the guild chest? I'll be here when you bring it.".to_string()
                } else {
                    "We're short on soothing drafts again. There's usually red tonic in the guild chest unless someone's cleared it out.".to_string()
                }
            },
            choices: [
                {
                    label: "I'll bring tonic from the guild chest.",
                    next: quest_start,
                    requires: requires![],
                    requires_fn: |n| n.quest_status(quests::QUEST_HEALER_DELIVERY).is_none(),
                    effects: effects![],
                },
                {
                    label: "Here's the tonic.",
                    next: done,
                    requires: requires![
                        Condition::QuestStatusIs {
                            quest: quests::QUEST_HEALER_DELIVERY,
                            status: QuestJournalStatus::InProgress
                        },
                        Condition::ItemCountAtLeast {
                            id: "health_tonic",
                            count: 1
                        },
                    ],
                    effects: effects![
                        Effect::TakeItem("health_tonic"),
                        Effect::SetQuestStage {
                            quest: quests::QUEST_HEALER_DELIVERY,
                            stage: 2
                        },
                        Effect::JournalAppend {
                            quest: quests::QUEST_HEALER_DELIVERY,
                            title_if_new: Some(quests::QUEST_HEALER_DELIVERY_TITLE),
                            text: "Delivered the tonic. Mara finally looked like she could breathe again."
                        },
                        Effect::JournalSetStatus {
                            quest: quests::QUEST_HEALER_DELIVERY,
                            status: QuestJournalStatus::Completed
                        },
                    ],
                    effects_fn: |narrative, log| {
                        super::try_bump_villager_help_stage(narrative, log)
                            .map_err(|e| format!("{e:?}"))
                    },
                },
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                },
            ],
        },
        quest_start => {
            text: "Good. Bring it quick; the whole hall sounds like one long cough.",
            effects: effects![
                Effect::SetQuestStage {
                    quest: quests::QUEST_HEALER_DELIVERY,
                    stage: 1
                },
                Effect::JournalSetStatus {
                    quest: quests::QUEST_HEALER_DELIVERY,
                    status: QuestJournalStatus::InProgress
                },
                Effect::JournalAppend {
                    quest: quests::QUEST_HEALER_DELIVERY,
                    title_if_new: Some(quests::QUEST_HEALER_DELIVERY_TITLE),
                    text: "Mara needs a bottle of red tonic from the guild chest."
                },
            ],
            choices: [],
            continue_to: directions,
        },
        directions => {
            text: "Straight through when you hear cups clinking—the chest sits under the peg-board of ribbons.",
            choices: [
                {
                    label: "I'll check the chest.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        done => {
            text: "Perfect—color's coming back already. Thank you.",
            choices: [
                {
                    label: "Glad it helped.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        }
    }
}

pub const NPC_HEALER: super::NpcSpec = super::NpcSpec {
    blueprint: BLUEPRINT,
    dialogue: &TREE_HEALER,
};
