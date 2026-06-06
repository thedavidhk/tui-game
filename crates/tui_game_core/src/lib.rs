//! Core simulation, rendering buffers, save format, and content for the TUI RPG.

pub mod behavior;
pub mod combat;
pub mod content;
pub mod entity;
pub mod game;
pub mod game_content;
pub mod input;
pub mod item;
pub mod level;
pub mod magic;
pub mod math;
pub mod narrative;
pub mod rect;
pub mod render;
pub mod save;
pub(crate) mod step_pacing;
pub mod ui;
pub mod world;

pub use combat::AttackStyle;
pub use content::{EntityBlueprint, QuestJournalStatus};
pub use game::Game;
pub use item::{
    EquipSlot, Inventory, ItemCatalog, ItemCategory, ItemDef, ItemStack, StackEquipped, WeaponKind,
};
pub use narrative::{JournalEntry, JournalQuestRecord, NarrativeState};

#[cfg(test)]
mod architecture_tests;
