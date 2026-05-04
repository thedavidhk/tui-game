//! Keyboard bindings for the game binary.
//!
//! Mode handlers only see [`tui_game_core::game::GameCommand`]; chords are resolved in
//! [`Game::step`](tui_game_core::game::Game) using [`tui_game_core::game::GameKeyMap`] and
//! [`tui_game_core::game::KeyMapLayer`].
//!
//! To customize: define your own `&'static [(KeyChord, GameCommand)]` tables (see
//! [`tui_game_core::game::default_game_key_map`] in the core crate for the default layout) and
//! return [`GameKeyMap::new`](tui_game_core::game::GameKeyMap::new)(exploration, combat, confirm_modal, browse_modal, transfer) from `game_key_map` here.

use tui_game_core::game::{default_game_key_map, GameKeyMap};

#[inline]
#[must_use]
pub fn game_key_map() -> GameKeyMap {
    default_game_key_map()
}
