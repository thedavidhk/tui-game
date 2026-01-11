use crate::content::{Condition, Effect, QuestJournalStatus, TriggerKind, TriggerRule};
use crate::game_content::{effects, quests, requires};

pub static TRIGGER_RULES: &[TriggerRule] = &[
    TriggerRule {
        id: "healer_quest_started",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "healer",
            node_index: 0,
            choice_label: "I'll bring tonic from the guild chest.",
        },
        requires: requires![],
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
        once: true,
    },
    TriggerRule {
        id: "scholar_quest_started",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "scholar",
            node_index: 0,
            choice_label: "Tell me about the tale.",
        },
        requires: requires![],
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
        once: true,
    },
    TriggerRule {
        id: "villager_help_progress_healer",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "healer",
            node_index: 0,
            choice_label: "Here's the tonic.",
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_VILLAGER_HELP,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![Effect::AddQuestStage {
            quest: quests::QUEST_VILLAGER_HELP,
            delta: 1
        }],
        once: true,
    },
    TriggerRule {
        id: "villager_help_progress_scholar",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "scholar",
            node_index: 0,
            choice_label: "I found this brass ring.",
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_VILLAGER_HELP,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![Effect::AddQuestStage {
            quest: quests::QUEST_VILLAGER_HELP,
            delta: 1
        }],
        once: true,
    },
    TriggerRule {
        id: "merchant_farewell_rumor",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "merchant",
            node_index: 0,
            choice_label: "Farewell.",
        },
        requires: requires![],
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
                text: "Riva suggested the tavern keeps the best rumors in town.",
            },
        ],
        once: true,
    },
    TriggerRule {
        id: "villager_help_progress_tavern",
        when: TriggerKind::RegionEnter {
            region_id: "tavern_approach",
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_VILLAGER_HELP,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![Effect::AddQuestStage {
            quest: quests::QUEST_VILLAGER_HELP,
            delta: 1
        }],
        once: true,
    },
    TriggerRule {
        id: "villager_help_completed_on_report",
        when: TriggerKind::DialogueChoice {
            dialogue_id: "guide",
            node_index: 1,
            choice_label: "How are we doing?",
        },
        requires: requires![
            Condition::QuestStatusIs {
                quest: quests::QUEST_VILLAGER_HELP,
                status: QuestJournalStatus::InProgress
            },
            Condition::QuestStatusIs {
                quest: quests::QUEST_HEALER_DELIVERY,
                status: QuestJournalStatus::Completed
            },
            Condition::QuestStatusIs {
                quest: quests::QUEST_SCHOLAR_RING,
                status: QuestJournalStatus::Completed
            },
            Condition::QuestStatusIs {
                quest: quests::QUEST_TAVERN_VISIT,
                status: QuestJournalStatus::Completed
            },
        ],
        effects: effects![
            Effect::SetQuestStage {
                quest: quests::QUEST_VILLAGER_HELP,
                stage: 3
            },
            Effect::JournalAppend {
                quest: quests::QUEST_VILLAGER_HELP,
                title_if_new: Some(quests::QUEST_VILLAGER_HELP_TITLE),
                text: "Rowan said the village is in better shape thanks to my help."
            },
            Effect::JournalSetStatus {
                quest: quests::QUEST_VILLAGER_HELP,
                status: QuestJournalStatus::Completed
            },
        ],
        once: true,
    },
    TriggerRule {
        id: "healer_tonic_found",
        when: TriggerKind::InventoryCheck {
            item_id: "health_tonic",
            min_count: 1,
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_HEALER_DELIVERY,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![Effect::JournalAppend {
            quest: quests::QUEST_HEALER_DELIVERY,
            title_if_new: Some(quests::QUEST_HEALER_DELIVERY_TITLE),
            text: "I found the red tonic Mara asked for.",
        }],
        once: true,
    },
    TriggerRule {
        id: "scholar_ring_found",
        when: TriggerKind::InventoryCheck {
            item_id: "brass_ring",
            min_count: 1,
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_SCHOLAR_RING,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![Effect::JournalAppend {
            quest: quests::QUEST_SCHOLAR_RING,
            title_if_new: Some(quests::QUEST_SCHOLAR_RING_TITLE),
            text: "I found a brass ring matching Aldwin's description.",
        }],
        once: true,
    },
    TriggerRule {
        id: "tavern_approach_entered",
        when: TriggerKind::RegionEnter {
            region_id: "tavern_approach",
        },
        requires: requires![Condition::QuestStatusIs {
            quest: quests::QUEST_TAVERN_VISIT,
            status: QuestJournalStatus::InProgress
        }],
        effects: effects![
            Effect::SetQuestStage {
                quest: quests::QUEST_TAVERN_VISIT,
                stage: 1
            },
            Effect::JournalAppend {
                quest: quests::QUEST_TAVERN_VISIT,
                title_if_new: Some(quests::QUEST_TAVERN_VISIT_TITLE),
                text: "I reached the tavern approach and soaked in the noise and smoke.",
            },
            Effect::JournalSetStatus {
                quest: quests::QUEST_TAVERN_VISIT,
                status: QuestJournalStatus::Completed
            },
        ],
        once: true,
    },
];
