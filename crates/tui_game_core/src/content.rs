//! Rust-native narrative content: dialogue trees, quest phases, validation.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NpcId(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuestId(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialogueChoice {
    pub label: &'static str,
    pub next: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogueNode {
    pub text: &'static str,
    pub choices: &'static [DialogueChoice],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogueTree {
    pub id: &'static str,
    pub nodes: &'static [DialogueNode],
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DemoQuestPhase {
    #[default]
    NotStarted,
    TalkedToGuide,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestProgress {
    pub demo: DemoQuestPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentPack {
    pub dialogues: HashMap<&'static str, &'static DialogueTree>,
    pub guide_dialogue: &'static DialogueTree,
}

static DC_GUIDE_0: [DialogueChoice; 2] = [
    DialogueChoice {
        label: "I'll listen.",
        next: 1,
    },
    DialogueChoice {
        label: "Not interested.",
        next: 2,
    },
];
/// `next == tree.nodes.len()` means close dialogue (sentinel).
static DC_GUIDE_1: [DialogueChoice; 1] = [DialogueChoice {
    label: "Farewell.",
    next: 3,
}];
static DC_GUIDE_2: [DialogueChoice; 1] = [DialogueChoice {
    label: "Leave.",
    next: 3,
}];

static NODES_GUIDE: [DialogueNode; 3] = [
    DialogueNode {
        text: "Welcome, traveler. The old cellar key is yours if you listen.",
        choices: &DC_GUIDE_0,
    },
    DialogueNode {
        text: "Good. Remember: the plan is only beginning.",
        choices: &DC_GUIDE_1,
    },
    DialogueNode {
        text: "As you wish.",
        choices: &DC_GUIDE_2,
    },
];

static TREE_GUIDE: DialogueTree = DialogueTree {
    id: "guide",
    nodes: &NODES_GUIDE,
};

impl ContentPack {
    pub fn demo() -> Self {
        let mut dialogues = HashMap::new();
        dialogues.insert("guide", &TREE_GUIDE as &'static DialogueTree);
        Self {
            dialogues,
            guide_dialogue: &TREE_GUIDE,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        for (k, tree) in &self.dialogues {
            if !ids.insert(*k) {
                return Err(format!("duplicate dialogue id: {k}"));
            }
            let exit = tree.nodes.len();
            for (i, node) in tree.nodes.iter().enumerate() {
                for c in node.choices {
                    if c.next > exit {
                        return Err(format!(
                            "dialogue {} node {} choice points to invalid {}",
                            tree.id, i, c.next
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_pack_validates() {
        let p = ContentPack::demo();
        p.validate().unwrap();
    }
}
