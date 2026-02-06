//! Input routing for [`super::Game`] (Phase D: isolate mode dispatch from orchestration).

pub(super) mod combat;
pub(super) mod dialogue;
pub(super) mod exploration;
pub(super) mod inventory;
pub(super) mod journal;
pub(super) mod menu;
pub(super) mod transfer;

use crate::input::InputEvent;

use super::{Game, GameMode};

pub fn route(game: &mut Game, ev: InputEvent) {
    match game.modes.current().cloned() {
        None => {}
        Some(GameMode::MainMenu { selected }) => game.handle_menu(ev, selected),
        Some(GameMode::Exploration) => game.handle_explore(ev),
        Some(GameMode::Dialogue { .. }) => game.handle_dialogue(ev),
        Some(GameMode::Inventory { .. }) => game.handle_inventory(ev),
        Some(GameMode::Journal { .. }) => game.handle_journal(ev),
        Some(GameMode::ItemTransfer { .. }) => game.handle_item_transfer(ev),
        Some(GameMode::Combat(ref c)) => game.handle_combat(ev, c.clone()),
    }
}
