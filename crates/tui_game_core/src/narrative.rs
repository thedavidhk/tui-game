//! Player narrative state: inventory, equipment, quest progression, and effect application.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::content::{Condition, DemoQuestPhase, Effect, QuestJournalStatus};
use crate::item::{EquipSlot, Inventory, InventoryError, ItemStack, StackEquipped};

/// One timestamped line under a quest in the journal (ordering uses `seq`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub text: String,
    pub seq: u32,
}

/// Journal row for a single quest id (`guide_fetch`, …).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalQuestRecord {
    pub id: String,
    pub title: String,
    pub status: QuestJournalStatus,
    pub entries: Vec<JournalEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NarrativeState {
    /// Demo storyline phase (HUD and simple gating).
    pub quests: DemoQuestPhase,
    /// Arbitrary per-quest numeric counters (e.g. second quest lines later).
    #[serde(default)]
    pub quest_stages: HashMap<String, u32>,
    /// Monotonic counter for journal entry ordering (not wall-clock time).
    #[serde(default)]
    pub journal_next_seq: u32,
    /// Quest journal: one record per quest id, in discovery order.
    #[serde(default)]
    pub quest_journal: Vec<JournalQuestRecord>,
    /// Dialogue ids for which the player has already passed the first auto-continue beat (`_intro` / `_greet`).
    #[serde(default)]
    pub met_npcs: HashSet<String>,
    pub inventory: Inventory,
    pub container_inventories: HashMap<u32, Inventory>,
    /// Legacy only (save schema &lt; 8); migrated into [`Inventory`] stacks with [`StackEquipped`].
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub equipment: HashMap<EquipSlot, String>,
    /// Legacy only; migrated into a quiver [`ItemStack`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipped_ammo: Option<ItemStack>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NarrativeApplyError {
    MissingItemForTake,
    Inventory(InventoryError),
}

impl NarrativeState {
    /// Imports legacy `equipment` / `equipped_ammo` into [`ItemStack::equipped`] rows. Idempotent.
    pub fn migrate_legacy_equipment_into_stacks(&mut self) {
        if self.equipment.is_empty() && self.equipped_ammo.is_none() {
            return;
        }
        let legacy_eq = std::mem::take(&mut self.equipment);
        let legacy_ammo = self.equipped_ammo.take();
        for (slot, item_id) in legacy_eq {
            self.inventory.stacks.push(ItemStack::worn(item_id, slot));
        }
        if let Some(am) = legacy_ammo {
            if am.count > 0 {
                self.inventory
                    .absorb_stack(ItemStack::quiver(am.id, am.count));
            }
        }
    }

    #[must_use]
    pub fn worn_item_id_in_slot(&self, slot: EquipSlot) -> Option<&str> {
        self.inventory.stacks.iter().find_map(|s| {
            if matches!(s.equipped, Some(StackEquipped::Wear(sl)) if sl == slot) {
                Some(s.id.as_str())
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn main_hand_item_id(&self) -> Option<&str> {
        self.worn_item_id_in_slot(EquipSlot::MainHand)
    }

    /// Count of arrows in the quiver stack (any ammo id currently flagged [`StackEquipped::Quiver`]).
    #[must_use]
    pub fn quiver_count_for_ranged(&self, ammo_id: &str) -> u32 {
        self.inventory
            .stacks
            .iter()
            .find(|s| s.id == ammo_id && matches!(s.equipped, Some(StackEquipped::Quiver)))
            .map(|s| s.count)
            .unwrap_or(0)
    }

    /// Toggle **wear** for equippable items: splits a stack when needed; toggle again on the worn
    /// row unequips.
    pub fn equip_wear_stack(&mut self, idx: usize, slot: EquipSlot) {
        let Some(stack) = self.inventory.stacks.get(idx).cloned() else {
            return;
        };
        if matches!(stack.equipped, Some(StackEquipped::Quiver)) {
            return;
        }
        if matches!(stack.equipped, Some(StackEquipped::Wear(s)) if s == slot) {
            self.inventory.stacks[idx].equipped = None;
            self.inventory.consolidate_loose(&stack.id);
            return;
        }
        for (i, s) in self.inventory.stacks.iter_mut().enumerate() {
            if i != idx && matches!(s.equipped, Some(StackEquipped::Wear(sl)) if sl == slot) {
                s.equipped = None;
            }
        }
        if matches!(stack.equipped, Some(StackEquipped::Wear(_))) {
            self.inventory.stacks[idx].equipped = None;
        }
        let Some(stack) = self.inventory.stacks.get(idx).cloned() else {
            return;
        };
        if stack.count == 1 {
            self.inventory.stacks[idx].equipped = Some(StackEquipped::Wear(slot));
            self.inventory.consolidate_loose(&stack.id);
        } else {
            self.inventory.stacks[idx].count -= 1;
            self.inventory
                .stacks
                .push(ItemStack::worn(stack.id.clone(), slot));
            self.inventory.consolidate_loose(&stack.id);
        }
    }

    /// Load / unload ammo quiver: at most one quiver stack; pickups merge into it via [`Inventory::add`].
    pub fn toggle_ammo_quiver(&mut self, idx: usize) {
        let Some(stack) = self.inventory.stacks.get(idx).cloned() else {
            return;
        };
        if matches!(stack.equipped, Some(StackEquipped::Quiver)) {
            self.inventory.stacks[idx].equipped = None;
            self.inventory.consolidate_loose(&stack.id);
            return;
        }
        let id = stack.id.clone();
        for s in self.inventory.stacks.iter_mut() {
            if matches!(s.equipped, Some(StackEquipped::Quiver)) {
                s.equipped = None;
            }
        }
        self.inventory.consolidate_loose(&id);
        if let Some(i) = self
            .inventory
            .stacks
            .iter()
            .position(|s| s.id == id && s.equipped.is_none())
        {
            self.inventory.stacks[i].equipped = Some(StackEquipped::Quiver);
        }
    }

    #[must_use]
    pub fn quest_status(&self, quest_id: &str) -> Option<QuestJournalStatus> {
        self.quest_journal
            .iter()
            .find(|q| q.id == quest_id)
            .map(|q| q.status)
    }

    #[must_use]
    pub fn quest_status_is(&self, quest_id: &str, status: QuestJournalStatus) -> bool {
        self.quest_status(quest_id) == Some(status)
    }

    #[must_use]
    pub fn quest_is_in_progress(&self, quest_id: &str) -> bool {
        self.quest_status_is(quest_id, QuestJournalStatus::InProgress)
    }

    #[must_use]
    pub fn quest_is_completed(&self, quest_id: &str) -> bool {
        self.quest_status_is(quest_id, QuestJournalStatus::Completed)
    }

    #[must_use]
    pub fn quest_is_failed(&self, quest_id: &str) -> bool {
        self.quest_status_is(quest_id, QuestJournalStatus::Failed)
    }

    #[must_use]
    pub fn has_seen_dialogue_intro(&self, dialogue_id: &str) -> bool {
        self.met_npcs.contains(dialogue_id)
    }

    pub fn mark_dialogue_intro_seen(&mut self, dialogue_id: &str) {
        self.met_npcs.insert(dialogue_id.to_string());
    }

    /// Append a journal line for `quest_id`, creating the quest row if needed.
    pub fn journal_append(&mut self, quest_id: &str, title_if_new: Option<&str>, text: &str) {
        let seq = self.journal_next_seq;
        self.journal_next_seq = self.journal_next_seq.saturating_add(1);
        if let Some(q) = self.quest_journal.iter_mut().find(|q| q.id == quest_id) {
            q.entries.push(JournalEntry {
                text: text.to_string(),
                seq,
            });
            return;
        }
        self.quest_journal.push(JournalQuestRecord {
            id: quest_id.to_string(),
            title: title_if_new.unwrap_or(quest_id).to_string(),
            status: QuestJournalStatus::InProgress,
            entries: vec![JournalEntry {
                text: text.to_string(),
                seq,
            }],
        });
    }

    pub fn journal_set_status(&mut self, quest_id: &str, status: QuestJournalStatus) {
        if let Some(q) = self.quest_journal.iter_mut().find(|q| q.id == quest_id) {
            q.status = status;
            return;
        }
        self.quest_journal.push(JournalQuestRecord {
            id: quest_id.to_string(),
            title: quest_id.to_string(),
            status,
            entries: Vec::new(),
        });
    }

    #[must_use]
    pub fn requires_met(&self, requires: &[Condition]) -> bool {
        for c in requires {
            match *c {
                Condition::HasItem(id) => {
                    if !self.inventory.has(id, 1) {
                        return false;
                    }
                }
                Condition::ItemCountAtLeast { id, count } => {
                    if !self.inventory.has(id, count) {
                        return false;
                    }
                }
                Condition::QuestStageAtLeast { quest, min } => {
                    let cur = self.quest_stages.get(quest).copied().unwrap_or(0);
                    if cur < min {
                        return false;
                    }
                }
                Condition::QuestStatusIs { quest, status } => {
                    if self.quest_status(quest) != Some(status) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Returns a user-facing log line if any condition fails.
    pub fn check_requires(&self, requires: &[Condition]) -> Result<(), String> {
        for c in requires {
            match *c {
                Condition::HasItem(id) => {
                    if !self.inventory.has(id, 1) {
                        return Err(format!("You need \"{id}\" for that choice."));
                    }
                }
                Condition::ItemCountAtLeast { id, count } => {
                    if !self.inventory.has(id, count) {
                        return Err(format!("You need {count}x \"{id}\" for that choice."));
                    }
                }
                Condition::QuestStageAtLeast { quest, min } => {
                    let cur = self.quest_stages.get(quest).copied().unwrap_or(0);
                    if cur < min {
                        return Err(format!("Quest \"{quest}\" needs stage >= {min}."));
                    }
                }
                Condition::QuestStatusIs { quest, status } => {
                    if self.quest_status(quest) != Some(status) {
                        return Err(format!("Quest \"{quest}\" status requirement is not met."));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn apply_effects(
        &mut self,
        log: &mut Vec<String>,
        effects: &[Effect],
    ) -> Result<(), NarrativeApplyError> {
        for e in effects {
            match *e {
                Effect::GiveItem(id) => {
                    self.inventory.add(id, 1);
                }
                Effect::TakeItem(id) => {
                    self.inventory.try_remove(id, 1).map_err(|err| {
                        if err == InventoryError::NotEnough {
                            NarrativeApplyError::MissingItemForTake
                        } else {
                            NarrativeApplyError::Inventory(err)
                        }
                    })?;
                }
                Effect::SetDemoQuest(phase) => {
                    self.quests = phase;
                }
                Effect::SetQuestStage { quest, stage } => {
                    self.quest_stages.insert(quest.to_string(), stage);
                }
                Effect::AddQuestStage { quest, delta } => {
                    let e = self.quest_stages.entry(quest.to_string()).or_insert(0);
                    *e = e.saturating_add(delta);
                }
                Effect::Log(msg) => {
                    log.push(msg.to_string());
                }
                Effect::JournalAppend {
                    quest,
                    title_if_new,
                    text,
                } => {
                    self.journal_append(quest, title_if_new, text);
                }
                Effect::JournalSetStatus { quest, status } => {
                    self.journal_set_status(quest, status);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::content::{DemoQuestPhase, Effect, QuestJournalStatus};

    use super::NarrativeState;

    #[test]
    fn apply_set_demo_quest() {
        let mut n = NarrativeState::default();
        let mut log = Vec::new();
        n.apply_effects(
            &mut log,
            &[Effect::SetDemoQuest(DemoQuestPhase::HasCellarKey)],
        )
        .unwrap();
        assert_eq!(n.quests, DemoQuestPhase::HasCellarKey);
    }

    #[test]
    fn journal_append_and_status_via_effects() {
        let mut n = NarrativeState::default();
        let mut log = Vec::new();
        n.apply_effects(
            &mut log,
            &[
                Effect::JournalAppend {
                    quest: "q1",
                    title_if_new: Some("Quest One"),
                    text: "Started.",
                },
                Effect::JournalSetStatus {
                    quest: "q1",
                    status: QuestJournalStatus::Completed,
                },
            ],
        )
        .unwrap();
        assert_eq!(n.quest_journal.len(), 1);
        let q = &n.quest_journal[0];
        assert_eq!(q.title, "Quest One");
        assert_eq!(q.status, QuestJournalStatus::Completed);
        assert_eq!(q.entries.len(), 1);
        assert_eq!(q.entries[0].text, "Started.");
    }

    #[test]
    fn quest_status_transitions_failed_then_completed() {
        let mut n = NarrativeState::default();
        let mut log = Vec::new();
        n.apply_effects(
            &mut log,
            &[
                Effect::JournalSetStatus {
                    quest: "q2",
                    status: QuestJournalStatus::Failed,
                },
                Effect::JournalSetStatus {
                    quest: "q2",
                    status: QuestJournalStatus::Completed,
                },
            ],
        )
        .unwrap();
        assert_eq!(n.quest_status("q2"), Some(QuestJournalStatus::Completed));
    }
}
