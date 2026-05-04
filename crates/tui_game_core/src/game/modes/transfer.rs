use crate::game::{Game, GameCommand, GameInput, GameMode, TransferFocus};
use crate::item::Inventory;

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    match ev {
        GameInput::Command(GameCommand::Back) => {
            let _ = game.modes.pop();
        }
        GameInput::Command(GameCommand::TransferFocusSide) => {
            if let Some(GameMode::ItemTransfer { focus, .. }) = game.modes.current_mut() {
                *focus = match *focus {
                    TransferFocus::Player => TransferFocus::Container,
                    TransferFocus::Container => TransferFocus::Player,
                };
            }
        }
        GameInput::Command(GameCommand::ListPrev) => {
            if let Some(GameMode::ItemTransfer {
                focus,
                cursor_player,
                cursor_container,
                container,
            }) = game.modes.current_mut()
            {
                let pn = game.narrative.inventory.stacks.len();
                let cn = game
                    .narrative
                    .container_inventories
                    .entry(container.0)
                    .or_default()
                    .stacks
                    .len();
                match focus {
                    TransferFocus::Player => {
                        *cursor_player = cursor_player.saturating_sub(1).min(pn.saturating_sub(1));
                    }
                    TransferFocus::Container => {
                        *cursor_container =
                            cursor_container.saturating_sub(1).min(cn.saturating_sub(1));
                    }
                }
            }
        }
        GameInput::Command(GameCommand::ListNext) => {
            if let Some(GameMode::ItemTransfer {
                focus,
                cursor_player,
                cursor_container,
                container,
            }) = game.modes.current_mut()
            {
                let pn = game.narrative.inventory.stacks.len();
                let cn = game
                    .narrative
                    .container_inventories
                    .entry(container.0)
                    .or_default()
                    .stacks
                    .len();
                match focus {
                    TransferFocus::Player => {
                        let max = pn.saturating_sub(1);
                        *cursor_player = (*cursor_player + 1).min(max);
                    }
                    TransferFocus::Container => {
                        let max = cn.saturating_sub(1);
                        *cursor_container = (*cursor_container + 1).min(max);
                    }
                }
            }
        }
        GameInput::Command(GameCommand::Confirm) => {
            let Some(GameMode::ItemTransfer {
                container,
                focus,
                cursor_player,
                cursor_container,
            }) = game.modes.current().cloned()
            else {
                return;
            };
            {
                let inv = &mut game.narrative.inventory;
                let ce = game
                    .narrative
                    .container_inventories
                    .entry(container.0)
                    .or_default();
                match focus {
                    TransferFocus::Player => {
                        if cursor_player < inv.stacks.len() {
                            let _ = Inventory::try_move_stack_index(inv, ce, cursor_player);
                        }
                    }
                    TransferFocus::Container => {
                        if cursor_container < ce.stacks.len() {
                            let _ = Inventory::try_move_stack_index(ce, inv, cursor_container);
                        }
                    }
                }
            }
            if let Some(GameMode::ItemTransfer {
                cursor_player: cp,
                cursor_container: cc,
                container: cid,
                ..
            }) = game.modes.current_mut()
            {
                let pn = game.narrative.inventory.stacks.len();
                let cn = game
                    .narrative
                    .container_inventories
                    .get(&cid.0)
                    .map(|c| c.stacks.len())
                    .unwrap_or(0);
                *cp = (*cp).min(pn.saturating_sub(1));
                *cc = (*cc).min(cn.saturating_sub(1));
            }
        }
        GameInput::Command(_) | GameInput::Raw(_) => {}
    }
}
