//! Turn-based combat core (initiative, AP economy, movement, melee attacks).

use serde::{Deserialize, Serialize};

use crate::entity::{EntityArena, EntityId, GridPos};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatAction {
    Move { target: GridPos },
    Attack { target: EntityId },
    Pass,
    Flee,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatActionReport {
    pub applied: bool,
    pub end_combat: bool,
    pub message: Option<String>,
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
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)).then_with(|| a.0 .0.cmp(&b.0 .0)));
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
                self.ap_remaining[idx] = arena.stats(id).map_or(1, |s| ap_from_speed(s.speed)) * ACTION_UNIT;
                return;
            }
        }
    }

    pub fn apply_action(
        &mut self,
        action: CombatAction,
        arena: &mut EntityArena,
        rng_seed: &mut u64,
        map_blocks: impl Fn(i32, i32) -> bool,
    ) -> CombatActionReport {
        let Some(actor) = self.current_actor() else {
            return CombatActionReport {
                applied: false,
                end_combat: true,
                message: Some("Combat has no active participants.".into()),
            };
        };
        if !actor_can_act(arena, actor) {
            self.end_turn(arena);
            return CombatActionReport {
                applied: false,
                end_combat: self.should_end_combat(arena),
                message: Some("Current actor cannot act.".into()),
            };
        }
        match action {
            CombatAction::Move { target } => {
                let Some(actor_pos) = arena.pos(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Actor has no position.".into()),
                    };
                };
                let Some(move_cost) = move_cost_units(actor_pos, target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Movement target must be adjacent.".into()),
                    };
                };
                if self.current_ap_units().unwrap_or(0) < move_cost {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Not enough AP to move.".into()),
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
                    };
                }
                let blocked = map_blocks(target.x, target.y);
                if !arena.can_move_to(blocked, target, Some(actor)) {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Movement target is blocked.".into()),
                    };
                }
                arena.set_pos(actor, target);
                self.consume_current_ap_units(move_cost);
                self.advance_turn_if_needed(arena);
                CombatActionReport {
                    applied: true,
                    end_combat: self.should_end_combat(arena),
                    message: None,
                }
            }
            CombatAction::Attack { target } => {
                if self.current_ap_units().unwrap_or(0) < ATTACK_COST_UNITS {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Not enough AP to attack.".into()),
                    };
                }
                if target == actor || !self.contains_actor(target) || !actor_can_act(arena, target) {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Invalid combat target.".into()),
                    };
                }
                let Some(attacker_pos) = arena.pos(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Attacker has no position.".into()),
                    };
                };
                let Some(target_pos) = arena.pos(target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target has no position.".into()),
                    };
                };
                let dx = (attacker_pos.x - target_pos.x).abs();
                let dy = (attacker_pos.y - target_pos.y).abs();
                if dx.max(dy) != 1 {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target must be adjacent for melee attack.".into()),
                    };
                }
                let Some(attacker_stats) = arena.stats(actor) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Attacker stats missing.".into()),
                    };
                };
                let Some(target_stats) = arena.stats(target) else {
                    return CombatActionReport {
                        applied: false,
                        end_combat: false,
                        message: Some("Target stats missing.".into()),
                    };
                };
                let attack_roll = i32::from(roll_d20(rng_seed)) + i32::from(strength_modifier(attacker_stats.strength));
                let armor_class = i32::from(armor_class(target_stats.agility));
                let mut message = String::from("Attack misses.");
                if attack_roll >= armor_class {
                    let damage = damage_on_hit(attacker_stats.strength);
                    if let Some(target_mut) = arena.stats_mut(target) {
                        target_mut.hp = target_mut.hp.saturating_sub(damage);
                    }
                    if let Some(updated) = arena.stats(target) {
                        message = format!("Attack hits for {damage} damage ({} HP left).", updated.hp);
                        if updated.hp == 0 && self.profile.ruleset.is_lethal() {
                            arena.despawn(target);
                            message = "Attack defeats the target.".into();
                        }
                    }
                }
                self.consume_current_ap_units(ATTACK_COST_UNITS);
                self.advance_turn_if_needed(arena);
                CombatActionReport {
                    applied: true,
                    end_combat: self.should_end_combat(arena),
                    message: Some(message),
                }
            }
            CombatAction::Pass => {
                self.end_turn(arena);
                CombatActionReport {
                    applied: true,
                    end_combat: self.should_end_combat(arena),
                    message: None,
                }
            }
            CombatAction::Flee => CombatActionReport {
                applied: true,
                end_combat: true,
                message: Some("Flee attempt ends combat.".into()),
            }
        }
    }

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
        ap_from_speed, armor_class, CombatAction, CombatRuleset, CombatState, EncounterOutcomePolicy,
        EncounterProfile,
    };
    use crate::entity::{ActorStats, EntityArena, GridPos};

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

    #[test]
    fn attack_reduces_hp_and_can_end_combat() {
        let mut arena = EntityArena::new();
        let a = arena.spawn(
            GridPos { x: 1, y: 1 },
            'a',
            crate::render::Color::rgb(220, 160, 120),
            "A".into(),
            true,
            None,
            None,
            false,
        );
        let b = arena.spawn(
            GridPos { x: 2, y: 1 },
            'b',
            crate::render::Color::rgb(220, 160, 120),
            "B".into(),
            true,
            None,
            None,
            false,
        );
        arena.set_stats(a, ActorStats::from_full(10, 10, 100, 5, 8));
        arena.set_stats(b, ActorStats::from_full(2, 2, 2, 0, 4));
        let mut seed = 7;
        let mut state = CombatState::from_participants(
            vec![a, b],
            &arena,
            10,
            10,
            &mut seed,
            EncounterProfile {
                ruleset: CombatRuleset::Lethal,
                outcome_policy: EncounterOutcomePolicy::None,
            },
        );
        state.turn_index = state
            .initiative
            .iter()
            .position(|id| *id == a)
            .expect("attacker must exist in initiative");
        let target = b;
        state.ap_remaining[state.turn_index] = 100;
        let report = state.apply_action(
            CombatAction::Attack { target },
            &mut arena,
            &mut seed,
            |_x, _y| false,
        );
        assert!(report.applied);
        assert!(
            report.end_combat
                || arena
                    .stats(target)
                    .is_some_and(|stats| stats.hp < stats.max_hp),
            "expected damage or combat end after attack"
        );
    }
}
