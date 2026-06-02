use super::pacing;
use crate::ai::combat::ChaseNearestPolicy;
use crate::ai::{AiIntent, CombatAiCtx, CombatDecisionPolicy};
use crate::combat::{
    CombatAction, CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile,
};
use crate::entity::EntityId;
use crate::game::{Game, GameMode, Relation};

pub fn detect_hostile_encounters(game: &mut Game) -> bool {
    let Some(pid) = game.player_id() else {
        return false;
    };
    for i in 0..game.entities.alive.len() {
        if !game.entities.alive[i] {
            continue;
        }
        let eid = EntityId(i as u32);
        if eid == pid {
            continue;
        }
        let Some(kind) = game.entities.npc_kind.get(i).and_then(|k| k.as_deref()) else {
            continue;
        };
        let Some(bp) = game.content.blueprint(kind) else {
            continue;
        };
        let Some(trigger) = bp.behavior.hostile_trigger else {
            continue;
        };
        if !matches!(game.relation_to_player(eid), Relation::Hostile) {
            continue;
        }
        if !game.hostile_trigger_met(eid, trigger) {
            continue;
        }

        if !matches!(game.modes.current(), Some(GameMode::Combat(_))) {
            start_combat_encounter(
                game,
                vec![pid, eid],
                EncounterProfile {
                    ruleset: CombatRuleset::Lethal,
                    outcome_policy: EncounterOutcomePolicy::None,
                },
                "Hostile contact!",
            );
            return true;
        }
    }
    false
}

pub fn start_combat_encounter(
    game: &mut Game,
    participants: Vec<EntityId>,
    profile: EncounterProfile,
    message: &str,
) {
    if participants.len() < 2 {
        game.log
            .push("Need at least two actors to start combat.".into());
        return;
    }
    let state = CombatState::from_participants(
        participants,
        &game.entities,
        game.map.width,
        game.map.height,
        &mut game.rng_seed,
        profile,
    );
    game.modes.push(GameMode::Combat(state));
    game.log.push(message.into());
}

pub fn finish_combat(game: &mut Game, state: &CombatState) {
    game.npc_combat_ai_tick_cooldown = 0;
    game.combat_hover_cell = None;
    game.clear_player_walk();
    if matches!(
        state.profile.ruleset,
        CombatRuleset::NonLethalSpar | CombatRuleset::NonLethalBrawl
    ) {
        game.schedule_training_spar_epilogue(state);
        for id in &state.initiative {
            if let Some(stats) = game.entities.stats_mut(*id) {
                stats.hp = stats.max_hp;
            }
        }
        game.log
            .push("Training fight ends. Everyone catches their breath.".into());
    }
    game.log.push("Combat ended.".into());
    let _ = game.modes.pop();
    if matches!(state.profile.ruleset, CombatRuleset::Lethal)
        && game
            .player_id()
            .is_some_and(|pid| !game.entities.is_alive(pid))
    {
        game.modes.stack = vec![GameMode::GameOver];
        game.log.push("You have fallen.".into());
    }
}

pub fn step_npc_combat_ai(game: &mut Game) {
    let Some(GameMode::Combat(state)) = game.modes.current().cloned() else {
        game.npc_combat_ai_tick_cooldown = 0;
        return;
    };
    let Some(actor) = state.current_actor() else {
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

    let mut next = state;

    let intent = if is_actively_hostile_to_player(game, actor) {
        let ai = ChaseNearestPolicy;
        ai.decide(
            actor,
            &CombatAiCtx {
                state: &next,
                map: &game.map,
                entities: &game.entities,
            },
        )
    } else if let Some(intent) = decide_investigation_intent(game, actor, &next) {
        intent
    } else {
        decide_routine_intent(game, actor, &next)
    };

    let pace_after_success = matches!(
        &intent,
        AiIntent::Combat(CombatAction::Move { .. } | CombatAction::Attack { .. })
    );
    let report = match intent {
        AiIntent::Combat(action) => next.apply_action(
            action,
            &mut game.entities,
            &mut game.rng_seed,
            |x, y| game.map.blocks_movement(x, y),
            Some(&game.map),
            None,
        ),
        AiIntent::Wait => next.apply_action(
            CombatAction::Pass,
            &mut game.entities,
            &mut game.rng_seed,
            |_x, _y| false,
            None,
            None,
        ),
    };
    if report.applied && pace_after_success {
        let speed = game.entities.stats(actor).map_or(1, |stats| stats.speed);
        game.npc_combat_ai_tick_cooldown = pacing::visual_step_cooldown_ticks_from_speed(speed);
    }
    game.apply_combat_report(&next, report);
    if let Some(GameMode::Combat(cs)) = game.modes.current_mut() {
        *cs = next;
    }
}

pub fn is_actively_hostile_to_player(game: &Game, actor: EntityId) -> bool {
    if !game.entities.is_alive(actor) {
        return false;
    }
    if !matches!(game.relation_to_player(actor), Relation::Hostile) {
        return false;
    }
    let Some(kind) = game
        .entities
        .npc_kind
        .get(actor.0 as usize)
        .and_then(|k| k.as_deref())
    else {
        return false;
    };
    let Some(bp) = game.content.blueprint(kind) else {
        return false;
    };
    let Some(trigger) = bp.behavior.hostile_trigger else {
        return false;
    };
    game.hostile_trigger_met(actor, trigger)
}

fn decide_routine_intent(game: &mut Game, actor: EntityId, _state: &CombatState) -> AiIntent {
    let Some(kind) = game
        .entities
        .npc_kind
        .get(actor.0 as usize)
        .and_then(|k| k.as_deref())
    else {
        return AiIntent::Wait;
    };
    let Some(bp) = game.content.blueprint(kind) else {
        return AiIntent::Wait;
    };
    let Some(from) = game.entities.pos(actor) else {
        return AiIntent::Wait;
    };
    let mut brain = game
        .entities
        .npc_brain
        .get(actor.0 as usize)
        .copied()
        .unwrap_or_default();
    let routine = bp.behavior.routine;
    let next_step = crate::ai::exploration::next_exploration_step(
        actor,
        from,
        routine,
        &mut brain,
        &game.map,
        &game.entities,
        &mut game.rng_seed,
    );
    if let Some(slot) = game.entities.npc_brain.get_mut(actor.0 as usize) {
        *slot = brain;
    }

    if let Some(target) = next_step {
        AiIntent::Combat(CombatAction::Move { target })
    } else {
        AiIntent::Wait
    }
}

fn decide_investigation_intent(
    game: &mut Game,
    actor: EntityId,
    _state: &CombatState,
) -> Option<AiIntent> {
    let brain = game.entities.npc_brain.get(actor.0 as usize)?;
    let goal = brain.investigation_goal?;
    let from = game.entities.pos(actor)?;

    if from == goal {
        if let Some(brain_mut) = game.entities.npc_brain.get_mut(actor.0 as usize) {
            brain_mut.investigation_goal = None;
        }
        return None;
    }

    let plan = crate::world::plan_path(
        &game.map,
        &game.entities,
        from,
        goal,
        Some(actor),
        true,
        u32::MAX,
    )
    .ok()?;

    let waypoint = plan.path.get(1).copied()?;
    let next = crate::world::first_step_on_line(from, waypoint)?;

    Some(AiIntent::Combat(CombatAction::Move { target: next }))
}

pub fn toggle_turn_based(game: &mut Game) {
    match game.modes.current().cloned() {
        Some(GameMode::Exploration) => {
            enter_turn_based_manual(game);
        }
        Some(GameMode::Combat(state)) => {
            exit_turn_based_manual(game, &state);
        }
        _ => {}
    }
}

fn enter_turn_based_manual(game: &mut Game) {
    let Some(pid) = game.player_id() else {
        return;
    };
    let Some(ppos) = game.entities.pos(pid) else {
        return;
    };

    let mut participants = vec![pid];
    for i in 0..game.entities.alive.len() {
        if !game.entities.alive[i] {
            continue;
        }
        let eid = EntityId(i as u32);
        if eid == pid {
            continue;
        }
        if game.entities.npc_kind[i].is_none() {
            continue;
        }
        let Some(epos) = game.entities.pos(eid) else {
            continue;
        };
        // FOW_RADIUS is 20
        if crate::game::services::hover::chebyshev(ppos, epos) <= 20 {
            participants.push(eid);
        }
    }

    start_combat_encounter(
        game,
        participants,
        EncounterProfile {
            ruleset: CombatRuleset::Lethal,
            outcome_policy: EncounterOutcomePolicy::None,
        },
        "Turn-based mode activated.",
    );
}

fn exit_turn_based_manual(game: &mut Game, state: &CombatState) {
    for &id in &state.initiative {
        if is_actively_hostile_to_player(game, id) {
            game.log
                .push("Cannot leave turn-based mode while hostiles are engaged!".into());
            return;
        }
    }

    game.log.push("Leaving turn-based mode.".into());
    finish_combat(game, state);
}
