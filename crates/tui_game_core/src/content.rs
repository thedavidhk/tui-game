//! Narrative content types, [`ContentPack`] container, and validation (game-agnostic).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::item::{ItemCatalog, ItemDef};
use crate::level::LevelFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NpcId(pub &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuestId(pub &'static str);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DemoQuestPhase {
    #[default]
    NotStarted,
    TalkedToGuide,
    /// Player received the cellar key (inventory or dialogue `give_item`).
    HasCellarKey,
    /// Turned in key to the guide (fetch demo).
    ReturnedKey,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestProgress {
    pub demo: DemoQuestPhase,
}

/// High-level state for a quest row in the player journal (separate from numeric `quest_stages`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestJournalStatus {
    #[default]
    InProgress,
    Failed,
    Completed,
}

/// Static quest metadata used by content definitions and validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuestDef {
    pub id: &'static str,
    pub title: &'static str,
}

/// Gating for a [`DialogueChoice`]; all must hold before effects run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Condition {
    HasItem(&'static str),
    ItemCountAtLeast { id: &'static str, count: u32 },
    QuestStageAtLeast { quest: &'static str, min: u32 },
    QuestStatusIs {
        quest: &'static str,
        status: QuestJournalStatus,
    },
}

/// Narrative / inventory mutations applied after [`Condition`] checks pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    GiveItem(&'static str),
    TakeItem(&'static str),
    SetDemoQuest(DemoQuestPhase),
    SetQuestStage { quest: &'static str, stage: u32 },
    AddQuestStage { quest: &'static str, delta: u32 },
    Log(&'static str),
    /// Append a dated line to the journal for `quest`, creating the row with `title_if_new` if needed.
    JournalAppend {
        quest: &'static str,
        title_if_new: Option<&'static str>,
        text: &'static str,
    },
    JournalSetStatus {
        quest: &'static str,
        status: QuestJournalStatus,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriggerKind {
    DialogueChoice {
        dialogue_id: &'static str,
        node_index: usize,
        choice_label: &'static str,
    },
    InventoryCheck {
        item_id: &'static str,
        min_count: u32,
    },
    RegionEnter {
        region_id: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TriggerRule {
    pub id: &'static str,
    pub when: TriggerKind,
    pub requires: &'static [Condition],
    pub effects: &'static [Effect],
    pub once: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriggerEvent<'a> {
    DialogueChoice {
        dialogue_id: &'a str,
        node_index: usize,
        choice_label: &'a str,
    },
    InventoryCheck {
        item_id: &'a str,
        min_count: u32,
    },
    RegionEnter {
        region_id: &'a str,
    },
}

impl TriggerKind {
    #[must_use]
    pub fn matches(self, ev: TriggerEvent<'_>) -> bool {
        match (self, ev) {
            (
                Self::DialogueChoice {
                    dialogue_id: ad,
                    node_index: an,
                    choice_label: ac,
                },
                TriggerEvent::DialogueChoice {
                    dialogue_id: bd,
                    node_index: bn,
                    choice_label: bc,
                },
            ) => ad == bd && an == bn && ac == bc,
            (
                Self::InventoryCheck {
                    item_id: ai,
                    min_count: am,
                },
                TriggerEvent::InventoryCheck {
                    item_id: bi,
                    min_count: bm,
                },
            ) => ai == bi && bm >= am,
            (Self::RegionEnter { region_id: ar }, TriggerEvent::RegionEnter { region_id: br }) => {
                ar == br
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialogueChoice {
    pub label: &'static str,
    pub next: usize,
    pub requires: &'static [Condition],
    pub effects: &'static [Effect],
}

pub type DialogueTextFn = fn(&crate::narrative::NarrativeState) -> String;

#[derive(Clone, Copy, Debug)]
pub struct DialogueNode {
    pub text: &'static str,
    pub text_fn: Option<DialogueTextFn>,
    pub choices: &'static [DialogueChoice],
}

#[derive(Clone, Copy, Debug)]
pub struct DialogueTree {
    pub id: &'static str,
    pub nodes: &'static [DialogueNode],
}

/// Static definition for an entity type that may appear in [`LevelFile`](crate::level::LevelFile)
/// spawns. Gameplay (dialogue graphs, quests) stays in Rust; levels store `kind` plus optional
/// per-instance `glyph` / `name` overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityBlueprint {
    /// Stable id written to `EntitySpawn.kind` and resolved at load time.
    pub kind: &'static str,
    /// Short label for editors and menus.
    pub display_name: &'static str,
    /// One-line summary for tooling (editor sidebar, docs).
    pub description: &'static str,
    pub default_glyph: char,
    pub default_label: &'static str,
    /// When set, must exist in `ContentPack::dialogues` and is used as `npc_kind` for talk hooks.
    pub dialogue_id: Option<&'static str>,
    /// When set, spawn carries this world pickup (`ItemDef.id`).
    pub world_item: Option<&'static str>,
    /// Entity opens `ItemTransfer` when interacted adjacent.
    pub is_container: bool,
}

#[derive(Clone, Debug)]
pub struct ContentPack {
    pub dialogues: HashMap<&'static str, &'static DialogueTree>,
    pub guide_dialogue: &'static DialogueTree,
    pub quest_defs: &'static [QuestDef],
    pub trigger_rules: &'static [TriggerRule],
    pub entity_blueprints: &'static [EntityBlueprint],
    pub item_defs: &'static [ItemDef],
}

impl ContentPack {
    #[must_use]
    pub fn blueprint(&self, kind: &str) -> Option<&'static EntityBlueprint> {
        self.entity_blueprints.iter().find(|b| b.kind == kind)
    }

    #[must_use]
    pub fn item_catalog(&self) -> ItemCatalog {
        ItemCatalog::new(self.item_defs)
    }

    #[must_use]
    pub fn item_def(&self, id: &str) -> Option<&'static ItemDef> {
        self.item_catalog().get(id)
    }

    fn item_id_known(&self, id: &str) -> bool {
        self.item_catalog().get(id).is_some()
    }

    fn quest_id_known(&self, id: &str) -> bool {
        self.quest_defs.iter().any(|q| q.id == id)
    }

    /// Check that a level only references known tiles and entity [`EntityBlueprint::kind`] values.
    pub fn validate_level(&self, level: &LevelFile) -> Result<(), String> {
        let mut def_ids = HashSet::new();
        for d in &level.tile_defs {
            if !def_ids.insert(d.id) {
                return Err(format!("duplicate tile_def id {}", d.id));
            }
        }
        for (i, tid) in level.tiles.iter().enumerate() {
            if !def_ids.contains(tid) {
                return Err(format!("tiles[{i}] references unknown tile id {tid}"));
            }
        }
        for (i, s) in level.spawns.iter().enumerate() {
            if self.blueprint(s.kind.as_str()).is_none() {
                return Err(format!(
                    "spawns[{i}].kind {:?} is not a known entity blueprint",
                    s.kind
                ));
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut quest_ids = HashSet::new();
        for q in self.quest_defs {
            if !quest_ids.insert(q.id) {
                return Err(format!("duplicate quest_def id: {}", q.id));
            }
        }
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
        let mut bk = HashSet::new();
        for b in self.entity_blueprints {
            if !bk.insert(b.kind) {
                return Err(format!("duplicate entity blueprint kind: {}", b.kind));
            }
            if let Some(did) = b.dialogue_id {
                if !self.dialogues.contains_key(did) {
                    return Err(format!(
                        "entity blueprint {:?} dialogue_id {:?} missing from dialogues",
                        b.kind, did
                    ));
                }
            }
            if let Some(wi) = b.world_item {
                if !self.item_id_known(wi) {
                    return Err(format!(
                        "entity blueprint {:?} world_item {:?} missing from item_defs",
                        b.kind, wi
                    ));
                }
            }
        }
        let mut item_ids = HashSet::new();
        for d in self.item_defs {
            if !item_ids.insert(d.id) {
                return Err(format!("duplicate item_def id {}", d.id));
            }
        }
        for (k, tree) in &self.dialogues {
            for (ni, node) in tree.nodes.iter().enumerate() {
                for (ci, c) in node.choices.iter().enumerate() {
                    for cond in c.requires {
                        match *cond {
                            Condition::HasItem(id) | Condition::ItemCountAtLeast { id, .. } => {
                                if !self.item_id_known(id) {
                                    return Err(format!(
                                        "dialogue {k} node {ni} choice {ci} HasItem unknown {id:?}"
                                    ));
                                }
                            }
                            Condition::QuestStageAtLeast { quest, .. }
                            | Condition::QuestStatusIs { quest, .. } => {
                                if !self.quest_id_known(quest) {
                                    return Err(format!(
                                        "dialogue {k} node {ni} choice {ci} condition quest unknown {quest:?}"
                                    ));
                                }
                            }
                        }
                    }
                    for eff in c.effects {
                        match *eff {
                            Effect::GiveItem(id) | Effect::TakeItem(id) => {
                                if !self.item_id_known(id) {
                                    return Err(format!(
                                        "dialogue {k} node {ni} choice {ci} effect item unknown {id:?}"
                                    ));
                                }
                            }
                            Effect::SetDemoQuest(_)
                            | Effect::Log(_) => {}
                            Effect::SetQuestStage { quest, .. }
                            | Effect::AddQuestStage { quest, .. }
                            | Effect::JournalAppend { quest, .. }
                            | Effect::JournalSetStatus { quest, .. } => {
                                if !self.quest_id_known(quest) {
                                    return Err(format!(
                                        "dialogue {k} node {ni} choice {ci} effect quest unknown {quest:?}"
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut trig_ids = HashSet::new();
        for t in self.trigger_rules {
            if !trig_ids.insert(t.id) {
                return Err(format!("duplicate trigger_rule id: {}", t.id));
            }
            match t.when {
                TriggerKind::DialogueChoice { dialogue_id, .. } => {
                    if !self.dialogues.contains_key(dialogue_id) {
                        return Err(format!(
                            "trigger_rule {:?} references unknown dialogue {:?}",
                            t.id, dialogue_id
                        ));
                    }
                }
                TriggerKind::InventoryCheck { item_id, .. } => {
                    if !self.item_id_known(item_id) {
                        return Err(format!(
                            "trigger_rule {:?} inventory item unknown {:?}",
                            t.id, item_id
                        ));
                    }
                }
                TriggerKind::RegionEnter { .. } => {}
            }
            for cond in t.requires {
                match *cond {
                    Condition::HasItem(id) | Condition::ItemCountAtLeast { id, .. } => {
                        if !self.item_id_known(id) {
                            return Err(format!(
                                "trigger_rule {:?} condition item unknown {:?}",
                                t.id, id
                            ));
                        }
                    }
                    Condition::QuestStageAtLeast { quest, .. }
                    | Condition::QuestStatusIs { quest, .. } => {
                        if !self.quest_id_known(quest) {
                            return Err(format!(
                                "trigger_rule {:?} condition quest unknown {:?}",
                                t.id, quest
                            ));
                        }
                    }
                }
            }
            for eff in t.effects {
                match *eff {
                    Effect::GiveItem(id) | Effect::TakeItem(id) => {
                        if !self.item_id_known(id) {
                            return Err(format!(
                                "trigger_rule {:?} effect item unknown {:?}",
                                t.id, id
                            ));
                        }
                    }
                    Effect::SetDemoQuest(_) | Effect::Log(_) => {}
                    Effect::SetQuestStage { quest, .. }
                    | Effect::AddQuestStage { quest, .. }
                    | Effect::JournalAppend { quest, .. }
                    | Effect::JournalSetStatus { quest, .. } => {
                        if !self.quest_id_known(quest) {
                            return Err(format!(
                                "trigger_rule {:?} effect quest unknown {:?}",
                                t.id, quest
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod validate_tests {
    use std::collections::HashMap;

    use super::{
        Condition, ContentPack, DialogueChoice, DialogueNode, DialogueTree, Effect, EntityBlueprint,
        QuestDef, TriggerKind, TriggerRule,
    };
    use crate::item::{ItemCategory, ItemDef};

    static BAD_GIVE_EFF: &[Effect] = &[Effect::GiveItem("not_an_item")];
    static BAD_GIVE: [DialogueChoice; 1] = [DialogueChoice {
        label: "x",
        next: 0,
        requires: &[],
        effects: BAD_GIVE_EFF,
    }];
    static BAD_NODE: [DialogueNode; 1] = [DialogueNode {
        text: "t",
        text_fn: None,
        choices: &BAD_GIVE,
    }];
    static BAD_TREE: DialogueTree = DialogueTree {
        id: "bad",
        nodes: &BAD_NODE,
    };

    #[test]
    fn validate_rejects_unknown_give_item() {
        let mut dialogues = HashMap::new();
        dialogues.insert("bad", &BAD_TREE);
        let pack = ContentPack {
            dialogues,
            guide_dialogue: &BAD_TREE,
            quest_defs: &[],
            trigger_rules: &[],
            entity_blueprints: &[],
            item_defs: &[],
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("effect item"), "{err}");
    }

    #[test]
    fn validate_rejects_blueprint_world_item_unknown() {
        let mut dialogues = HashMap::new();
        dialogues.insert("bad", &BAD_TREE);
        static BP: [EntityBlueprint; 1] = [EntityBlueprint {
            kind: "x",
            display_name: "X",
            description: "x",
            default_glyph: 'x',
            default_label: "X",
            dialogue_id: None,
            world_item: Some("nope"),
            is_container: false,
        }];
        static IDS: [ItemDef; 1] = [ItemDef {
            id: "a",
            name: "A",
            description: "d",
            glyph: 'a',
            category: ItemCategory::Mundane,
        }];
        let pack = ContentPack {
            dialogues,
            guide_dialogue: &BAD_TREE,
            quest_defs: &[],
            trigger_rules: &[],
            entity_blueprints: &BP,
            item_defs: &IDS,
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("world_item"), "{err}");
    }

    #[test]
    fn validate_rejects_unknown_quest_id_in_effect() {
        let mut dialogues = HashMap::new();
        static E: &[Effect] = &[Effect::SetQuestStage {
            quest: "missing",
            stage: 1,
        }];
        static C: [DialogueChoice; 1] = [DialogueChoice {
            label: "x",
            next: 0,
            requires: &[],
            effects: E,
        }];
        static N: [DialogueNode; 1] = [DialogueNode {
            text: "t",
            text_fn: None,
            choices: &C,
        }];
        static T: DialogueTree = DialogueTree { id: "t", nodes: &N };
        dialogues.insert("t", &T);
        static QUESTS: &[QuestDef] = &[QuestDef {
            id: "known",
            title: "Known",
        }];
        let pack = ContentPack {
            dialogues,
            guide_dialogue: &T,
            quest_defs: QUESTS,
            trigger_rules: &[],
            entity_blueprints: &[],
            item_defs: &[],
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("effect quest unknown"), "{err}");
    }

    #[test]
    fn validate_rejects_unknown_quest_id_in_trigger_rule() {
        let mut dialogues = HashMap::new();
        static GOOD_CHOICES: [DialogueChoice; 1] = [DialogueChoice {
            label: "x",
            next: 0,
            requires: &[],
            effects: &[],
        }];
        static GOOD_NODES: [DialogueNode; 1] = [DialogueNode {
            text: "t",
            text_fn: None,
            choices: &GOOD_CHOICES,
        }];
        static GOOD_TREE: DialogueTree = DialogueTree {
            id: "good",
            nodes: &GOOD_NODES,
        };
        dialogues.insert("good", &GOOD_TREE);
        static QUESTS: &[QuestDef] = &[QuestDef {
            id: "known",
            title: "Known",
        }];
        static RULES: &[TriggerRule] = &[TriggerRule {
            id: "r1",
            when: TriggerKind::InventoryCheck {
                item_id: "a",
                min_count: 1,
            },
            requires: &[Condition::QuestStageAtLeast {
                quest: "missing",
                min: 1,
            }],
            effects: &[],
            once: false,
        }];
        static IDS: [ItemDef; 1] = [ItemDef {
            id: "a",
            name: "A",
            description: "d",
            glyph: 'a',
            category: ItemCategory::Mundane,
        }];
        let pack = ContentPack {
            dialogues,
            guide_dialogue: &GOOD_TREE,
            quest_defs: QUESTS,
            trigger_rules: RULES,
            entity_blueprints: &[],
            item_defs: &IDS,
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("condition quest unknown"), "{err}");
    }
}
