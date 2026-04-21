use crate::content::{Disposition, Effect, EntityBlueprint, QuestJournalStatus, Rgb24};
use crate::game_content::{dialogue_tree, effects, quests, requires};
use crate::narrative::{NarrativeApplyError, NarrativeState};

pub const TAVERN_REGION_ID: &str = "tavern_approach";

pub const BLUEPRINT: EntityBlueprint = EntityBlueprint {
    kind: "merchant",
    display_name: "Merchant",
    description: "Flavor NPC (dialogue \"merchant\").",
    default_glyph: '♟',
    default_fg: Rgb24::new(220, 233, 243),
    default_label: "Merchant",
    dialogue_id: Some("merchant"),
    world_item: None,
    is_container: false,
    faction_id: "town",
    disposition_to_player: Disposition::Neutral,
    base_max_hp: 15,
    base_strength: 4,
    base_agility: 5,
    base_speed: 4,
};

pub fn on_region_enter(
    region_id: &str,
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    if region_id != TAVERN_REGION_ID {
        return Ok(());
    }
    super::try_bump_villager_help_stage(narrative, log)?;
    if !narrative.quest_status_is(quests::QUEST_TAVERN_VISIT, QuestJournalStatus::InProgress) {
        return Ok(());
    }
    narrative.apply_effects(
        log,
        &[
            Effect::SetQuestStage {
                quest: quests::QUEST_TAVERN_VISIT,
                stage: 1,
            },
            Effect::JournalAppend {
                quest: quests::QUEST_TAVERN_VISIT,
                title_if_new: Some(quests::QUEST_TAVERN_VISIT_TITLE),
                text: "I reached the tavern approach and soaked in the noise and smoke.",
            },
            Effect::JournalSetStatus {
                quest: quests::QUEST_TAVERN_VISIT,
                status: QuestJournalStatus::Completed,
            },
        ],
    )
}

dialogue_tree! {
    TREE_MERCHANT, "merchant", {
        _intro => {
            text: "(gestures at bare crates) Hard to say what hour it is in this shop. The caravan's late, so shelves are thin until it arrives.",
            choices: [],
            continue_to: hub,
        },
        hub => {
            text: "No stock to push today, but I still hear what passes through town. Need a pointer, or are you on your way?",
            choices: [
                {
                    label: "Heard anything worth knowing?",
                    next: rumor_start,
                    requires: requires![],
                    requires_fn: |n| n.quest_status(quests::QUEST_TAVERN_VISIT).is_none(),
                    effects: effects![],
                },
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        rumor_start => {
            text: "(leans in, voice dropping) Start at the tavern if you want the latest trouble.",
            effects: effects![
                Effect::SetQuestStage {
                    quest: quests::QUEST_TAVERN_VISIT,
                    stage: 0
                },
                Effect::JournalSetStatus {
                    quest: quests::QUEST_TAVERN_VISIT,
                    status: QuestJournalStatus::InProgress
                },
                Effect::JournalAppend {
                    quest: quests::QUEST_TAVERN_VISIT,
                    title_if_new: Some(quests::QUEST_TAVERN_VISIT_TITLE),
                    text: "Riva suggested the tavern keeps the best rumors in town."
                },
            ],
            choices: [],
            continue_to: EXIT,
        },
    }
}

pub const NPC_MERCHANT: super::NpcSpec = super::NpcSpec {
    blueprint: BLUEPRINT,
    dialogue: &TREE_MERCHANT,
};
