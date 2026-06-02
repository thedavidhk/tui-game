//! Input routing for [`super::Game`] (Phase D: isolate mode dispatch from orchestration).

pub(super) mod combat;
pub(super) mod dialogue;
pub(super) mod exploration;
pub(super) mod game_over;
pub(super) mod inventory;
pub(super) mod journal;
pub(super) mod menu;
pub(super) mod spell_targeting;
pub(super) mod transfer;

use super::{Game, GameInput, GameMode};

pub fn route(game: &mut Game, ev: GameInput) {
    match game.modes.current().cloned() {
        None => {}
        Some(GameMode::MainMenu { selected }) => game.handle_menu(ev, selected),
        Some(GameMode::Exploration) => {
            if game.turn_clock.is_some() {
                combat::handle(game, ev);
            } else {
                game.handle_explore(ev);
            }
        }
        Some(GameMode::Dialogue { .. }) => game.handle_dialogue(ev),
        Some(GameMode::Inventory { .. }) => game.handle_inventory(ev),
        Some(GameMode::Journal { .. }) => game.handle_journal(ev),
        Some(GameMode::ItemTransfer { .. }) => game.handle_item_transfer(ev),
        Some(GameMode::Combat(_)) => combat::handle(game, ev),
        Some(GameMode::SpellTargeting { spell, cursor }) => {
            spell_targeting::handle(game, ev, spell, cursor);
        }
        Some(GameMode::GameOver) => game.handle_game_over(ev),
    }
}
