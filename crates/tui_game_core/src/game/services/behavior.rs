//! Thin adapter between [`crate::game::Game`] and [`crate::behavior`].

use crate::behavior::{
    decide_actor_action, find_encounter_start, npc_action_to_combat, ActionConstraints,
    BehaviorCtx, NpcAction,
};
use crate::combat::{CombatAction, CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile};
use crate::entity::EntityId;
use crate::game::{Game, GameMode};

use super::combat::start_combat_encounter;
use super::pacing;

const NPC_EXPLORATION_AI_COOLDOWN_TICKS: u16 = 6;
const ROAM_PACE_MULTIPLIER: u16 = 2;

pub(crate) fn behavior_ctx(game: &mut Game) -> BehaviorCtx<'_> {
    let player = game.player_id();
    BehaviorCtx {
        map: &game.map,
        entities: &mut game.entities,
        content: &game.content,
        rng: &mut game.rng_seed,
        player,
    }
}

/// Active turn clock: combat encounter on the mode stack, or overworld turn-based session.
#[must_use]
pub fn active_turn_clock(game: &Game) -> Option<&CombatState> {
    if let Some(GameMode::Combat(cs)) = game.modes.current() {
        return Some(cs);
    }
    game.turn_clock.as_ref()
}

#[must_use]
pub fn active_turn_clock_mut(game: &mut Game) -> Option<&mut CombatState> {
    if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
        return Some(cs);
    }
    game.turn_clock.as_mut()
}

/// Whether the game is in a lethal/training encounter (combat HUD, encounter rules).
#[must_use]
pub fn in_encounter(game: &Game) -> bool {
    matches!(game.modes.current(), Some(GameMode::Combat(_)))
}

/// Whether any turn-based clock is running (encounter or overworld toggle).
#[must_use]
pub fn turn_based_active(game: &Game) -> bool {
    active_turn_clock(game).is_some()
}

/// NPC behavior tick: exploration realtime, overworld turn clock, or combat encounter.
pub fn tick_npcs(game: &mut Game) {
    if matches!(game.modes.current(), Some(GameMode::MainMenu { .. }) | Some(GameMode::GameOver)) {
        game.npc_exploration_ai_tick_cooldown = 0;
        game.npc_combat_ai_tick_cooldown = 0;
        return;
    }

    // Encounter start (exploration only).
    if matches!(game.modes.current(), Some(GameMode::Exploration)) && !in_encounter(game) {
        let encounter = {
            let ctx = behavior_ctx(game);
            find_encounter_start(&ctx)
        };
        if let Some((player, hostile)) = encounter {
            start_combat_encounter(
                game,
                vec![player, hostile],
                EncounterProfile {
                    ruleset: CombatRuleset::Lethal,
                    outcome_policy: EncounterOutcomePolicy::None,
                },
                "Hostile contact!",
            );
            game.npc_exploration_ai_tick_cooldown = 0;
            return;
        }
    }

    // Turn-based NPC turn (combat mode or overworld turn clock).
    if let Some(clock) = active_turn_clock(game).cloned() {
        tick_turn_actor(game, &clock);
        return;
    }

    // Realtime exploration movement.
    tick_realtime_explore(game);
}

fn tick_realtime_explore(game: &mut Game) {
    if !matches!(game.modes.current(), Some(GameMode::Exploration)) {
        game.npc_exploration_ai_tick_cooldown = 0;
        return;
    }

    for brain in &mut game.entities.npc_brain {
        brain.explore_step_cooldown = brain.explore_step_cooldown.saturating_sub(1);
    }

    let mut steps = Vec::new();
    {
        let mut ctx = behavior_ctx(game);
        for i in 0..ctx.entities.alive.len() {
            if !ctx.entities.alive[i] {
                continue;
            }
            let actor = EntityId(i as u32);
            if ctx.player_id() == Some(actor) {
                continue;
            }
            if ctx.blueprint_for(actor).is_none() {
                continue;
            }
            if ctx.entities.npc_brain[i].explore_step_cooldown > 0 {
                continue;
            }
            let Some(from) = ctx.entities.pos(actor) else {
                continue;
            };
            let constraints = ActionConstraints::realtime(actor);
            let action = decide_actor_action(&mut ctx, actor, constraints, None);
            if let Some(target) = action.step_target() {
                steps.push((actor, from, target, action.is_leisurely_step()));
            }
        }
    }

    for (actor, from, target, leisurely) in steps {
        if game.entities.can_move_to(
            game.map.blocks_movement(target.x, target.y),
            target,
            Some(actor),
        ) {
            let dx = target.x - from.x;
            let dy = target.y - from.y;
            game.entities.set_pos(actor, target);
            let speed = game.entities.stats(actor).map_or(5, |stats| stats.speed);
            let base = if leisurely {
                NPC_EXPLORATION_AI_COOLDOWN_TICKS
            } else {
                pacing::visual_step_cooldown_ticks_from_speed(speed)
            };
            let mut cooldown = pacing::scaled_step_cooldown(base, dx, dy);
            if leisurely {
                cooldown = cooldown.saturating_mul(ROAM_PACE_MULTIPLIER);
            }
            game.entities.npc_brain[actor.0 as usize].explore_step_cooldown = cooldown;
        }
    }
    game.npc_exploration_ai_tick_cooldown = 0;
}

fn tick_turn_actor(game: &mut Game, clock: &CombatState) {
    let Some(actor) = clock.current_actor() else {
        return;
    };
    if game.player_id() == Some(actor) {
        game.npc_combat_ai_tick_cooldown = 0;
        return;
    }
    if game.npc_combat_ai_tick_cooldown > 0 {
        game.npc_combat_ai_tick_cooldown = game.npc_combat_ai_tick_cooldown.saturating_sub(1);
        return;
    }

    let actor_pos_before = game.entities.pos(actor);
    let constraints = ActionConstraints::for_turn(clock, actor, in_encounter(game));
    let action = {
        let mut ctx = behavior_ctx(game);
        decide_actor_action(&mut ctx, actor, constraints, Some(clock))
    };

    let pace_after_success = matches!(
        action,
        NpcAction::Step(_) | NpcAction::RoamStep(_) | NpcAction::Attack { .. }
    );
    let move_step = action.step_target().and_then(|target| {
        actor_pos_before.map(|from| (target.x - from.x, target.y - from.y))
    });
    let leisurely_step = action.is_leisurely_step();

    let combat_action = npc_action_to_combat(action);
    apply_turn_action(
        game,
        combat_action,
        move_step,
        pace_after_success,
        leisurely_step,
        actor,
    );
}

fn apply_turn_action(
    game: &mut Game,
    action: CombatAction,
    move_step: Option<(i32, i32)>,
    pace_after_success: bool,
    leisurely_step: bool,
    actor: EntityId,
) {
    let Some(clock) = active_turn_clock_mut(game) else {
        return;
    };
    let mut next = clock.clone();
    let report = next.apply_action(
        action,
        &mut game.entities,
        &mut game.rng_seed,
        |x, y| game.map.blocks_movement(x, y),
        Some(&game.map),
        None,
        Some(&game.content),
    );

    if report.applied && pace_after_success {
        let speed = game.entities.stats(actor).map_or(1, |stats| stats.speed);
        let mut base = pacing::visual_step_cooldown_ticks_from_speed(speed);
        if leisurely_step {
            base = base.saturating_mul(ROAM_PACE_MULTIPLIER);
        }
        game.npc_combat_ai_tick_cooldown = move_step
            .map_or(base, |(dx, dy)| pacing::scaled_step_cooldown(base, dx, dy));
    }

    if in_encounter(game) {
        game.apply_combat_report(&next, report);
        if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
            *cs = next;
        }
    } else if let Some(slot) = game.turn_clock.as_mut() {
        *slot = next;
    }
}

