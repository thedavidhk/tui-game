use crate::content::{Condition, Effect, QuestJournalStatus};
use crate::game_content::{dialogue_tree, effects, quests, requires};

dialogue_tree! {
    TREE_SCHOLAR, "scholar", {
        hub => {
            text: "(taps a charcoal sketch) An old brass ring was lost near the marker stones outside town.",
            choices: [
                {
                    label: "Tell me about the tale.",
                    next: clue,
                    requires: requires![],
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
                },
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                },
            ],
        },
        clue => {
            text: "You'll know it when you see it - brass that catches lantern light and feels warm in your hand.",
            choices: [
                {
                    label: "I'll listen for rumors.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        done => {
            text: "Remarkable condition. I'll document this before nightfall.",
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
