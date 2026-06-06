//! Headless behavior tests (no `Game`, no render).

use crate::combat::{
    CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile,
};
use crate::content::{EncounterTriggerDef, NpcRoutineDef, ReactionDef, Relation};
use crate::entity::{ActiveReaction, ActorStats, EntityArena, EntityId, GridPos, NpcBrainState};
use crate::game_content;
use crate::math::chebyshev;
use crate::render::Color;
use crate::world::{MapGrid, TileTable};

use super::action::NpcAction;
use super::constraints::ActionConstraints;
use super::ctx::BehaviorCtx;
use super::decide::decide_actor_action;
use super::encounter::find_encounter_start;
use super::exploration::{routine_tick, ExplorationIntent};
use super::navigation::next_step_toward;
use super::relation::{BlueprintRelationResolver, RelationResolver};
use super::threat::{evaluate_encounter_trigger, is_actively_hostile_to_player_with, nearest_non_allied_threat};

fn floor_map(w: u16, h: u16) -> MapGrid {
    let table = TileTable::default_pack().expect("default pack");
    let floor = table.defs.first().map_or(0, |d| d.idx);
    MapGrid::filled(w, h, floor, table)
}

fn spawn_npc(
    arena: &mut EntityArena,
    pos: GridPos,
    kind: &'static str,
    mut brain: NpcBrainState,
) -> EntityId {
    let id = arena.spawn(
        pos,
        'n',
        Color::rgb(200, 200, 200),
        kind.into(),
        true,
        Some(kind.to_string()),
        None,
        false,
    );
    arena.set_stats(id, ActorStats::from_full(10, 10, 10, 0, 5));
    brain.home = pos;
    arena.npc_brain[id.0 as usize] = brain;
    id
}

fn spawn_player(arena: &mut EntityArena, pos: GridPos) -> EntityId {
    let id = arena.spawn(
        pos,
        '@',
        Color::rgb(255, 255, 255),
        "Player".into(),
        true,
        None,
        None,
        true,
    );
    arena.set_stats(id, ActorStats::from_full(20, 20, 20, 0, 8));
    id
}

#[test]
fn navigation_open_path_returns_adjacent_step() {
    let map = floor_map(8, 8);
    let mut arena = EntityArena::new();
    let actor = spawn_npc(
        &mut arena,
        GridPos { x: 1, y: 1 },
        "wolf",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: None,
    };
    let step = next_step_toward(
        &ctx,
        actor,
        GridPos { x: 1, y: 1 },
        GridPos { x: 4, y: 1 },
    );
    assert_eq!(step, Some(GridPos { x: 2, y: 1 }));
}

#[test]
fn navigation_same_cell_returns_none() {
    let map = floor_map(8, 8);
    let mut arena = EntityArena::new();
    let actor = spawn_npc(
        &mut arena,
        GridPos { x: 2, y: 2 },
        "wolf",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: None,
    };
    let here = GridPos { x: 2, y: 2 };
    assert_eq!(next_step_toward(&ctx, actor, here, here), None);
}

#[test]
fn exploration_roam_picks_step_within_radius() {
    let map = floor_map(12, 12);
    let mut arena = EntityArena::new();
    let home = GridPos { x: 5, y: 5 };
    let mut brain = NpcBrainState::default();
    brain.home = home;
    let actor = spawn_npc(&mut arena, home, "wolf", brain);
    let mut rng = 42u64;
    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: None,
    };
    let mut brain = ctx.entities.npc_brain[actor.0 as usize];
    let intent = routine_tick(
        &mut ctx,
        actor,
        home,
        NpcRoutineDef::Roam {
            radius: 3,
            wait_ticks: 0,
        },
        &mut brain,
    );
    if let ExplorationIntent::Step(target) = intent {
        assert!(crate::math::chebyshev(home, target) <= 3);
    }
}

#[test]
fn encounter_trigger_fires_when_hostile_and_in_range() {
    let map = floor_map(10, 10);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 5, y: 5 });
    let wolf = spawn_npc(
        &mut arena,
        GridPos { x: 7, y: 5 },
        "wolf",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    assert!(evaluate_encounter_trigger(
        &ctx,
        wolf,
        EncounterTriggerDef::PlayerWithinChebyshev { range: 4 }
    ));
    assert_eq!(find_encounter_start(&ctx), Some((player, wolf)));
    assert!(is_actively_hostile_to_player_with(&arena, &content, Some(player), wolf));
}

#[test]
fn encounter_trigger_does_not_fire_when_out_of_range() {
    let map = floor_map(20, 20);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 0, y: 0 });
    let wolf = spawn_npc(
        &mut arena,
        GridPos { x: 15, y: 15 },
        "wolf",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    assert!(!evaluate_encounter_trigger(
        &ctx,
        wolf,
        EncounterTriggerDef::PlayerWithinChebyshev { range: 4 }
    ));
    assert_eq!(find_encounter_start(&ctx), None);
}

#[test]
fn fight_nearest_attacks_adjacent_in_encounter() {
    let map = floor_map(12, 12);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 4, y: 4 });
    let wolf = spawn_npc(
        &mut arena,
        GridPos { x: 5, y: 4 },
        "wolf",
        NpcBrainState::default(),
    );
    let mut rng = 7u64;
    let mut state = CombatState::from_participants(
        vec![wolf, player],
        &arena,
        12,
        12,
        &mut rng,
        EncounterProfile {
            ruleset: CombatRuleset::Lethal,
            outcome_policy: EncounterOutcomePolicy::None,
        },
    );
    state.turn_index = state.initiative.iter().position(|&id| id == wolf).unwrap();
    state.ap_remaining[state.turn_index] = 100;

    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    let constraints = ActionConstraints::for_turn(&state, wolf, true);
    let action = decide_actor_action(&mut ctx, wolf, constraints, Some(&state));
    assert!(matches!(
        action,
        NpcAction::Attack {
            target,
            ..
        } if target == player
    ));
}

#[test]
fn investigate_clears_at_goal() {
    use super::reactions;

    let map = floor_map(8, 8);
    let mut arena = EntityArena::new();
    let goal = GridPos { x: 3, y: 3 };
    let mut brain = NpcBrainState::default();
    brain.active = ActiveReaction::Investigate(goal);
    let npc = spawn_npc(&mut arena, goal, "wolf", brain);
    let mut rng = 1u64;
    let mut state = CombatState::from_participants(
        vec![npc],
        &arena,
        8,
        8,
        &mut rng,
        EncounterProfile {
            ruleset: CombatRuleset::Lethal,
            outcome_policy: EncounterOutcomePolicy::None,
        },
    );
    state.turn_index = 0;
    state.ap_remaining[0] = 50;

    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: None,
    };
    let constraints = ActionConstraints::for_turn(&state, npc, true);
    let _action = reactions::try_reaction(
        &mut ctx,
        npc,
        ReactionDef::InvestigateLastHit,
        constraints,
        Some(&state),
    );
    assert!(matches!(
        ctx.entities.npc_brain[npc.0 as usize].active,
        ActiveReaction::None
    ));
}

#[test]
fn proximity_threat_detects_adjacent_player() {
    let map = floor_map(12, 12);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 5, y: 5 });
    let deer = spawn_npc(
        &mut arena,
        GridPos { x: 6, y: 5 },
        "deer",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    assert!(nearest_non_allied_threat(&ctx, deer, 10).is_some());
}

#[test]
fn skittish_deer_flees_when_player_near() {
    let map = floor_map(40, 40);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 5, y: 5 });
    let deer = spawn_npc(
        &mut arena,
        GridPos { x: 6, y: 5 },
        "deer",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    let constraints = ActionConstraints::realtime(deer);
    let action = decide_actor_action(&mut ctx, deer, constraints, None);
    assert!(
        matches!(action, NpcAction::Step(_)),
        "expected urgent flee step, got {action:?}"
    );
    assert_eq!(find_encounter_start(&ctx), None);
}

#[test]
fn fleeing_latches_until_clearance() {
    let map = floor_map(40, 40);
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 5, y: 5 });
    let mut brain = NpcBrainState::default();
    brain.active = ActiveReaction::Flee {
        from: GridPos { x: 5, y: 5 },
    };
    let deer = spawn_npc(
        &mut arena,
        GridPos { x: 14, y: 5 },
        "deer",
        brain,
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: Some(player),
    };
    let constraints = ActionConstraints::realtime(deer);
    let action = decide_actor_action(&mut ctx, deer, constraints, None);
    assert!(
        matches!(action, NpcAction::Step(_)),
        "outside trigger but inside clearance should keep fleeing"
    );
}

#[test]
fn skittish_deer_flees_to_clearance_before_roaming() {
    let map = floor_map(40, 40);
    let mut arena = EntityArena::new();
    let player_pos = GridPos { x: 5, y: 5 };
    let player = spawn_player(&mut arena, player_pos);
    let deer = spawn_npc(
        &mut arena,
        GridPos { x: 6, y: 5 },
        "deer",
        NpcBrainState::default(),
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let constraints = ActionConstraints::realtime(deer);

    for _ in 0..40 {
        let mut ctx = BehaviorCtx {
            map: &map,
            entities: &mut arena,
            content: &content,
            rng: &mut rng,
            player: Some(player),
        };
        let action = decide_actor_action(&mut ctx, deer, constraints, None);
        if let Some(target) = action.step_target() {
            arena.set_pos(deer, target);
        }
    }

    let deer_pos = arena.pos(deer).expect("deer position");
    assert!(
        chebyshev(deer_pos, player_pos) >= 10,
        "deer should reach flee clearance (10) from player"
    );
    assert!(arena.npc_brain[deer.0 as usize].alarmed_ticks > 0);
}

#[test]
fn routine_movement_uses_roam_step() {
    let map = floor_map(12, 12);
    let mut arena = EntityArena::new();
    let mut brain = NpcBrainState::default();
    brain.roam_goal = Some(GridPos { x: 4, y: 1 });
    let wolf = spawn_npc(
        &mut arena,
        GridPos { x: 1, y: 1 },
        "wolf",
        brain,
    );
    let mut rng = 1u64;
    let content = game_content::content_pack();
    let mut ctx = BehaviorCtx {
        map: &map,
        entities: &mut arena,
        content: &content,
        rng: &mut rng,
        player: None,
    };
    let constraints = ActionConstraints::realtime(wolf);
    let action = decide_actor_action(&mut ctx, wolf, constraints, None);
    assert!(matches!(action, NpcAction::RoamStep(_)));
}

#[test]
fn skittish_preset_includes_flee_reaction() {
    let content = game_content::content_pack();
    let deer = content.blueprint("deer").expect("deer blueprint");
    assert!(deer.behavior.reactions.iter().any(|r| {
        matches!(r, ReactionDef::FleeFromThreat { .. })
    }));
}

#[test]
fn relation_resolver_uses_disposition() {
    let mut arena = EntityArena::new();
    let player = spawn_player(&mut arena, GridPos { x: 0, y: 0 });
    let wolf = spawn_npc(
        &mut arena,
        GridPos { x: 1, y: 0 },
        "wolf",
        NpcBrainState::default(),
    );
    let healer = spawn_npc(
        &mut arena,
        GridPos { x: 2, y: 0 },
        "healer",
        NpcBrainState::default(),
    );
    let content = game_content::content_pack();
    let resolver = BlueprintRelationResolver {
        player,
        content: &content,
        entities: &arena,
    };
    assert_eq!(resolver.relation(player, wolf), Relation::Hostile);
    assert_eq!(resolver.relation(player, healer), Relation::Friendly);
}
