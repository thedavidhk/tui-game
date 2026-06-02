//! Spell definitions and casting logic.
//!
//! ## Adding a new spell
//! 1. Add a variant to [`SpellKind`].
//! 2. Implement a `SpellDef` constant (range, radius, damage, cooldown, visual).
//! 3. Write a `cast_<name>` free function; call it from [`cast_spell`].
//!
//! Spell state (cooldowns, charges) lives on [`super::Game`] — keep it minimal until a
//! full spell/ability system is warranted.

use serde::{Deserialize, Serialize};

use crate::entity::GridPos;
use crate::game::Game;
use crate::math::{euclidean_dist_sq, lcg_next_u32, within_euclidean_radius};
use crate::render::area_effects::{AreaEffect, AreaEffectKind};
use crate::render::Color;

// ── Spell catalogue ───────────────────────────────────────────────────────────

/// All spells the player can cast. Extend freely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellKind {
    Fireball,
}

/// Static properties of a spell; all tunable constants are collected here.
pub struct SpellDef {
    pub name: &'static str,
    /// Max Euclidean distance from the caster to the target center.
    pub range: i32,
    /// Euclidean radius of the area of effect.
    pub aoe_radius: u8,
    /// Base damage applied to every entity in the area of effect.
    pub damage_base: u16,
    /// Additional random damage: rolled as `0..damage_random` and added to base.
    pub damage_random: u16,
    /// Ticks before the same spell can be cast again.
    pub cooldown_ticks: u16,
    /// How long the visual area effect lingers (ticks).
    pub effect_duration_ticks: u32,
    /// Peak visual strength of the area effect (`0..=255`).
    pub effect_strength: u8,
}

pub const FIREBALL: SpellDef = SpellDef {
    name: "Fireball",
    range: 15,
    aoe_radius: 5,
    damage_base: 3,
    damage_random: 5, // roll 0..5, so total 3–7
    cooldown_ticks: 90, // ~1.5 s at 60 Hz
    effect_duration_ticks: 120, // ~2 s lingering fire
    effect_strength: 200,
};

/// Return the static definition for a spell kind.
#[must_use]
pub fn def(kind: SpellKind) -> &'static SpellDef {
    match kind {
        SpellKind::Fireball => &FIREBALL,
    }
}

// ── Targeting helpers ─────────────────────────────────────────────────────────

/// Whether `target` is within casting range of `caster` for `spell`.
#[must_use]
pub fn in_range(caster: GridPos, target: GridPos, spell: SpellKind) -> bool {
    within_euclidean_radius(caster, target, def(spell).range)
}

/// Color used to tint cells inside the area-of-effect preview circle.
pub const AOE_PREVIEW_COLOR: Color = Color::rgb(240, 140, 30);

// ── Casting ───────────────────────────────────────────────────────────────────

/// Attempt to cast `spell` at `target`; returns `Err` with a log message on failure.
///
/// Validates range and cooldown; on success, applies area-of-effect damage, triggers the visual
/// area effect, and starts the cooldown.
///
/// # Errors
/// Returns a static error message if the target is out of range or the spell is on cooldown.
pub fn cast_spell(
    game: &mut Game,
    spell: SpellKind,
    target: GridPos,
) -> Result<(), &'static str> {
    let Some(caster_pos) = game.player_pos() else {
        return Err("No player to cast.");
    };
    let d = def(spell);
    if !in_range(caster_pos, target, spell) {
        return Err("Target is out of range.");
    }
    if game.fireball_cooldown_ticks > 0 {
        return Err("Spell is still on cooldown.");
    }

    match spell {
        SpellKind::Fireball => cast_fireball(game, target, d),
    }
    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
fn cast_fireball(game: &mut Game, target: GridPos, d: &SpellDef) {
    // Damage all entities in AoE (circular).
    let r = i64::from(d.aoe_radius);
    let r_sq = r * r;
    let mut hit_count = 0u32;

    // Collect targets first to avoid borrowing issues.
    let targets: Vec<crate::entity::EntityId> = (0..game.entities.alive.len())
        .filter_map(|i| {
            if !game.entities.alive[i] {
                return None;
            }
            // Entity count fits u32 in any plausible game world.
            let eid = crate::entity::EntityId(i as u32);
            // Don't damage the player.
            if Some(eid) == game.player_id() {
                return None;
            }
            let pos = game.entities.position[i]?;
            if euclidean_dist_sq(pos.x - target.x, pos.y - target.y) <= r_sq {
                Some(eid)
            } else {
                None
            }
        })
        .collect();

    for eid in targets {
        let damage = d.damage_base
            + if d.damage_random > 0 {
                // Remainder fits u16 because it is bounded by damage_random (u16).
                (lcg_next_u32(&mut game.rng_seed) % u32::from(d.damage_random)) as u16
            } else {
                0
            };
        if let Some(stats) = game.entities.stats_mut(eid) {
            stats.hp = stats.hp.saturating_sub(damage);
            let survived = stats.hp > 0;
            let name = game.entities.name.get(eid.0 as usize).cloned().unwrap_or_default();
            let msg = if survived {
                format!("Fireball hits {name} for {damage} damage.")
            } else {
                format!("Fireball defeats {name}!")
            };
            game.log.push(msg);
            if !survived {
                game.entities.despawn(eid);
            }
        }
        hit_count += 1;
    }

    if hit_count == 0 {
        game.log.push("Fireball erupts — no targets hit.".into());
    }

    // Trigger lingering fire visual.
    game.trigger_area_effect(
        AreaEffect {
            center: target,
            radius: d.aoe_radius,
            strength: d.effect_strength,
            kind: AreaEffectKind::Fire,
            phase: 0,
        },
        d.effect_duration_ticks,
    );

    game.fireball_cooldown_ticks = d.cooldown_ticks;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::input::InputBatch;

    fn pos(x: i32, y: i32) -> GridPos {
        GridPos { x, y }
    }

    #[test]
    fn in_range_accepts_cells_within_range() {
        let range = def(SpellKind::Fireball).range;
        assert!(in_range(pos(0, 0), pos(range, 0), SpellKind::Fireball));
        assert!(in_range(pos(0, 0), pos(0, 0), SpellKind::Fireball));
        assert!(!in_range(pos(0, 0), pos(range + 1, 0), SpellKind::Fireball));
        // Euclidean range is circular, not square (aspect-aware).
        assert!(!in_range(pos(0, 0), pos(range + 1, range + 1), SpellKind::Fireball));
    }

    #[test]
    fn fireball_damages_entities_in_aoe() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player_pos = game.player_pos().expect("player exists");
        // Place a wolf adjacent (inside AoE when targeting the same cell as wolf).
        let wolf = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, k)| k.as_deref() == Some("wolf"))
            .map(|(i, _)| crate::entity::EntityId(i as u32))
            .expect("wolf entity must exist");
        // Put wolf 2 tiles east of player (inside range and inside AoE radius 2).
        game.entities.set_pos(wolf, GridPos { x: player_pos.x + 2, y: player_pos.y });
        let hp_before = game.entities.stats(wolf).unwrap().hp;
        let target = GridPos { x: player_pos.x + 2, y: player_pos.y };
        cast_spell(&mut game, SpellKind::Fireball, target).expect("cast must succeed");
        let hp_after = game.entities.stats(wolf).map(|s| s.hp).unwrap_or(0);
        assert!(hp_after < hp_before, "wolf should take damage from fireball");
    }

    #[test]
    fn fireball_ignores_entities_outside_aoe() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player_pos = game.player_pos().expect("player exists");
        let wolf = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, k)| k.as_deref() == Some("wolf"))
            .map(|(i, _)| crate::entity::EntityId(i as u32))
            .expect("wolf entity must exist");
        // Place wolf 6 tiles away; target fireball at player position (dist = 6 > aoe_radius 2).
        game.entities.set_pos(wolf, GridPos { x: player_pos.x + 6, y: player_pos.y });
        let hp_before = game.entities.stats(wolf).unwrap().hp;
        cast_spell(&mut game, SpellKind::Fireball, player_pos).expect("cast must succeed");
        let hp_after = game.entities.stats(wolf).unwrap().hp;
        assert_eq!(hp_after, hp_before, "wolf outside AoE should be unharmed");
    }

    #[test]
    fn cooldown_prevents_immediate_recast() {
        let mut game = Game::new_bootstrapped(80, 30);
        let target = game.player_pos().expect("player exists");
        cast_spell(&mut game, SpellKind::Fireball, target).expect("first cast must succeed");
        let result = cast_spell(&mut game, SpellKind::Fireball, target);
        assert!(result.is_err(), "second cast must fail while on cooldown");
    }

    #[test]
    fn cooldown_expires_after_ticks() {
        let mut game = Game::new_bootstrapped(80, 30);
        let target = game.player_pos().expect("player exists");
        cast_spell(&mut game, SpellKind::Fireball, target).expect("first cast must succeed");
        let cd = FIREBALL.cooldown_ticks;
        for _ in 0..=cd {
            game.step(&InputBatch::default());
        }
        assert_eq!(game.fireball_cooldown_ticks, 0, "cooldown must expire after enough ticks");
        cast_spell(&mut game, SpellKind::Fireball, target).expect("cast after cooldown must succeed");
    }

    #[test]
    fn fireball_spawns_fire_area_effect() {
        let mut game = Game::new_bootstrapped(80, 30);
        let target = game.player_pos().expect("player exists");
        let effects_before = game.active_area_effects.len();
        cast_spell(&mut game, SpellKind::Fireball, target).expect("cast must succeed");
        assert_eq!(
            game.active_area_effects.len(),
            effects_before + 1,
            "fireball must spawn a fire area effect"
        );
    }
}
