use crate::content::{Condition, DemoQuestPhase, Effect, QuestJournalStatus};
use crate::game_content::{dialogue_tree, effects, quests, requires};

dialogue_tree! {
    TREE_GUIDE, "guide", {
        _intro => {
            text: "I'm the village guide. I keep track of who needs help and who can give it.",
            choices: [
                {
                    label: "How can I make myself useful?",
                    next: jobs,
                    requires: requires![],
                    effects: effects![
                        Effect::SetDemoQuest(DemoQuestPhase::TalkedToGuide),
                        Effect::SetQuestStage {
                            quest: quests::QUEST_VILLAGER_HELP,
                            stage: 0
                        },
                        Effect::JournalSetStatus {
                            quest: quests::QUEST_VILLAGER_HELP,
                            status: QuestJournalStatus::InProgress
                        },
                        Effect::JournalAppend {
                            quest: quests::QUEST_VILLAGER_HELP,
                            title_if_new: Some(quests::QUEST_VILLAGER_HELP_TITLE),
                            text: "Rowan asked me to lend a hand around the village."
                        },
                    ],
                },
                {
                    label: "Farewell.",
                    next: EXIT,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        hub => {
            text: "Good to see you again. Need direction, a progress check, or the cellar key?",
            choices: [
                {
                    label: "How can I make myself useful?",
                    next: jobs,
                    requires: requires![],
                    effects: effects![],
                },
                {
                    label: "How are we doing?",
                    next: report,
                    requires: requires![],
                    effects: effects![],
                },
                {
                    label: "I'd like to borrow the cellar key.",
                    next: key_offer,
                    requires: requires![Condition::QuestStatusIs {
                        quest: quests::QUEST_VILLAGER_HELP,
                        status: QuestJournalStatus::Completed
                    }],
                    effects: effects![],
                },
                {
                    label: "I brought the cellar key back.",
                    next: key_returned,
                    requires: requires![
                        Condition::QuestStatusIs {
                            quest: quests::QUEST_GUIDE_FETCH,
                            status: QuestJournalStatus::InProgress
                        },
                        Condition::ItemCountAtLeast {
                            id: "cellar_key",
                            count: 1
                        }
                    ],
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
        jobs => {
            text: "Start with Mara, Aldwin, and Riva. They'll tell you exactly what they need.",
            choices: [
                {
                    label: "Back.",
                    next: hub,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        report => {
            text: "...",
            text_fn: |narrative| {
                let mara_done = narrative.quest_is_completed(quests::QUEST_HEALER_DELIVERY);
                let aldwin_done = narrative.quest_is_completed(quests::QUEST_SCHOLAR_RING);
                let riva_done = narrative.quest_is_completed(quests::QUEST_TAVERN_VISIT);

                let mara = if mara_done {
                    "Mara said you were a big help getting that medicine to her."
                } else {
                    "Mara is still waiting on that medicine."
                };
                let aldwin = if aldwin_done {
                    "Aldwin said you were a big help recovering that ring."
                } else {
                    "Aldwin is still waiting on the ring."
                };
                let riva = if riva_done {
                    "Riva said you handled yourself well at the tavern."
                } else {
                    "Riva is still waiting to hear from you at the tavern."
                };

                let completed_count =
                    usize::from(mara_done) + usize::from(aldwin_done) + usize::from(riva_done);
                let overall = match completed_count {
                    0 => "You've still got a lot of work ahead of you.",
                    1 | 2 => "Keep this up - you're making a real difference.",
                    _ => "You've helped out everyone. You've earned the key.",
                };
                format!("{mara} {aldwin} {riva} {overall}")
            },
            choices: [
                {
                    label: "Back.",
                    next: hub,
                    requires: requires![],
                    effects: effects![],
                }
            ],
        },
        key_offer => {
            text: "You've earned it. Take the cellar key, and bring it back when you're done.",
            choices: [
                {
                    label: "Understood.",
                    next: hub,
                    requires: requires![],
                    effects: effects![
                        Effect::GiveItem("cellar_key"),
                        Effect::SetDemoQuest(DemoQuestPhase::HasCellarKey),
                        Effect::SetQuestStage {
                            quest: quests::QUEST_GUIDE_FETCH,
                            stage: 2
                        },
                        Effect::JournalSetStatus {
                            quest: quests::QUEST_GUIDE_FETCH,
                            status: QuestJournalStatus::InProgress
                        },
                        Effect::JournalAppend {
                            quest: quests::QUEST_GUIDE_FETCH,
                            title_if_new: Some(quests::QUEST_GUIDE_FETCH_TITLE),
                            text: "The guide finally trusted me with the cellar key."
                        },
                    ],
                }
            ],
        },
        key_returned => {
            text: "Perfect. That's exactly why I trust you with village business.",
            choices: [
                {
                    label: "Glad to help.",
                    next: hub,
                    requires: requires![],
                    effects: effects![
                        Effect::TakeItem("cellar_key"),
                        Effect::SetDemoQuest(DemoQuestPhase::ReturnedKey),
                        Effect::SetQuestStage {
                            quest: quests::QUEST_GUIDE_FETCH,
                            stage: 3
                        },
                        Effect::JournalAppend {
                            quest: quests::QUEST_GUIDE_FETCH,
                            title_if_new: Some(quests::QUEST_GUIDE_FETCH_TITLE),
                            text: "Returned the cellar key to the guide after finishing the errand."
                        },
                        Effect::JournalSetStatus {
                            quest: quests::QUEST_GUIDE_FETCH,
                            status: QuestJournalStatus::Completed
                        },
                    ],
                }
            ],
        }
    }
}
