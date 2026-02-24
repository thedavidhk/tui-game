use crate::content::{Condition, Effect, EntityBlueprint, QuestJournalStatus};
use crate::game_content::{dialogue_tree, effects, quests, requires};
use crate::narrative::{NarrativeApplyError, NarrativeState};

pub const BLUEPRINT: EntityBlueprint = EntityBlueprint {
    kind: "scholar",
    display_name: "Scholar",
    description: "Ring hand-in quest (dialogue \"scholar\").",
    default_glyph: 's',
    default_label: "Scholar",
    dialogue_id: Some("scholar"),
    world_item: None,
    is_container: false,
    base_max_hp: 14,
    base_strength: 4,
    base_agility: 6,
    base_speed: 5,
    hostile: false,
};

pub fn on_item_picked(
    item_id: &str,
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    if item_id != "brass_ring" {
        return Ok(());
    }
    if !narrative.quest_status_is(quests::QUEST_SCHOLAR_RING, QuestJournalStatus::InProgress) {
        return Ok(());
    }
    narrative.apply_effects(
        log,
        &[Effect::JournalAppend {
            quest: quests::QUEST_SCHOLAR_RING,
            title_if_new: Some(quests::QUEST_SCHOLAR_RING_TITLE),
            text: "I found a brass ring matching Aldwin's description.",
        }],
    )
}

dialogue_tree! {
    TREE_SCHOLAR, "scholar", {
        _intro => {
            text: "(looks up from a ribbon-marked parchment) Oh—hello. I was lost in notes; I didn't hear you come in.",
            choices: [],
            continue_to: hub,
        },
        hub => {
            text: "",
            text_fn: |n| {
                if n.quest_is_completed(quests::QUEST_SCHOLAR_RING) {
                    "The brass ring is catalogued and safe—thank you for bringing it back.".to_string()
                } else if n.quest_is_in_progress(quests::QUEST_SCHOLAR_RING) {
                    "Any luck near the marker stones? I'm still after that brass ring.".to_string()
                } else {
                    "I'm short one artifact for my archive: a brass ring, last seen out by the old marker stones. I could use a careful pair of hands.".to_string()
                }
            },
            choices: [
                {
                    label: "I'll help look for the ring.",
                    next: quest_start,
                    requires: requires![],
                    requires_fn: |n| n.quest_status(quests::QUEST_SCHOLAR_RING).is_none(),
                    effects: effects![],
                },
                {
                    label: "I found this brass ring.",
                    next: done,
                    requires: requires![
                        Condition::QuestStatusIs {
                            quest: quests::QUEST_SCHOLAR_RING,
                            status: QuestJournalStatus::InProgress
                        },
                        Condition::ItemCountAtLeast {
                            id: "brass_ring",
                            count: 1
                        },
                    ],
                    effects: effects![
                        Effect::TakeItem("brass_ring"),
                        Effect::SetQuestStage {
                            quest: quests::QUEST_SCHOLAR_RING,
                            stage: 3
                        },
                        Effect::JournalAppend {
                            quest: quests::QUEST_SCHOLAR_RING,
                            title_if_new: Some(quests::QUEST_SCHOLAR_RING_TITLE),
                            text: "Returned the ring to Aldwin. They immediately started planning how to catalogue it."
                        },
                        Effect::JournalSetStatus {
                            quest: quests::QUEST_SCHOLAR_RING,
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
            text: "If you find it, you'll save me weeks of guesswork—and the village keeps a bit of its history intact.",
            effects: effects![
                Effect::SetQuestStage {
                    quest: quests::QUEST_SCHOLAR_RING,
                    stage: 1
                },
                Effect::JournalSetStatus {
                    quest: quests::QUEST_SCHOLAR_RING,
                    status: QuestJournalStatus::InProgress
                },
                Effect::JournalAppend {
                    quest: quests::QUEST_SCHOLAR_RING,
                    title_if_new: Some(quests::QUEST_SCHOLAR_RING_TITLE),
                    text: "Aldwin asked me to recover a brass ring from near the old marker stones."
                },
            ],
            choices: [],
            continue_to: clue,
        },
        clue => {
            text: "Brass, worn smooth—search around the marker stones where the path splits. It picks up lamplight and feels slightly warm if you've got the right one.",
            choices: [
                {
                    label: "I'll head that way.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        done => {
            text: "Remarkable condition. Ink tonight, catalog tomorrow—thank you.",
            choices: [
                {
                    label: "Study it well.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        }
    }
}

pub const NPC_SCHOLAR: super::NpcSpec = super::NpcSpec {
    blueprint: BLUEPRINT,
    dialogue: &TREE_SCHOLAR,
};
