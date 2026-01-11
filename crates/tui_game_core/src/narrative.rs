//! Player narrative state: inventory, equipment, quest progression, and effect application.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::content::{Condition, DemoQuestPhase, Effect};
use crate::item::{EquipSlot, Inventory, InventoryError};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeState {
    /// Demo storyline phase (HUD and simple gating).
    pub quests: DemoQuestPhase,
    /// Arbitrary per-quest numeric counters (e.g. second quest lines later).
    #[serde(default)]
    pub quest_stages: HashMap<String, u32>,
    pub inventory: Inventory,
    pub container_inventories: HashMap<u32, Inventory>,
    pub equipment: HashMap<EquipSlot, String>,
}

impl Default for NarrativeState {
    fn default() -> Self {
        Self {
            quests: DemoQuestPhase::default(),
            quest_stages: HashMap::new(),
            inventory: Inventory::default(),
            container_inventories: HashMap::new(),
            equipment: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NarrativeApplyError {
    MissingItemForTake,
    Inventory(InventoryError),
}

impl NarrativeState {
    /// Returns a user-facing log line if any condition fails.
    pub fn check_requires(&self, requires: &[Condition]) -> Result<(), String> {
        for c in requires {
            match *c {
                Condition::HasItem(id) => {
                    if !self.inventory.has(id, 1) {
                        return Err(format!("You need \"{id}\" for that choice."));
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
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::content::{DemoQuestPhase, Effect};

    use super::NarrativeState;

    #[test]
    fn apply_set_demo_quest() {
        let mut n = NarrativeState::default();
        let mut log = Vec::new();
        n.apply_effects(&mut log, &[Effect::SetDemoQuest(DemoQuestPhase::HasCellarKey)])
            .unwrap();
        assert_eq!(n.quests, DemoQuestPhase::HasCellarKey);
    }
}
