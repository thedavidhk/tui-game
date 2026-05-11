mod guide;
mod healer;
mod merchant;
mod scholar;
mod trainer;

use std::collections::HashMap;

use crate::content::{DemoQuestPhase, DialogueTree, Effect, EntityBlueprint, QuestJournalStatus};
use crate::narrative::{NarrativeApplyError, NarrativeState};

use super::quests;

pub struct NpcSpec {
    pub blueprint: EntityBlueprint,
    pub dialogue: &'static DialogueTree,
}

pub static NPC_SPECS: &[NpcSpec] = &[
    guide::NPC_GUIDE,
    healer::NPC_HEALER,
    scholar::NPC_SCHOLAR,
    merchant::NPC_MERCHANT,
    trainer::NPC_TRAINER,
];

#[must_use]
pub fn dialogue_map() -> HashMap<&'static str, &'static DialogueTree> {
    let mut dialogues = HashMap::new();
    for npc in NPC_SPECS {
        dialogues.insert(npc.dialogue.id, npc.dialogue);
    }
    dialogues
}

#[must_use]
pub fn default_dialogue() -> &'static DialogueTree {
    &guide::TREE_GUIDE
}

#[must_use]
pub fn npc_blueprints() -> &'static [EntityBlueprint] {
    static BLUEPRINTS: std::sync::OnceLock<Vec<EntityBlueprint>> = std::sync::OnceLock::new();
    BLUEPRINTS.get_or_init(|| NPC_SPECS.iter().map(|npc| npc.blueprint).collect())
}

pub fn on_item_picked(
    item_id: &str,
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    guide::on_item_picked(item_id, narrative, log)?;
    healer::on_item_picked(item_id, narrative, log)?;
    scholar::on_item_picked(item_id, narrative, log)?;
    Ok(())
}

pub fn on_region_enter(
    region_id: &str,
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    merchant::on_region_enter(region_id, narrative, log)
}

#[must_use]
pub fn training_spar_epilogue_node(player_hp: u16, trainer_hp: u16) -> &'static str {
    trainer::spar_epilogue_node(player_hp, trainer_hp)
}

#[must_use]
pub fn dialogue_start_node(
    dialogue_id: &str,
    tree: &'static DialogueTree,
    narrative: &NarrativeState,
) -> usize {
    if dialogue_id == "guide" && narrative.quests != DemoQuestPhase::NotStarted {
        return tree.node_index("hub").unwrap_or(0);
    }
    if narrative.has_seen_dialogue_intro(dialogue_id) {
        if dialogue_id == "guide" {
            return tree
                .node_index("welcome")
                .or_else(|| tree.node_index("hub"))
                .unwrap_or(0);
        }
        return tree.node_index("hub").unwrap_or(0);
    }
    0
}

/// When a villager errand completes while `villager_help` is in progress, advance its stage once.
pub(super) fn try_bump_villager_help_stage(
    narrative: &mut NarrativeState,
    log: &mut Vec<String>,
) -> Result<(), NarrativeApplyError> {
    if !narrative.quest_status_is(quests::QUEST_VILLAGER_HELP, QuestJournalStatus::InProgress) {
        return Ok(());
    }
    narrative.apply_effects(
        log,
        &[Effect::AddQuestStage {
            quest: quests::QUEST_VILLAGER_HELP,
            delta: 1,
        }],
    )
}
