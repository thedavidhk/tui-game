use crate::game::services::hover;
use crate::game::{Game, GameMode};
use crate::input::{InputEvent, Key, KeyChord, MouseButton, MouseEventKind};
use crate::ui::layout::GameShellLayout;

pub(crate) fn handle(game: &mut Game, ev: InputEvent) {
    match ev {
        InputEvent::Key(KeyChord { key: Key::F(1), .. }) => {
            game.debug_overlay = !game.debug_overlay;
        }
        InputEvent::Key(KeyChord { key: Key::F(5), .. }) => match game.save_to_path("save.ron") {
            Ok(()) => game.log.push("Saved save.ron (F5).".into()),
            Err(e) => game.log.push(format!("Save failed: {e}")),
        },
        InputEvent::Key(KeyChord { key: Key::F(9), .. }) => match game.load_from_path("save.ron") {
            Ok(()) => {}
            Err(e) => game.log.push(format!("Load failed: {e}")),
        },
        InputEvent::Key(KeyChord {
            key: Key::Char('i'),
            ..
        }) => {
            game.modes.push(GameMode::Inventory { cursor: 0 });
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('j'),
            ..
        }) => {
            game.modes.push(GameMode::Journal { quest_cursor: 0 });
        }
        InputEvent::Key(KeyChord { key: Key::Up, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, -1);
        }
        InputEvent::Key(KeyChord { key: Key::Down, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, 1);
        }
        InputEvent::Key(KeyChord { key: Key::Left, .. }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(-1, 0);
        }
        InputEvent::Key(KeyChord {
            key: Key::Right,
            ..
        }) if game.world_view_needs_pan() => {
            game.nudge_view_pan(1, 0);
        }
        InputEvent::Key(KeyChord {
            key: Key::Char('w'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Up, .. }) => game.try_move_player(0, -1),
        InputEvent::Key(KeyChord {
            key: Key::Char('s'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Down, .. }) => game.try_move_player(0, 1),
        InputEvent::Key(KeyChord {
            key: Key::Char('a'),
            ..
        })
        | InputEvent::Key(KeyChord { key: Key::Left, .. }) => game.try_move_player(-1, 0),
        InputEvent::Key(KeyChord {
            key: Key::Char('d'),
            ..
        })
        | InputEvent::Key(KeyChord {
            key: Key::Right,
            ..
        }) => game.try_move_player(1, 0),
        InputEvent::Mouse {
            kind,
            cell,
            ..
        } => {
            let world_r = GameShellLayout::root_panels(game.viewport_w, game.viewport_h).0;
            hover::sync_exploration_hover(game, cell, world_r);
            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    game.try_exploration_primary_click(cell);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    game.try_set_player_walk_goal_from_screen(cell);
                }
                _ => {}
            }
        }
        _ => {}
    }
}
