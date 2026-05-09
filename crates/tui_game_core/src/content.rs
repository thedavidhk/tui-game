//! Narrative content types, [`ContentPack`] container, and validation (game-agnostic).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::entity::ActorStats;
use crate::item::{ItemCatalog, ItemDef};
use crate::level::LevelFile;
use crate::narrative::{NarrativeApplyError, NarrativeState};

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
pub enum DialogueAction {
    StartTrainingSpar,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Disposition {
    Friendly,
    #[default]
    Neutral,
    Hostile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Relation {
    Allied,
    Friendly,
    Neutral,
    Hostile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb24 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb24 {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub const fn to_render_color(self) -> crate::render::Color {
        crate::render::Color::rgb(self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatrolStopDef {
    pub dx: i16,
    pub dy: i16,
    pub wait_ticks: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NpcRoutineDef {
    Idle,
    Roam {
        radius: u16,
        wait_ticks: u16,
    },
    Patrol {
        stops: &'static [PatrolStopDef],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostileTriggerDef {
    PlayerWithinChebyshev {
        range: u16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NpcBehaviorDef {
    pub routine: NpcRoutineDef,
    pub hostile_trigger: Option<HostileTriggerDef>,
}

impl NpcBehaviorDef {
    pub const fn idle() -> Self {
        Self {
            routine: NpcRoutineDef::Idle,
            hostile_trigger: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DialogueChoice {
    pub label: &'static str,
    pub next: usize,
    pub action: Option<DialogueAction>,
    pub requires: &'static [Condition],
    pub requires_fn: Option<DialogueChoiceRequiresFn>,
    pub effects: &'static [Effect],
    pub effects_fn: Option<DialogueChoiceEffectsFn>,
}

pub type DialogueTextFn = fn(&crate::narrative::NarrativeState) -> String;
pub type DialogueChoiceRequiresFn = fn(&crate::narrative::NarrativeState) -> bool;
pub type DialogueChoiceEffectsFn =
    fn(&mut crate::narrative::NarrativeState, &mut Vec<String>) -> Result<(), String>;

#[derive(Clone, Copy, Debug)]
pub struct DialogueNode {
    pub id: &'static str,
    pub text: &'static str,
    pub text_fn: Option<DialogueTextFn>,
    pub effects: &'static [Effect],
    /// When [`choices`](Self::choices) is empty, proceed to this node on continue (Enter, Space, or click).
    pub auto_next: Option<usize>,
    pub choices: &'static [DialogueChoice],
}

#[derive(Clone, Copy, Debug)]
pub struct DialogueTree {
    pub id: &'static str,
    pub nodes: &'static [DialogueNode],
}

impl DialogueTree {
    #[must_use]
    pub fn node_index(&self, node_id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == node_id)
    }
}

pub trait ContentRuntimeHooks: Send + Sync + std::fmt::Debug {
    fn resolve_dialogue_text(&self, node: &DialogueNode, narrative: &NarrativeState) -> String {
        node.text_fn.map_or_else(|| node.text.to_string(), |f| f(narrative))
    }

    fn dialogue_start_node(
        &self,
        _dialogue_id: &str,
        _tree: &'static DialogueTree,
        _narrative: &NarrativeState,
    ) -> usize {
        0
    }

    fn hud_quest_status_lines(&self, _narrative: &NarrativeState) -> Vec<String> {
        Vec::new()
    }

    fn training_spar_epilogue_node(&self, _player_hp: u16, _trainer_hp: u16) -> &'static str {
        "post_spar_even"
    }

    fn on_item_picked(
        &self,
        _item_id: &str,
        _narrative: &mut NarrativeState,
        _log: &mut Vec<String>,
    ) -> Result<(), NarrativeApplyError> {
        Ok(())
    }

    fn on_region_enter(
        &self,
        _region_id: &str,
        _narrative: &mut NarrativeState,
        _log: &mut Vec<String>,
    ) -> Result<(), NarrativeApplyError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoopContentRuntimeHooks;

impl ContentRuntimeHooks for NoopContentRuntimeHooks {}

pub static NOOP_CONTENT_RUNTIME_HOOKS: NoopContentRuntimeHooks = NoopContentRuntimeHooks;

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
    pub default_fg: Rgb24,
    pub default_label: &'static str,
    /// Actor entities participate in exploration/combat simulation and can block movement.
    pub is_actor: bool,
    /// Exploration routine + hostile aggro trigger for actor entities.
    pub behavior: NpcBehaviorDef,
    /// When set, must exist in `ContentPack::dialogues` and is used as `npc_kind` for talk hooks.
    pub dialogue_id: Option<&'static str>,
    /// When set, spawn carries this world pickup (`ItemDef.id`).
    pub world_item: Option<&'static str>,
    /// Entity opens `ItemTransfer` when the player uses an adjacent tile interaction (e.g. exploration LMB).
    pub is_container: bool,
    /// Static social grouping; runtime systems can override relation dynamically.
    pub faction_id: &'static str,
    /// Default relation toward the player before quest/zone modifiers.
    pub disposition_to_player: Disposition,
    pub base_max_hp: u16,
    pub base_strength: u16,
    pub base_agility: u16,
    pub base_speed: u16,
}

#[derive(Clone, Debug)]
pub struct ContentPack {
    pub dialogues: HashMap<&'static str, &'static DialogueTree>,
    pub default_dialogue: &'static DialogueTree,
    pub runtime_hooks: &'static dyn ContentRuntimeHooks,
    pub quest_defs: &'static [QuestDef],
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

    #[must_use]
    pub fn blueprint_stats(&self, kind: &str) -> Option<ActorStats> {
        self.blueprint(kind).map(|bp| {
            ActorStats::from_full(
                bp.base_max_hp,
                bp.base_max_hp,
                bp.base_strength,
                bp.base_agility,
                bp.base_speed,
            )
        })
    }

    fn item_id_known(&self, id: &str) -> bool {
        self.item_catalog().get(id).is_some()
    }

    fn quest_id_known(&self, id: &str) -> bool {
        self.quest_defs.iter().any(|q| q.id == id)
    }

    /// Check that a level only references known tiles and entity [`EntityBlueprint::kind`] values.
    pub fn validate_level(&self, level: &LevelFile) -> Result<(), String> {
        let n_defs = level.tile_defs.len();
        let expected = (level.width as usize) * (level.height as usize);
        for (i, tid) in level.tiles.iter().enumerate() {
            let ti = *tid as usize;
            if ti >= n_defs {
                return Err(format!(
                    "tiles[{i}] references unknown tile id {tid} (only {n_defs} tile_defs)"
                ));
            }
        }
        if !level.props.is_empty() && level.props.len() != expected {
            return Err(format!(
                "props len {} != width*height {expected}",
                level.props.len()
            ));
        }
        for (i, tid) in level.props.iter().enumerate() {
            if *tid == crate::world::EMPTY_PROP_ID {
                continue;
            }
            let ti = *tid as usize;
            if ti >= n_defs {
                return Err(format!(
                    "props[{i}] references unknown tile id {tid} (only {n_defs} tile_defs)"
                ));
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
        if let Some(p) = &level.player_spawn {
            if p.x < 0
                || p.y < 0
                || p.x >= i32::from(level.width)
                || p.y >= i32::from(level.height)
            {
                return Err(format!(
                    "player_spawn ({},{}) is outside map bounds {}×{}",
                    p.x, p.y, level.width, level.height
                ));
            }
            let idx = p.y as usize * level.width as usize + p.x as usize;
            let g_tid = level
                .tiles
                .get(idx)
                .copied()
                .ok_or_else(|| format!("player_spawn tile index {idx} out of range"))?;
            let p_tid = level
                .props
                .get(idx)
                .copied()
                .unwrap_or(crate::world::EMPTY_PROP_ID);
            let g_blocks = level
                .tile_defs
                .iter()
                .find(|d| d.id == g_tid)
                .is_some_and(|d| d.blocks_movement);
            let p_blocks = p_tid != crate::world::EMPTY_PROP_ID
                && level
                    .tile_defs
                    .iter()
                    .find(|d| d.id == p_tid)
                    .is_some_and(|d| d.blocks_movement);
            if g_blocks || p_blocks {
                return Err(format!(
                    "player_spawn ({},{}) is on a blocking tile (ground {g_tid} prop {p_tid})",
                    p.x, p.y
                ));
            }
        }
        if level.tile_defs.is_empty() {
            return Err(
                "tile_defs is empty — load terrain pack (materialize_tile_defs_from_pack) before validate_level".into(),
            );
        }
        let w = i32::from(level.width);
        let h = i32::from(level.height);
        const MAX_ZONE_FALLOFF: u16 = 64;
        for (i, z) in level.atmosphere_zones.iter().enumerate() {
            if z.edge_falloff_tiles > MAX_ZONE_FALLOFF {
                return Err(format!(
                    "atmosphere_zones[{i}].edge_falloff_tiles {} exceeds max {MAX_ZONE_FALLOFF}",
                    z.edge_falloff_tiles
                ));
            }
            use crate::level::AtmosphereShape;
            match z.shape {
                AtmosphereShape::Rectangle {
                    width_tiles,
                    height_tiles,
                } => {
                    if width_tiles == 0 || height_tiles == 0 {
                        return Err(format!(
                            "atmosphere_zones[{i}] rectangle has zero width or height"
                        ));
                    }
                }
                AtmosphereShape::Circle { radius_tiles } => {
                    if radius_tiles == 0 {
                        return Err(format!("atmosphere_zones[{i}] circle has zero radius"));
                    }
                }
            }
            if z.anchor_x < -w || z.anchor_x >= w * 2 || z.anchor_y < -h || z.anchor_y >= h * 2 {
                return Err(format!(
                    "atmosphere_zones[{i}] anchor ({},{}) is unreasonably far from map",
                    z.anchor_x, z.anchor_y
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
            let mut node_ids = HashSet::new();
            for (i, node) in tree.nodes.iter().enumerate() {
                if !node_ids.insert(node.id) {
                    return Err(format!(
                        "dialogue {} has duplicate node id {:?}",
                        tree.id, node.id
                    ));
                }
                if node.choices.is_empty() {
                    if node.auto_next.is_none() {
                        return Err(format!(
                            "dialogue {} node {} has empty choices but no continue_to target",
                            tree.id, i
                        ));
                    }
                } else if node.auto_next.is_some() {
                    return Err(format!(
                        "dialogue {} node {} mixes choices with continue_to (not allowed)",
                        tree.id, i
                    ));
                }
                if let Some(n) = node.auto_next {
                    if n > exit {
                        return Err(format!(
                            "dialogue {} node {} continue_to points to invalid {}",
                            tree.id, i, n
                        ));
                    }
                }
                for eff in node.effects {
                    match *eff {
                        Effect::GiveItem(id) | Effect::TakeItem(id) => {
                            if !self.item_id_known(id) {
                                return Err(format!(
                                    "dialogue {} node {} effect item unknown {id:?}",
                                    tree.id, i
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
                                    "dialogue {} node {} effect quest unknown {quest:?}",
                                    tree.id, i
                                ));
                            }
                        }
                    }
                }
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
        Ok(())
    }
}

#[cfg(test)]
mod validate_tests {
    use std::collections::HashMap;

    use super::{
        Condition, ContentPack, Disposition, DialogueChoice, DialogueNode, DialogueTree, Effect,
        EntityBlueprint, NOOP_CONTENT_RUNTIME_HOOKS, NpcBehaviorDef, QuestDef, Rgb24,
    };
    use crate::item::{ItemCategory, ItemDef};

    static BAD_GIVE_EFF: &[Effect] = &[Effect::GiveItem("not_an_item")];
    static BAD_GIVE: [DialogueChoice; 1] = [DialogueChoice {
        label: "x",
        next: 0,
        action: None,
        requires: &[],
        requires_fn: None,
        effects: BAD_GIVE_EFF,
        effects_fn: None,
    }];
    static BAD_NODE: [DialogueNode; 1] = [DialogueNode {
        id: "bad",
        text: "t",
        text_fn: None,
        effects: &[],
        auto_next: None,
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
            default_dialogue: &BAD_TREE,
            runtime_hooks: &NOOP_CONTENT_RUNTIME_HOOKS,
            quest_defs: &[],
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
            default_fg: Rgb24::new(200, 200, 200),
            default_label: "X",
            is_actor: false,
            behavior: NpcBehaviorDef::idle(),
            dialogue_id: None,
            world_item: Some("nope"),
            is_container: false,
            faction_id: "test",
            disposition_to_player: Disposition::Neutral,
            base_max_hp: 1,
            base_strength: 1,
            base_agility: 1,
            base_speed: 1,
        }];
        static IDS: [ItemDef; 1] = [ItemDef {
            id: "a",
            name: "A",
            description: "d",
            glyph: 'a',
            category: ItemCategory::Mundane,
            weapon: None,
        }];
        let pack = ContentPack {
            dialogues,
            default_dialogue: &BAD_TREE,
            runtime_hooks: &NOOP_CONTENT_RUNTIME_HOOKS,
            quest_defs: &[],
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
            action: None,
            requires: &[],
            requires_fn: None,
            effects: E,
            effects_fn: None,
        }];
        static N: [DialogueNode; 1] = [DialogueNode {
            id: "t",
            text: "t",
            text_fn: None,
            effects: &[],
            auto_next: None,
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
            default_dialogue: &T,
            runtime_hooks: &NOOP_CONTENT_RUNTIME_HOOKS,
            quest_defs: QUESTS,
            entity_blueprints: &[],
            item_defs: &[],
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("effect quest unknown"), "{err}");
    }

    #[test]
    fn validate_rejects_unknown_quest_id_in_dialogue_condition() {
        let mut dialogues = HashMap::new();
        static GOOD_CHOICES: [DialogueChoice; 1] = [DialogueChoice {
            label: "x",
            next: 0,
            action: None,
            requires: &[Condition::QuestStageAtLeast {
                quest: "missing",
                min: 1,
            }],
            requires_fn: None,
            effects: &[],
            effects_fn: None,
        }];
        static GOOD_NODES: [DialogueNode; 1] = [DialogueNode {
            id: "good",
            text: "t",
            text_fn: None,
            effects: &[],
            auto_next: None,
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
        static IDS: [ItemDef; 1] = [ItemDef {
            id: "a",
            name: "A",
            description: "d",
            glyph: 'a',
            category: ItemCategory::Mundane,
            weapon: None,
        }];
        let pack = ContentPack {
            dialogues,
            default_dialogue: &GOOD_TREE,
            runtime_hooks: &NOOP_CONTENT_RUNTIME_HOOKS,
            quest_defs: QUESTS,
            entity_blueprints: &[],
            item_defs: &IDS,
        };
        let err = pack.validate().unwrap_err();
        assert!(err.contains("condition quest unknown"), "{err}");
    }
}
