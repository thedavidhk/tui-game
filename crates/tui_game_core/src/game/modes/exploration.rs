use crate::game::services::hover;
use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::ui::layout::GameShellLayout;

pub(crate) fn handle(game: &mut Game, ev: GameInput) {
    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Command(GameCommand::QuickSave) => match game.save_to_path("save.ron") {
            Ok(()) => game.log.push("Saved save.ron (F5).".into()),
            Err(e) => game.log.push(format!("Save failed: {e}")),
        },
        GameInput::Command(GameCommand::QuickLoad) => match game.load_from_path("save.ron") {
            Ok(()) => {}
            Err(e) => game.log.push(format!("Load failed: {e}")),
        },
        GameInput::Command(GameCommand::OpenInventory) => {
            game.modes.push(GameMode::Inventory { cursor: 0 });
        }
        GameInput::Command(GameCommand::OpenJournal) => {
            game.modes.push(GameMode::Journal { quest_cursor: 0 });
        }
        GameInput::Command(GameCommand::ToggleTurnBased) => {
            game.toggle_turn_based();
        }
        GameInput::Command(GameCommand::StepNorth) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, -1);
        }
        GameInput::Command(GameCommand::StepSouth) if game.world_view_needs_pan() => {
            game.nudge_view_pan(0, 1);
        }
        GameInput::Command(GameCommand::StepWest) if game.world_view_needs_pan() => {
            game.nudge_view_pan(-1, 0);
        }
        GameInput::Command(GameCommand::StepEast) if game.world_view_needs_pan() => {
            game.nudge_view_pan(1, 0);
        }
        GameInput::Command(GameCommand::StepNorth) => game.try_move_player(0, -1),
        GameInput::Command(GameCommand::StepSouth) => game.try_move_player(0, 1),
        GameInput::Command(GameCommand::StepWest) => game.try_move_player(-1, 0),
        GameInput::Command(GameCommand::StepEast) => game.try_move_player(1, 0),
        GameInput::Raw(InputEvent::Mouse { kind, cell, .. }) => {
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
        GameInput::Raw(_) => {}
        GameInput::Command(_) => {}
    }
}
