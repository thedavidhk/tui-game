//! Turn-based combat core (initiative, AP economy, movement, melee and ranged attacks).

use serde::{Deserialize, Serialize};

use crate::entity::{EntityArena, EntityId, GridPos};
use crate::item::StackEquipped;
use crate::narrative::NarrativeState;
use crate::world::{projectile_sight_clear, MapGrid};

pub const ACTION_UNIT: u16 = 10;
pub const MOVE_ORTHOGONAL_COST_UNITS: u16 = 10;
pub const MOVE_DIAGONAL_COST_UNITS: u16 = 14;
pub const ATTACK_COST_UNITS: u16 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatRuleset {
    Lethal,
    NonLethalSpar,
    NonLethalBrawl,
}

impl CombatRuleset {
    #[must_use]
    pub fn is_lethal(self) -> bool {
        matches!(self, Self::Lethal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncounterOutcomePolicy {
    None,
    TrainingSpar { trainer: EntityId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterProfile {
    pub ruleset: CombatRuleset,
    pub outcome_policy: EncounterOutcomePolicy,
}

/// Resolved weapon profile for an [`CombatAction::Attack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackStyle {
    Unarmed,
    Melee {
        to_hit: i8,
        damage_bonus: i8,
    },
    Bow {
        to_hit: i8,
        damage_bonus: i8,
        range: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatAction {
    Move {
        target: GridPos,
    },
    Attack {
        target: EntityId,
        style: AttackStyle,
    },
    Pass,
    Flee,
}
/// Ticks per tile of travel for a flying arrow (at ~60 Hz, 3 ticks ≈ 50 ms per cell).
pub const PROJECTILE_TICKS_PER_CELL: u8 = 1;
/// Floor for ranged flight time so even close shots have a visible arc.
pub const PROJECTILE_MIN_TICKS: u8 = 4;
/// Duration of a melee hit flash (ticks ≈ wall-clock ms / 16).
pub const MELEE_HIT_TICKS: u8 = 6;

/// A resolved attack whose damage application is deferred until the animation completes.
///
/// Stored on [`crate::game::Game`] and ticked down each frame; when `delay_ticks` reaches 0
/// the damage is applied to the target's HP and [`resolved_message`] is pushed to the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingHit {
    pub target: EntityId,
    /// Pre-rolled damage to apply (0 on a miss — effect still shows).
    pub damage: u16,
    /// Kills the target if lethal ruleset and `damage >= target.hp`.
    pub lethal: bool,
    /// Message logged at resolution time.
    pub resolved_message: String,
    /// Countdown in game ticks; damage applies when this reaches 0.
    pub delay_ticks: u8,
    /// Whether the hit was landed (false = miss — no damage, but animation still plays).
    pub hit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatActionReport {
    pub applied: bool,
    /// True only for non-deferred outcomes (Pass / Flee / immediate checks). For attacks with a
    /// pending hit, the caller checks for combat end after [`PendingHit`] resolves.
    pub end_combat: bool,
    /// Immediate log message (e.g. validation errors).  Resolution messages live in [`PendingHit`].
    pub message: Option<String>,
    /// Present when an attack was committed; caller spawns any visual and stores the deferred hit.
    pub pending_hit: Option<PendingHit>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombatState {
    pub initiative: Vec<EntityId>,
    pub turn_index: usize,
    pub ap_remaining: Vec<u16>,
    pub grid_w: u16,
    pub grid_h: u16,
    pub profile: EncounterProfile,
}

impl CombatState {
    pub fn from_participants(
        participants: Vec<EntityId>,
        arena: &EntityArena,
        w: u16,
        h: u16,
        rng_seed: &mut u64,
        profile: EncounterProfile,
    ) -> Self {
        let mut ranked: Vec<(EntityId, i32, i32)> = participants
            .into_iter()
            .map(|id| {
                let speed = arena.stats(id).map_or(1, |s| s.speed);
                let roll = i32::from(roll_d20(rng_seed));
                let total = roll + i32::from(speed_modifier(speed));
                (id, total, i32::from(speed))
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0 .0.cmp(&b.0 .0))
        });
        let initiative: Vec<EntityId> = ranked.into_iter().map(|(id, _, _)| id).collect();
        let ap_remaining = initiative
            .iter()
            .map(|id| arena.stats(*id).map_or(1, |s| ap_from_speed(s.speed)) * ACTION_UNIT)
            .collect();
        Self {
            initiative,
            turn_index: 0,
            ap_remaining,
            grid_w: w,
            grid_h: h,
            profile,
        }
    }

    pub fn current_actor(&self) -> Option<EntityId> {
        self.initiative.get(self.turn_index).copied()
    }

    pub fn contains_actor(&self, id: EntityId) -> bool {
        self.initiative.contains(&id)
    }

    pub fn current_ap(&self) -> Option<u16> {
        self.ap_remaining
            .get(self.turn_index)
            .map(|units| units / ACTION_UNIT)
    }

    pub fn end_turn(&mut self, arena: &EntityArena) {
        if self.initiative.is_empty() {
            return;
        }
        let len = self.initiative.len();
        for step in 1..=len {
            let idx = (self.turn_index + step) % len;
            let id = self.initiative[idx];
            if actor_can_act(arena, id) {
                self.turn_index = idx;
                self.ap_remaining[idx] =
                    arena.stats(id).map_or(1, |s| ap_from_speed(s.speed)) * ACTION_UNIT;
                return;
            }
        }
    }

    pub fn apply_action(
        &mut self,
        action: CombatAction,
        arena: &mut EntityArena,
        rng_seed: &mut u64,
        map_blocks_move: impl Fn(i32, i32) -> bool,
        map: Option<&MapGrid>,
        narrative: Option<&mut NarrativeState>,
    ) -> CombatActionReport {
        let Some(actor) = self.current_actor() else {
            return CombatActionReport {
                applied: false,
                end_combat: true,
                message: Some("Combat has no active participants.".into()),
                pending_hit: None,
            };
        };
        if !actor_can_act(arena, actor) {
            self.end_turn(arena);
            return CombatActionReport {
                applied: false,
                end_combat: self.should_end_combat(arena),
                message: Some("Current actor cannot act.".into()),
                pending_hit: None,
            };
        }
        match action {
            CombatAction::Move { target } => {
                let Some(actor_pos) = arena.pos(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Actor has no position.".into()),
                        pending_hit: None,
                    };
                };
                let Some(move_cost) = move_cost_units(actor_pos, target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Movement target must be adjacent.".into()),
                        pending_hit: None,
                    };
                };
                if self.current_ap_units().unwrap_or(0) < move_cost {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Not enough AP to move.".into()),
                        pending_hit: None,
                    };
                }
                if target.x < 0
                    || target.y < 0
                    || target.x >= self.grid_w as i32
                    || target.y >= self.grid_h as i32
                {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Movement target is out of bounds.".into()),
                        pending_hit: None,
                    };
                }
                let blocked = map_blocks_move(target.x, target.y);
                if !arena.can_move_to(blocked, target, Some(actor)) {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Movement target is blocked.".into()),
                        pending_hit: None,
                    };
                }
                arena.set_pos(actor, target);
                self.consume_current_ap_units(move_cost);
                self.advance_turn_if_needed(arena);
                CombatActionReport {
                    applied: true,
                    end_combat: self.should_end_combat(arena),
                    message: None,
                    pending_hit: None,
                }
            }
            CombatAction::Attack { target, style } => {
                if self.current_ap_units().unwrap_or(0) < ATTACK_COST_UNITS {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Not enough AP to attack.".into()),
                        pending_hit: None,
                    };
                }
                if target == actor || !self.contains_actor(target) || !actor_can_act(arena, target)
                {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Invalid combat target.".into()),
                        pending_hit: None,
                    };
                }
                let Some(attacker_pos) = arena.pos(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Attacker has no position.".into()),
                        pending_hit: None,
                    };
                };
                let Some(target_pos) = arena.pos(target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target has no position.".into()),
                        pending_hit: None,
                    };
                };
                let dist = chebyshev(attacker_pos, target_pos);
                let is_ranged = matches!(style, AttackStyle::Bow { .. });
                let range_ok = match style {
                    AttackStyle::Unarmed | AttackStyle::Melee { .. } => dist == 1,
                    AttackStyle::Bow { range, .. } => {
                        let r = i32::from(range);
                        dist >= 1 && dist <= r
                    }
                };
                if !range_ok {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target is out of weapon range.".into()),
                        pending_hit: None,
                    };
                }
                if is_ranged {
                    let Some(m) = map else {
                        return CombatActionReport {
                            applied: false,
                            end_combat: false,
                            message: Some("Ranged attacks require map context.".into()),
                            pending_hit: None,
                        };
                    };
                    if !projectile_sight_clear(m, attacker_pos, target_pos) {
                        return CombatActionReport {
                            applied: false,
                            end_combat: false,
                            message: Some("Shot is blocked.".into()),
                            pending_hit: None,
                        };
                    }
                    let has_arrow = narrative
                        .as_ref()
                        .is_some_and(|n| n.quiver_count_for_ranged("arrow") > 0);
                    if !has_arrow {
                        return CombatActionReport {
                            applied: false,
                            end_combat: false,
                            message: Some(
                                "Bow needs arrows in the quiver (e on arrows in inventory).".into(),
                            ),
                            pending_hit: None,
                        };
                    }
                }
                let Some(attacker_stats) = arena.stats(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Attacker stats missing.".into()),
                        pending_hit: None,
                    };
                };
                let Some(target_stats) = arena.stats(target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target stats missing.".into()),
                        pending_hit: None,
                    };
                };
                let to_hit = match style {
                    AttackStyle::Unarmed => 0,
                    AttackStyle::Melee { to_hit, .. } | AttackStyle::Bow { to_hit, .. } => {
                        i32::from(to_hit)
                    }
                };
                let attack_roll = i32::from(roll_d20(rng_seed))
                    + i32::from(strength_modifier(attacker_stats.strength))
                    + to_hit;
                let ac = i32::from(armor_class(target_stats.agility));
                let verb = if is_ranged { "Shot" } else { "Attack" };

                // Roll the result now; damage is stored and applied after the animation.
                let (hit, damage, resolved_message) = if attack_roll >= ac {
                    let dmg_bonus = match style {
                        AttackStyle::Unarmed => 0,
                        AttackStyle::Melee { damage_bonus, .. }
                        | AttackStyle::Bow { damage_bonus, .. } => i32::from(damage_bonus),
                    };
                    let dmg =
                        (i32::from(damage_on_hit(attacker_stats.strength)) + dmg_bonus).max(1)
                            as u16;
                    let surviving_hp = target_stats.hp.saturating_sub(dmg);
                    let msg = if surviving_hp == 0 && self.profile.ruleset.is_lethal() {
                        format!("{verb} defeats the target.")
                    } else {
                        format!("{verb} hits for {dmg} damage ({surviving_hp} HP left).")
                    };
                    (true, dmg, msg)
                } else {
                    (false, 0, format!("{verb} misses."))
                };

                // Alert the target's AI about the attacker even before damage lands.
                if hit {
                    if let Some(brain) = arena.npc_brain.get_mut(target.0 as usize) {
                        brain.investigation_goal = Some(attacker_pos);
                    }
                }

                if is_ranged {
                    if let Some(n) = narrative {
                        consume_one_arrow(n);
                    }
                }

                // Delay: ranged scales with distance; melee uses a short haptic flash.
                let delay_ticks = if is_ranged {
                    let dist_u8 = u8::try_from(dist).unwrap_or(u8::MAX);
                    PROJECTILE_MIN_TICKS.max(dist_u8.saturating_mul(PROJECTILE_TICKS_PER_CELL))
                } else {
                    MELEE_HIT_TICKS
                };

                self.consume_current_ap_units(ATTACK_COST_UNITS);
                self.advance_turn_if_needed(arena);
                // end_combat is resolved by the caller after the pending hit fires.
                CombatActionReport {
                    applied: true,
                    end_combat: false,
                    message: None,
                    pending_hit: Some(PendingHit {
                        target,
                        damage,
                        lethal: self.profile.ruleset.is_lethal(),
                        resolved_message,
                        delay_ticks,
                        hit,
                    }),
                }
            }
            CombatAction::Pass => {
                self.end_turn(arena);
                CombatActionReport {
                    applied: true,
                    end_combat: self.should_end_combat(arena),
                    message: None,
                    pending_hit: None,
                }
            }
            CombatAction::Flee => CombatActionReport {
                applied: true,
                end_combat: true,
                message: Some("Flee attempt ends combat.".into()),
                pending_hit: None,
            },
        }
    }

    #[must_use]
    pub fn current_ap_units(&self) -> Option<u16> {
        self.ap_remaining.get(self.turn_index).copied()
    }

    fn consume_current_ap_units(&mut self, cost: u16) {
        if let Some(ap) = self.ap_remaining.get_mut(self.turn_index) {
            *ap = ap.saturating_sub(cost);
        }
    }

    fn advance_turn_if_needed(&mut self, arena: &EntityArena) {
        if self
            .current_ap_units()
            .is_some_and(|units| units < MOVE_ORTHOGONAL_COST_UNITS)
        {
            self.end_turn(arena);
        }
    }

    fn should_end_combat(&self, arena: &EntityArena) -> bool {
        self.initiative
            .iter()
            .copied()
            .filter(|id| actor_can_act(arena, *id))
            .count()
            <= 1
    }
}

fn chebyshev(a: GridPos, b: GridPos) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

fn consume_one_arrow(n: &mut NarrativeState) {
    let Some(i) = n.inventory.stacks.iter().position(|s| {
        s.id == "arrow" && matches!(s.equipped, Some(StackEquipped::Quiver)) && s.count > 0
    }) else {
        return;
    };
    n.inventory.stacks[i].count = n.inventory.stacks[i].count.saturating_sub(1);
    if n.inventory.stacks[i].count == 0 {
        n.inventory.stacks.remove(i);
    }
}

fn actor_can_act(arena: &EntityArena, id: EntityId) -> bool {
    arena.is_alive(id) && arena.stats(id).is_some_and(|s| s.hp > 0)
}

fn next_u32(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    (*seed >> 32) as u32
}

fn roll_d20(seed: &mut u64) -> u16 {
    (next_u32(seed) % 20 + 1) as u16
}

pub fn move_cost_units(from: GridPos, to: GridPos) -> Option<u16> {
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    if dx == 0 && dy == 0 {
        return None;
    }
    if dx > 1 || dy > 1 {
        return None;
    }
    if dx == 1 && dy == 1 {
        return Some(MOVE_DIAGONAL_COST_UNITS);
    }
    Some(MOVE_ORTHOGONAL_COST_UNITS)
}

pub fn speed_modifier(speed: u16) -> i16 {
    (speed / 2) as i16
}

pub fn ap_from_speed(speed: u16) -> u16 {
    (speed / 2).max(1)
}

pub fn strength_modifier(strength: u16) -> i16 {
    (strength / 2) as i16
}

pub fn agility_modifier(agility: u16) -> i16 {
    (agility / 2) as i16
}

pub fn armor_class(agility: u16) -> u16 {
    (10 + agility_modifier(agility)).max(1) as u16
}

pub fn damage_on_hit(strength: u16) -> u16 {
    (1 + strength_modifier(strength)).max(1) as u16
}

#[cfg(test)]
mod tests {
    use super::{
        ap_from_speed, armor_class, AttackStyle, CombatAction, CombatRuleset, CombatState,
        EncounterOutcomePolicy, EncounterProfile, MELEE_HIT_TICKS,
    };
    use crate::entity::{ActorStats, EntityArena, GridPos};

    fn strong_arena() -> (EntityArena, crate::entity::EntityId, crate::entity::EntityId) {
        let mut arena = EntityArena::new();
        let col = crate::render::Color::rgb(200, 200, 200);
        let a = arena.spawn(GridPos { x: 1, y: 1 }, 'a', col, "A".into(), true, None, None, false);
        let b = arena.spawn(GridPos { x: 2, y: 1 }, 'b', col, "B".into(), true, None, None, false);
        arena.set_stats(a, ActorStats::from_full(10, 10, 100, 5, 8));
        arena.set_stats(b, ActorStats::from_full(20, 20, 2, 0, 4));
        (arena, a, b)
    }

    fn lethal_state(arena: &EntityArena, a: crate::entity::EntityId, b: crate::entity::EntityId, seed: &mut u64) -> CombatState {
        let mut state = CombatState::from_participants(
            vec![a, b],
            arena,
            10,
            10,
            seed,
            EncounterProfile { ruleset: CombatRuleset::Lethal, outcome_policy: EncounterOutcomePolicy::None },
        );
        state.turn_index = state.initiative.iter().position(|id| *id == a).unwrap();
        state.ap_remaining[state.turn_index] = 200;
        state
    }

    #[test]
    fn speed_drives_ap() {
        assert_eq!(ap_from_speed(1), 1);
        assert_eq!(ap_from_speed(2), 1);
        assert_eq!(ap_from_speed(5), 2);
        assert_eq!(ap_from_speed(10), 5);
    }

    #[test]
    fn armor_class_scales_with_agility() {
        assert_eq!(armor_class(2), 11);
        assert_eq!(armor_class(8), 14);
    }

    /// Damage must be deferred: HP must not change immediately after `apply_action`.
    #[test]
    fn attack_damage_is_deferred_not_immediate() {
        let (mut arena, a, b) = strong_arena();
        let hp_before = arena.stats(b).unwrap().hp;
        let mut seed = 7;
        let mut state = lethal_state(&arena, a, b, &mut seed);

        let report = state.apply_action(
            CombatAction::Attack { target: b, style: AttackStyle::Unarmed },
            &mut arena,
            &mut seed,
            |_, _| false,
            None,
            None,
        );

        assert!(report.applied, "attack must be applied");
        assert!(!report.end_combat, "combat must not end immediately (damage deferred)");
        assert!(report.pending_hit.is_some(), "attack must produce a pending hit");
        // HP must NOT have changed yet.
        assert_eq!(arena.stats(b).unwrap().hp, hp_before, "HP must not change until pending hit fires");
    }

    /// Melee delay must use MELEE_HIT_TICKS.
    #[test]
    fn melee_attack_has_haptic_delay() {
        let (mut arena, a, b) = strong_arena();
        let mut seed = 42;
        let mut state = lethal_state(&arena, a, b, &mut seed);

        let report = state.apply_action(
            CombatAction::Attack { target: b, style: AttackStyle::Unarmed },
            &mut arena,
            &mut seed,
            |_, _| false,
            None,
            None,
        );

        let hit = report.pending_hit.expect("melee must produce a pending hit");
        assert_eq!(hit.delay_ticks, MELEE_HIT_TICKS);
    }

    /// Ranged attack delay must scale with Chebyshev distance.
    #[test]
    fn ranged_delay_scales_with_distance() {
        use super::{PROJECTILE_MIN_TICKS, PROJECTILE_TICKS_PER_CELL};
        use crate::world::{MapGrid, TileTable};

        let col = crate::render::Color::rgb(200, 200, 200);
        let mut arena = EntityArena::new();
        let a = arena.spawn(GridPos { x: 0, y: 0 }, 'a', col, "A".into(), true, None, None, false);
        let b = arena.spawn(GridPos { x: 5, y: 0 }, 'b', col, "B".into(), true, None, None, false);
        arena.set_stats(a, ActorStats::from_full(10, 10, 100, 0, 8));
        arena.set_stats(b, ActorStats::from_full(20, 20, 2, 0, 4));

        let mut seed = 99;
        let mut state = CombatState::from_participants(
            vec![a, b],
            &arena,
            20, 20,
            &mut seed,
            EncounterProfile { ruleset: CombatRuleset::Lethal, outcome_policy: EncounterOutcomePolicy::None },
        );
        state.turn_index = state.initiative.iter().position(|id| *id == a).unwrap();
        state.ap_remaining[state.turn_index] = 200;

        // Build a minimal passable map for LOS check.
        // Use the default pack so `blocks_sight` resolves correctly (empty table = walls).
        let table = TileTable::default_pack().expect("default pack must be available");
        // Ground tile 0 is the first def's idx; use it as a passable floor.
        let floor_id = table.defs.first().map_or(0, |d| d.idx);
        let map = MapGrid::filled(20, 20, floor_id, table);

        // Equip arrows in a narrative state so the bow fires.
        let mut narrative = crate::narrative::NarrativeState::default();
        narrative.inventory.stacks.push(crate::item::ItemStack {
            id: "arrow".into(),
            count: 5,
            equipped: Some(crate::item::StackEquipped::Quiver),
        });

        let report = state.apply_action(
            CombatAction::Attack {
                target: b,
                style: AttackStyle::Bow { to_hit: 2, damage_bonus: 1, range: 10 },
            },
            &mut arena,
            &mut seed,
            |_, _| false,
            Some(&map),
            Some(&mut narrative),
        );

        assert!(report.applied, "ranged attack must apply");
        let hit = report.pending_hit.expect("ranged must produce pending hit");
        let expected = PROJECTILE_MIN_TICKS.max(5u8.saturating_mul(PROJECTILE_TICKS_PER_CELL));
        assert_eq!(hit.delay_ticks, expected, "delay must scale with Chebyshev dist=5");
    }

    /// Existing sanity-check: attack committed + AP consumed (HP deferred).
    #[test]
    fn attack_commits_and_consumes_ap() {
        let (mut arena, a, b) = strong_arena();
        let mut seed = 7;
        let mut state = lethal_state(&arena, a, b, &mut seed);
        let ap_before = state.ap_remaining[state.turn_index];

        let report = state.apply_action(
            CombatAction::Attack { target: b, style: AttackStyle::Unarmed },
            &mut arena,
            &mut seed,
            |_, _| false,
            None,
            None,
        );

        assert!(report.applied);
        assert!(
            state.ap_remaining[state.turn_index] < ap_before
                || state.turn_index != state.initiative.iter().position(|id| *id == a).unwrap_or(0),
            "AP must have been consumed or turn must have advanced"
        );
    }
}
