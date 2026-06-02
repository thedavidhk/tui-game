//! Semantic player commands and key → command resolution.
//!
//! Raw [`crate::input::KeyChord`] values are mapped in one place ([`default_game_key_map`]) so
//! mode handlers only match on [`GameCommand`]. [`KeyMapLayer`] picks the table so the same key
//! can differ by context (e.g. **Enter** passes a combat turn but does nothing in inventory).

use crate::input::{InputEvent, Key, KeyChord};

use super::GameMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameCommand {
    /// Close dialogue, inventory, journal, transfer; leave combat; **main menu**: exit game.
    Back,
    /// Confirm menu item, dialogue choice / continue, transfer stack move, game over → menu, combat pass.
    Confirm,
    ListPrev,
    ListNext,
    TransferFocusSide,
    InventoryUse,
    InventoryEquip,
    ToggleDebug,
    QuickSave,
    QuickLoad,
    OpenInventory,
    OpenJournal,
    StepNorth,
    StepSouth,
    StepWest,
    StepEast,
    CombatFlee,
    ToggleTurnBased,
    /// Enter fireball targeting mode (usable in both exploration and combat).
    CastFireball,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyMapLayer {
    /// Walking the overworld (no combat pass / flee / menu-dismiss keys).
    Exploration,
    /// Combat grid + pass, flee, and journal shortcuts.
    Combat,
    /// Main menu, dialogue, game over — list navigation plus **Enter** / **Space** to confirm.
    ConfirmModal,
    /// Inventory and journal — browse only (**Enter** / **Space** intentionally unbound).
    BrowseModal,
    /// Chest transfer: browse keys plus **Tab** and **Enter** / **Space** to move stacks.
    ItemTransfer,
}

#[derive(Clone, Copy, Debug)]
pub struct GameKeyMap {
    exploration: &'static [(KeyChord, GameCommand)],
    combat: &'static [(KeyChord, GameCommand)],
    confirm_modal: &'static [(KeyChord, GameCommand)],
    browse_modal: &'static [(KeyChord, GameCommand)],
    transfer: &'static [(KeyChord, GameCommand)],
}

impl GameKeyMap {
    #[must_use]
    pub const fn new(
        exploration: &'static [(KeyChord, GameCommand)],
        combat: &'static [(KeyChord, GameCommand)],
        confirm_modal: &'static [(KeyChord, GameCommand)],
        browse_modal: &'static [(KeyChord, GameCommand)],
        transfer: &'static [(KeyChord, GameCommand)],
    ) -> Self {
        Self {
            exploration,
            combat,
            confirm_modal,
            browse_modal,
            transfer,
        }
    }

    #[must_use]
    pub fn resolve(&self, layer: KeyMapLayer, chord: KeyChord) -> Option<GameCommand> {
        let table = match layer {
            KeyMapLayer::Exploration => self.exploration,
            KeyMapLayer::Combat => self.combat,
            KeyMapLayer::ConfirmModal => self.confirm_modal,
            KeyMapLayer::BrowseModal => self.browse_modal,
            KeyMapLayer::ItemTransfer => self.transfer,
        };
        table.iter().find(|(k, _)| *k == chord).map(|(_, cmd)| *cmd)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameInput {
    Command(GameCommand),
    /// Mouse and any future non-key device events the shell forwards unchanged.
    Raw(InputEvent),
}

#[must_use]
pub fn key_layer_for_game(game: &crate::game::Game) -> KeyMapLayer {
    if game.turn_clock.is_some() && matches!(game.modes.current(), Some(crate::game::GameMode::Exploration))
    {
        return KeyMapLayer::Combat;
    }
    key_layer_for_mode(game.modes.current())
}

#[must_use]
pub fn key_layer_for_mode(mode: Option<&GameMode>) -> KeyMapLayer {
    match mode {
        None => KeyMapLayer::ConfirmModal,
        Some(GameMode::Exploration) => KeyMapLayer::Exploration,
        Some(GameMode::Combat(_)) => KeyMapLayer::Combat,
        // Targeting reuses exploration navigation (WASD / arrows + Enter/Esc).
        Some(GameMode::SpellTargeting { .. }) => KeyMapLayer::Exploration,
        Some(GameMode::ItemTransfer { .. }) => KeyMapLayer::ItemTransfer,
        Some(GameMode::MainMenu { .. })
        | Some(GameMode::Dialogue { .. })
        | Some(GameMode::GameOver) => KeyMapLayer::ConfirmModal,
        Some(GameMode::Inventory { .. }) | Some(GameMode::Journal { .. }) => {
            KeyMapLayer::BrowseModal
        }
    }
}

const fn chord(key: Key) -> KeyChord {
    KeyChord {
        key,
        ctrl: false,
        alt: false,
        shift: false,
    }
}

/// Default bindings shipped with the library; the game binary should pass this (or a custom
/// [`GameKeyMap`]) when constructing [`super::Game`].
#[must_use]
pub fn default_game_key_map() -> GameKeyMap {
    static EXPLORATION: &[(KeyChord, GameCommand)] = &[
        (chord(Key::F(1)), GameCommand::ToggleDebug),
        (chord(Key::F(5)), GameCommand::QuickSave),
        (chord(Key::F(9)), GameCommand::QuickLoad),
        (chord(Key::Char('i')), GameCommand::OpenInventory),
        (chord(Key::Char('j')), GameCommand::OpenJournal),
        (chord(Key::Char('w')), GameCommand::StepNorth),
        (chord(Key::Up), GameCommand::StepNorth),
        (chord(Key::Char('s')), GameCommand::StepSouth),
        (chord(Key::Down), GameCommand::StepSouth),
        (chord(Key::Char('a')), GameCommand::StepWest),
        (chord(Key::Left), GameCommand::StepWest),
        (chord(Key::Char('d')), GameCommand::StepEast),
        (chord(Key::Right), GameCommand::StepEast),
        (chord(Key::Char('t')), GameCommand::ToggleTurnBased),
        (chord(Key::Char('z')), GameCommand::CastFireball),
    ];
    static COMBAT: &[(KeyChord, GameCommand)] = &[
        (chord(Key::F(1)), GameCommand::ToggleDebug),
        (chord(Key::F(5)), GameCommand::QuickSave),
        (chord(Key::F(9)), GameCommand::QuickLoad),
        (chord(Key::Char('i')), GameCommand::OpenInventory),
        (chord(Key::Char('j')), GameCommand::OpenJournal),
        (chord(Key::Char('w')), GameCommand::StepNorth),
        (chord(Key::Up), GameCommand::StepNorth),
        (chord(Key::Char('s')), GameCommand::StepSouth),
        (chord(Key::Down), GameCommand::StepSouth),
        (chord(Key::Char('a')), GameCommand::StepWest),
        (chord(Key::Left), GameCommand::StepWest),
        (chord(Key::Char('d')), GameCommand::StepEast),
        (chord(Key::Right), GameCommand::StepEast),
        (chord(Key::Enter), GameCommand::Confirm),
        (chord(Key::Char(' ')), GameCommand::Confirm),
        (chord(Key::Esc), GameCommand::Back),
        (chord(Key::Char('q')), GameCommand::Back),
        (chord(Key::Char('f')), GameCommand::CombatFlee),
        (chord(Key::Char('t')), GameCommand::ToggleTurnBased),
        (chord(Key::Char('z')), GameCommand::CastFireball),
    ];
    static CONFIRM_MODAL: &[(KeyChord, GameCommand)] = &[
        (chord(Key::F(1)), GameCommand::ToggleDebug),
        (chord(Key::Esc), GameCommand::Back),
        (chord(Key::Char('q')), GameCommand::Back),
        (chord(Key::Enter), GameCommand::Confirm),
        (chord(Key::Char(' ')), GameCommand::Confirm),
        (chord(Key::Up), GameCommand::ListPrev),
        (chord(Key::Down), GameCommand::ListNext),
        (chord(Key::PageUp), GameCommand::ListPrev),
        (chord(Key::PageDown), GameCommand::ListNext),
    ];
    static BROWSE_MODAL: &[(KeyChord, GameCommand)] = &[
        (chord(Key::F(1)), GameCommand::ToggleDebug),
        (chord(Key::Esc), GameCommand::Back),
        (chord(Key::Char('q')), GameCommand::Back),
        (chord(Key::Up), GameCommand::ListPrev),
        (chord(Key::Down), GameCommand::ListNext),
        (chord(Key::PageUp), GameCommand::ListPrev),
        (chord(Key::PageDown), GameCommand::ListNext),
        (chord(Key::Char('u')), GameCommand::InventoryUse),
        (chord(Key::Char('e')), GameCommand::InventoryEquip),
    ];
    static TRANSFER: &[(KeyChord, GameCommand)] = &[
        (chord(Key::F(1)), GameCommand::ToggleDebug),
        (chord(Key::Esc), GameCommand::Back),
        (chord(Key::Char('q')), GameCommand::Back),
        (chord(Key::Enter), GameCommand::Confirm),
        (chord(Key::Char(' ')), GameCommand::Confirm),
        (chord(Key::Up), GameCommand::ListPrev),
        (chord(Key::Down), GameCommand::ListNext),
        (chord(Key::PageUp), GameCommand::ListPrev),
        (chord(Key::PageDown), GameCommand::ListNext),
        (chord(Key::Tab), GameCommand::TransferFocusSide),
        (chord(Key::Char('u')), GameCommand::InventoryUse),
        (chord(Key::Char('e')), GameCommand::InventoryEquip),
    ];
    GameKeyMap::new(EXPLORATION, COMBAT, CONFIRM_MODAL, BROWSE_MODAL, TRANSFER)
}
