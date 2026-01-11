//! Core simulation, rendering buffers, save format, and content for the TUI RPG.

pub mod combat;
pub mod content;
pub mod entity;
pub mod game;
pub mod game_content;
pub mod input;
pub mod item;
pub mod level;
pub mod narrative;
pub mod rect;
pub mod render;
pub mod save;
pub mod ui;
pub mod world;

pub use content::{EntityBlueprint, QuestJournalStatus};
pub use game::Game;
pub use item::{EquipSlot, Inventory, ItemCatalog, ItemCategory, ItemDef, ItemStack};
pub use narrative::{JournalEntry, JournalQuestRecord, NarrativeState};
