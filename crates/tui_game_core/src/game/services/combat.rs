use crate::combat::{
    CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile,
};
use crate::entity::EntityId;
use crate::game::{Game, GameMode};

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
    game.turn_clock = None;
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
    game.active_projectiles.clear();
    game.pending_hits.clear();
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

pub fn toggle_turn_based(game: &mut Game) {
    match game.modes.current().cloned() {
        Some(GameMode::Exploration) => {
            if game.turn_clock.is_some() {
                game.turn_clock = None;
                game.log.push("Realtime exploration.".into());
            } else {
                enter_turn_based_overworld(game);
            }
        }
        Some(GameMode::Combat(state)) => {
            exit_turn_based_encounter(game, &state);
        }
        _ => {}
    }
}

fn enter_turn_based_overworld(game: &mut Game) {
    let Some(pid) = game.player_id() else {
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
        if game.entities.npc_kind[i].is_some() {
            participants.push(eid);
        }
    }
    game.turn_clock = Some(CombatState::from_participants(
        participants,
        &game.entities,
        game.map.width,
        game.map.height,
        &mut game.rng_seed,
        EncounterProfile {
            ruleset: CombatRuleset::Lethal,
            outcome_policy: EncounterOutcomePolicy::None,
        },
    ));
    game.log.push("Turn-based mode.".into());
}

fn exit_turn_based_encounter(game: &mut Game, state: &CombatState) {
    finish_combat(game, state);
    game.modes.push(GameMode::Exploration);
}
