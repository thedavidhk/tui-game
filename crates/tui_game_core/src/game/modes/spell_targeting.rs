//! Input handling for [`super::super::GameMode::SpellTargeting`].
//!
//! Mouse hover tracks the aim cursor; LMB fires; WASD / arrow keys nudge by one cell;
//! Enter / Space confirms; Esc cancels.

use crate::entity::GridPos;
use crate::game::spell::{self, SpellKind};
use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn handle(game: &mut Game, ev: GameInput, spell: SpellKind, cursor: GridPos) {
    match ev {
        GameInput::Command(GameCommand::Back) => {
            game.modes.pop();
            game.log.push("Spell cancelled.".into());
        }
        GameInput::Command(GameCommand::Confirm) => {
            try_cast(game, spell, cursor);
        }
        GameInput::Command(GameCommand::StepNorth) => set_cursor(game, GridPos { x: cursor.x, y: cursor.y - 1 }),
        GameInput::Command(GameCommand::StepSouth) => set_cursor(game, GridPos { x: cursor.x, y: cursor.y + 1 }),
        GameInput::Command(GameCommand::StepWest) => set_cursor(game, GridPos { x: cursor.x - 1, y: cursor.y }),
        GameInput::Command(GameCommand::StepEast) => set_cursor(game, GridPos { x: cursor.x + 1, y: cursor.y }),
        GameInput::Raw(InputEvent::Mouse { kind, cell, .. }) => match kind {
            MouseEventKind::Moved => {
                if let Some(wp) = game.screen_cell_to_world(cell) {
                    set_cursor(game, wp);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Update cursor to click position first so the cast uses the latest position.
                if let Some(wp) = game.screen_cell_to_world(cell) {
                    set_cursor(game, wp);
                    try_cast(game, spell, wp);
                } else {
                    // Clicked outside the world viewport — cancel.
                    game.modes.pop();
                    game.log.push("Spell cancelled.".into());
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                game.modes.pop();
                game.log.push("Spell cancelled.".into());
            }
            _ => {}
        },
        GameInput::Raw(_) | GameInput::Command(_) => {}
    }
}

fn set_cursor(game: &mut Game, new_cursor: GridPos) {
    if let Some(GameMode::SpellTargeting { cursor: ref mut c, .. }) = game.modes.current_mut() {
        *c = new_cursor;
    }
}

fn try_cast(game: &mut Game, spell: SpellKind, cursor: GridPos) {
    let def = spell::def(spell);
    let Some(player_pos) = game.player_pos() else {
        game.modes.pop();
        return;
    };
    let in_range = spell::in_range(player_pos, cursor, spell);
    game.modes.pop();
    if in_range {
        if let Err(msg) = spell::cast_spell(game, spell, cursor) {
            game.log.push(msg.to_string());
        }
    } else {
        game.log.push(format!("{} is out of range.", def.name));
    }
}
