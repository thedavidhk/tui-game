use crate::content::{Condition, Effect, QuestJournalStatus};
use crate::game_content::{dialogue_tree, effects, quests, requires};

dialogue_tree! {
    TREE_HEALER, "healer", {
        hub => {
            text: "(wipes her brow) We're out of medicine. There should be red tonic left in the guild chest.",
            choices: [
                {
                    label: "I'll bring tonic from the guild chest.",
                    next: directions,
                    requires: requires![],
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
                },
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                },
            ],
        },
        directions => {
            text: "East wall, wooden chest. You can't miss it in this little hall.",
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
            text: "Perfect. That'll keep people on their feet. Thank you.",
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
