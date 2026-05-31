use crate::combat::{CombatAction, CombatState};
use crate::game::{Game, GameCommand, GameInput, GameMode};
use crate::game::services::hover;
use crate::input::{InputEvent, MouseButton, MouseEventKind};
use crate::ui::layout::GameShellLayout;

pub(crate) fn handle(game: &mut Game, ev: GameInput, state: CombatState) {
    let mut next = state.clone();
    let world_r = GameShellLayout::root_panels(game.viewport_w, game.viewport_h).0;

    match ev {
        GameInput::Command(GameCommand::ToggleDebug) => {
            game.debug_overlay = !game.debug_overlay;
        }
        GameInput::Raw(InputEvent::Mouse {
            kind,
            cell,
            ..
        }) => {
            hover::sync_combat_hover(game, cell, world_r);
            match kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    game.combat_try_primary_click(&mut next, cell);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    game.combat_rmb_march_toward(&mut next, cell);
                }
                _ => {}
            }
        }
        GameInput::Command(GameCommand::Confirm) => {
            let report = next.apply_action(
                CombatAction::Pass,
                &mut game.entities,
                &mut game.rng_seed,
                |_x, _y| false,
                None,
                None,
            );
            game.apply_combat_report(&next, report);
        }
        GameInput::Command(GameCommand::ToggleTurnBased) => {
            game.toggle_turn_based();
            return;
        }
        GameInput::Command(GameCommand::CombatFlee) => {
            let report = next.apply_action(
                CombatAction::Flee,
                &mut game.entities,
                &mut game.rng_seed,
                |_x, _y| false,
                None,
                None,
            );
            game.apply_combat_report(&next, report);
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
        GameInput::Command(GameCommand::StepNorth) => {
            game.combat_try_move(&mut next, 0, -1);
        }
        GameInput::Command(GameCommand::StepSouth) => {
            game.combat_try_move(&mut next, 0, 1);
        }
        GameInput::Command(GameCommand::StepWest) => {
            game.combat_try_move(&mut next, -1, 0);
        }
        GameInput::Command(GameCommand::StepEast) => {
            game.combat_try_move(&mut next, 1, 0);
        }
        GameInput::Command(GameCommand::Back) => {
            game.finish_combat_player_quit(&next);
            return;
        }
        GameInput::Command(_) => {}
        GameInput::Raw(_) => {}
    }
    if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
        *cs = next;
    }
}
