//! Core simulation, rendering buffers, save format, and content for the TUI RPG.

pub mod combat;
pub mod content;
pub mod entity;
pub mod game;
pub mod input;
pub mod level;
pub mod rect;
pub mod render;
pub mod save;
pub mod ui;
pub mod world;

pub use game::Game;
