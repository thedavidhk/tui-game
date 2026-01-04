//! Narrative content types, [`ContentPack`] container, and validation (game-agnostic).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::level::LevelFile;

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentPack {
    pub dialogues: HashMap<&'static str, &'static DialogueTree>,
    pub guide_dialogue: &'static DialogueTree,
    pub entity_blueprints: &'static [EntityBlueprint],
}

impl ContentPack {
    #[must_use]
    pub fn blueprint(&self, kind: &str) -> Option<&'static EntityBlueprint> {
        self.entity_blueprints
            .iter()
            .find(|b| b.kind == kind)
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
        }
        Ok(())
    }
}
