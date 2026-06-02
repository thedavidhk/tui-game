//! Top-level game state, mode stack, and stepping.

mod effects;
mod hud;
mod key_commands;
mod modes;
mod overlay_layout;
pub mod projectile;
pub(crate) mod services;
pub mod spell;
mod view;

pub use key_commands::{
    default_game_key_map, key_layer_for_mode, GameCommand, GameInput, GameKeyMap, KeyMapLayer,
};

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::combat::{
    AttackStyle, CombatAction, CombatRuleset, CombatState, EncounterOutcomePolicy,
    EncounterProfile, PendingHit, ATTACK_COST_UNITS, MELEE_HIT_TICKS, MOVE_ORTHOGONAL_COST_UNITS,
    PROJECTILE_MIN_TICKS, PROJECTILE_TICKS_PER_CELL,
};
use crate::content::{ContentPack, DialogueAction, HostileTriggerDef, NpcRoutineDef, Relation};
use crate::entity::{ActorStats, EntityArena, EntityId, GridPos};
use crate::game_content;
use crate::input::{InputBatch, InputEvent, MouseCell};
use crate::item::{Inventory, ItemStack, StackEquipped, WeaponKind};
use crate::level::{
    derive_visual_seed, derive_visual_seed_from_map, level_from_ron,
    materialize_tile_defs_from_pack, LevelFile,
};
use crate::narrative::NarrativeState;
use crate::rect::Rect;
use crate::render::{Color, FrameBuffer, FrameSample};
use crate::ui::hit::UiHitState;
use crate::ui::layout::GameShellLayout;
use crate::ui::viewport_scroll::{
    edge_scroll_pan_delta, map_larger_than_view, screen_cell_to_world, world_view_origin,
    EDGE_SCROLL_COOLDOWN_TICKS,
};
use crate::world::{
    compute_visible, effective_fow_radius_cells, first_step_on_line, merge_explored, mix64,
    plan_path, plan_path_player_fow, rebuild_atmosphere_bake, FogBakedTrio, MapGrid,
};

const FOW_RADIUS: i32 = 20;
const NPC_EXPLORATION_AI_COOLDOWN_TICKS: u16 = 6;

/// `(weight, glyph)`; weights sum to **256** (lighter / speck glyphs more common than heavy blocks).
const FOG_GLYPH_WEIGHTS: &[(u16, char)] = &[(166, ' '), (39, '·'), (27, '░'), (16, '▒'), (8, '▓')];

#[inline]
fn weighted_fog_glyph(r: u8) -> char {
    let x = u16::from(r);
    let mut hi = 0u16;
    for &(w, ch) in FOG_GLYPH_WEIGHTS {
        hi += w;
        if x < hi {
            return ch;
        }
    }
    '░'
}

/// Deterministic mist glyph per world cell: scrambled hash + [`weighted_fog_glyph`].
#[inline]
fn unseen_fog_glyph(wx: i32, wy: i32, level_seed: u64) -> char {
    let a = wx as i64 as u64;
    let b = wy as i64 as u64;
    let mut h = a
        .wrapping_add(b.rotate_left(37))
        .wrapping_mul(0xD134_2543_DE82_EF97)
        ^ b.wrapping_add(a.rotate_left(17))
        ^ level_seed.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h = mix64(h);
    h ^= mix64(a.rotate_left(11) ^ b.wrapping_mul(0x85EB_CA6B) ^ level_seed.rotate_left(5));
    h = mix64(h);
    let r = ((h ^ h >> 17 ^ h >> 34 ^ h >> 51) & 0xFF) as u8;
    weighted_fog_glyph(r)
}

#[derive(Clone, Debug)]
struct PendingForcedDialogue {
    npc: EntityId,
    node_id: String,
}

#[derive(Clone, Copy, Debug)]
struct PendingPlayerAction {
    command: services::actions::ActionCommand,
}

fn player_default_stats() -> ActorStats {
    ActorStats::from_full(24, 24, 7, 6, 20)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFocus {
    Player,
    Container,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    MainMenu {
        selected: usize,
    },
    Exploration,
    Dialogue {
        npc_entity: EntityId,
        dialogue_id: String,
        node_index: usize,
        choice_cursor: usize,
    },
    Inventory {
        cursor: usize,
    },
    Journal {
        quest_cursor: usize,
    },
    ItemTransfer {
        container: EntityId,
        focus: TransferFocus,
        cursor_player: usize,
        cursor_container: usize,
    },
    Combat(CombatState),
    /// Player is aiming a spell: cursor moves around the world, preview shown, Enter confirms.
    SpellTargeting {
        spell: spell::SpellKind,
        /// Current aim cursor in world coordinates.
        cursor: GridPos,
    },
    /// Lethal defeat: world frozen until the player returns to the main menu.
    GameOver,
}

#[inline]
fn inventory_stack_display_line(cat: &crate::item::ItemCatalog, s: &ItemStack) -> String {
    let label = cat.display_name(s.id.as_str());
    match s.equipped {
        Some(StackEquipped::Wear(slot)) => format!("{label} ({slot})"),
        Some(StackEquipped::Quiver) => format!("{label} x{} (Quiver)", s.count),
        None => format!("{label} x{}", s.count),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameModeStack {
    pub stack: Vec<GameMode>,
}

impl GameModeStack {
    pub fn push(&mut self, m: GameMode) {
        self.stack.push(m);
    }

    pub fn pop(&mut self) -> Option<GameMode> {
        self.stack.pop()
    }

    pub fn current(&self) -> Option<&GameMode> {
        self.stack.last()
    }

    pub fn current_mut(&mut self) -> Option<&mut GameMode> {
        self.stack.last_mut()
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub modes: GameModeStack,
    pub map: MapGrid,
    pub entities: EntityArena,
    pub explored: Vec<bool>,
    pub visible: Vec<bool>,
    pub narrative: NarrativeState,
    pub content: ContentPack,
    pub rng_seed: u64,
    pub debug_overlay: bool,
    pub viewport_w: u16,
    pub viewport_h: u16,
    pub log: Vec<String>,
    pub menu_items: Vec<&'static str>,
    pub quit_requested: bool,
    /// Last-frame mouse hit targets (menu rows, dialogue choices, …).
    pub ui_hits: UiHitState,
    /// Last reported pointer cell (used for hover highlights during `compose`).
    pub last_mouse_cell: Option<MouseCell>,
    pub last_perf: Option<FrameSample>,
    /// Seed for baked static tile variants (grass); stable for the loaded level / save.
    pub map_visual_seed: u64,
    /// Increments each [`Game::step`] for animated terrain (water).
    pub surface_tick: u64,
    player_walk_path: Vec<GridPos>,
    player_walk_goal: Option<GridPos>,
    player_walk_tick_cooldown: u16,
    /// When > 0, NPC combat AI waits (same tick pacing as exploration auto-walk).
    npc_combat_ai_tick_cooldown: u16,
    /// Shared pacing gate for exploration NPC movement.
    npc_exploration_ai_tick_cooldown: u16,
    /// Last mouse cell over the world view while exploring (for hover text).
    pub exploration_hover_cell: Option<MouseCell>,
    /// Last mouse cell over the world view during combat (for hover text).
    pub combat_hover_cell: Option<MouseCell>,
    /// Last pointer cell while cursor was inside the world panel (exploration or combat).
    last_world_pointer_cell: Option<MouseCell>,
    /// Extra pan added to the player-centered camera (world tiles); clamped when computing origin.
    view_pan_offset: (i32, i32),
    /// Counts down between edge-scroll steps while the pointer stays in the margin.
    viewport_edge_scroll_cooldown: u16,
    pending_forced_dialogue: Option<PendingForcedDialogue>,
    pending_player_action: Option<PendingPlayerAction>,
    /// When set, [`Game::restart_new_game`] reloads this `.ron`; when `None`, uses the embedded demo level.
    pub restart_level_ron_path: Option<String>,
    /// Baked fog colors per cell ([`rebuild_atmosphere_bake`]); rebuilt with the map display cache.
    pub atmosphere_bake: Vec<FogBakedTrio>,
    /// Key chord → [`GameCommand`] tables ([`GameKeyMap`]); normally supplied by the shell binary.
    pub key_map: GameKeyMap,
    /// In-flight arrows / melee flashes being rendered this frame (not saved).
    pub active_projectiles: Vec<projectile::Projectile>,
    /// Attacks whose damage fires after the animation completes (not saved).
    pub pending_hits: Vec<PendingHit>,
    /// World-space area effects currently alive (fire, poison cloud, magic aura, …).
    /// Not serialized; re-triggered by game events or restored from level data on load.
    pub active_area_effects: Vec<effects::ActiveAreaEffect>,
    /// Ticks until the fireball spell can be cast again (`0` = ready).
    pub fireball_cooldown_ticks: u16,
}

impl Game {
    fn maybe_region_id_for_pos(&self, x: i32, y: i32) -> Option<&'static str> {
        let w = i32::from(self.map.width);
        let h = i32::from(self.map.height);
        // Northeast quarter: tavern approach (scales with map size).
        if x * 4 >= w * 3 && y * 4 <= h {
            return Some("tavern_approach");
        }
        None
    }

    fn dialogue_visible_choice_indices(&self, node: &crate::content::DialogueNode) -> Vec<usize> {
        node.choices
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let base_ok = self.narrative.requires_met(c.requires);
                let fn_ok = c.requires_fn.map_or_else(|| true, |f| f(&self.narrative));
                (base_ok && fn_ok).then_some(i)
            })
            .collect()
    }

    pub fn new_bootstrapped(viewport_w: u16, viewport_h: u16) -> Self {
        Self::new_bootstrapped_with_keymap(viewport_w, viewport_h, default_game_key_map())
    }

    /// Same as [`Self::new_bootstrapped`], but uses the supplied [`GameKeyMap`] (for binaries).
    pub fn new_bootstrapped_with_keymap(
        viewport_w: u16,
        viewport_h: u16,
        key_map: GameKeyMap,
    ) -> Self {
        let level = game_content::embedded_demo_level();
        let mut game = Self::from_level_file(&level, viewport_w, viewport_h, key_map, None)
            .expect("built-in default village level must load");
        game.modes = GameModeStack {
            stack: vec![GameMode::MainMenu { selected: 0 }],
        };
        game.rng_seed = 1;
        game.log =
            vec!["Welcome. LMB interact, WASD move, I/J inventory & journal, F1 debug.".into()];
        game.refresh_fow();
        game
    }

    pub fn from_level_file(
        level: &LevelFile,
        viewport_w: u16,
        viewport_h: u16,
        key_map: GameKeyMap,
        terrain_pack_base: Option<&Path>,
    ) -> Result<Self, String> {
        let content = game_content::content_pack();
        content.validate().map_err(|e| e.to_string())?;
        let mut level = level.clone();
        materialize_tile_defs_from_pack(&mut level, terrain_pack_base)
            .map_err(|e| e.to_string())?;
        content.validate_level(&level).map_err(|e| e.to_string())?;
        let map_visual_seed = level
            .visual_seed
            .unwrap_or_else(|| derive_visual_seed(&level));
        let mut map = level.to_map()?;
        map.rebuild_display_cache(map_visual_seed);
        let n = (map.width as usize) * (map.height as usize);
        let mut atmosphere_bake = Vec::new();
        rebuild_atmosphere_bake(&map, &mut atmosphere_bake);
        let mut entities = EntityArena::new();
        for s in &level.spawns {
            let bp = content.blueprint(s.kind.as_str()).ok_or_else(|| {
                format!(
                    "internal error: missing blueprint for spawn kind {:?}",
                    s.kind
                )
            })?;
            let npc = if bp.is_actor {
                Some(s.kind.clone())
            } else {
                None
            };
            let item = bp.world_item.map(|id| ItemStack::new(id, 1));
            let blocks_movement = if item.is_some() {
                false
            } else if bp.is_container {
                true
            } else {
                npc.is_some()
            };
            let eid = entities.spawn(
                GridPos { x: s.x, y: s.y },
                s.glyph_override.unwrap_or(bp.default_glyph),
                s.fg_override
                    .unwrap_or_else(|| bp.default_fg.to_render_color()),
                s.name_override
                    .clone()
                    .unwrap_or_else(|| bp.default_label.to_string()),
                blocks_movement,
                npc,
                item,
                bp.is_container,
            );
            let stats = content.blueprint_stats(s.kind.as_str()).unwrap_or_default();
            entities.set_stats(eid, stats);
        }
        let (px, py) = if let Some(p) = level.player_spawn {
            (p.x, p.y)
        } else {
            ((map.width / 2) as i32, (map.height / 2) as i32)
        };
        if !map.in_bounds(px, py) {
            return Err(format!(
                "player start ({px},{py}) is outside the map ({}×{})",
                map.width, map.height
            ));
        }
        if map.blocks_movement(px, py) {
            return Err(format!(
                "player start ({px},{py}) must be a walkable tile (not solid)"
            ));
        }
        let player = entities.spawn(
            GridPos { x: px, y: py },
            '@',
            Color::rgb(255, 235, 180),
            "You".into(),
            false,
            None,
            None,
            false,
        );
        entities.set_player(player);
        entities.set_stats(player, player_default_stats());

        let mut game = Self {
            modes: GameModeStack {
                stack: vec![GameMode::Exploration],
            },
            map,
            entities,
            explored: vec![false; n],
            visible: vec![false; n],
            narrative: NarrativeState::default(),
            content,
            rng_seed: 2,
            debug_overlay: false,
            viewport_w,
            viewport_h,
            log: vec![format!("Loaded level: {}", level.name)],
            menu_items: vec!["Start game", "Quit"],
            quit_requested: false,
            ui_hits: UiHitState::default(),
            last_mouse_cell: None,
            last_perf: None,
            map_visual_seed,
            surface_tick: 0,
            player_walk_path: Vec::new(),
            player_walk_goal: None,
            player_walk_tick_cooldown: 0,
            npc_combat_ai_tick_cooldown: 0,
            npc_exploration_ai_tick_cooldown: 0,
            exploration_hover_cell: None,
            combat_hover_cell: None,
            last_world_pointer_cell: None,
            view_pan_offset: (0, 0),
            viewport_edge_scroll_cooldown: 0,
            pending_forced_dialogue: None,
            pending_player_action: None,
            restart_level_ron_path: None,
            atmosphere_bake,
            key_map,
            active_projectiles: Vec::new(),
            pending_hits: Vec::new(),
            active_area_effects: Vec::new(),
            fireball_cooldown_ticks: 0,
        };
        game.seed_demo_weapon_chest();
        game.refresh_fow();
        Ok(game)
    }

    /// Remember a level file path so "Start game" after game over (or similar) reloads the same level.
    pub fn set_restart_level_ron_path(&mut self, path: Option<String>) {
        self.restart_level_ron_path = path;
    }

    /// Full world reset from the embedded demo or [`Self::restart_level_ron_path`], then exploration mode.
    pub fn restart_new_game(&mut self) -> Result<(), String> {
        let stored_path = self.restart_level_ron_path.clone();
        let level = if let Some(ref p) = stored_path {
            let raw = fs::read_to_string(p).map_err(|e| format!("read {p}: {e}"))?;
            level_from_ron(&raw).map_err(|e| e.to_string())?
        } else {
            game_content::embedded_demo_level()
        };
        let vw = self.viewport_w;
        let vh = self.viewport_h;
        let pack_base = stored_path
            .as_ref()
            .and_then(|p| Path::new(p.as_str()).parent());
        let mut g = Self::from_level_file(&level, vw, vh, self.key_map, pack_base)?;
        g.restart_level_ron_path = stored_path;
        g.modes.stack = vec![GameMode::Exploration];
        g.log.push("New game.".into());
        *self = g;
        Ok(())
    }

    fn player_id(&self) -> Option<EntityId> {
        self.entities.player
    }

    fn player_pos(&self) -> Option<GridPos> {
        let pid = self.player_id()?;
        self.entities.pos(pid)
    }

    fn player_stats(&self) -> Option<ActorStats> {
        let pid = self.player_id()?;
        self.entities.stats(pid)
    }

    pub fn refresh_fow(&mut self) {
        let Some(p) = self.player_pos() else {
            return;
        };
        let n = (self.map.width as usize) * (self.map.height as usize);
        if self.visible.len() < n {
            self.visible.resize(n, false);
        }
        if self.explored.len() < n {
            self.explored.resize(n, false);
        }
        let radius = effective_fow_radius_cells(FOW_RADIUS, &self.map.default_atmosphere);
        compute_visible(&self.map, p.x, p.y, radius, &mut self.visible);
        merge_explored(&self.map, &self.visible, &mut self.explored);
    }

    pub fn try_move_player(&mut self, dx: i32, dy: i32) {
        self.clear_player_walk();
        let _ = self.try_move_player_step(dx, dy);
    }

    fn try_move_player_step(&mut self, dx: i32, dy: i32) -> bool {
        let Some(pid) = self.player_id() else {
            return false;
        };
        let Some(p) = self.entities.pos(pid) else {
            return false;
        };
        let nx = p.x + dx;
        let ny = p.y + dy;
        if !self.map.in_bounds(nx, ny) {
            return false;
        }
        if self.map.blocks_movement(nx, ny) {
            return false;
        }
        if let Some(occ) = self.entities.first_npc_at(nx, ny) {
            if occ != pid {
                let can_talk = self
                    .entities
                    .npc_kind
                    .get(occ.0 as usize)
                    .and_then(|k| k.as_deref())
                    .and_then(|kind| self.content.blueprint(kind))
                    .is_some_and(|bp| bp.dialogue_id.is_some());
                if can_talk {
                    self.start_dialogue(occ);
                }
                return false;
            }
        }
        for oid in self.entities.occupants_at(nx, ny) {
            if oid == pid {
                continue;
            }
            if self.entities.blocks_movement[oid.0 as usize] {
                return false;
            }
        }
        self.entities.set_pos(pid, GridPos { x: nx, y: ny });
        self.try_pickup_ground_items(pid);
        if let Some(region_id) = self.maybe_region_id_for_pos(nx, ny) {
            if let Err(e) = self.content.runtime_hooks.on_region_enter(
                region_id,
                &mut self.narrative,
                &mut self.log,
            ) {
                self.log.push(format!("Region hook failed: {e:?}"));
            }
        }
        self.refresh_fow();
        true
    }

    pub(crate) fn try_set_player_walk_goal_from_screen(&mut self, cell: MouseCell) {
        let Some(goal) = self.screen_cell_to_world(cell) else {
            return;
        };
        self.try_set_player_walk_goal(goal);
    }

    pub(crate) fn try_set_player_walk_goal(&mut self, goal: GridPos) {
        let Some(start) = self.player_pos() else {
            return;
        };
        let Some(pid) = self.player_id() else {
            return;
        };
        if start == goal {
            self.clear_player_walk();
            return;
        }
        let explored = self.explored.clone();
        let plan = plan_path_player_fow(
            &self.map,
            &self.entities,
            &explored,
            start,
            goal,
            Some(pid),
            true,
            u32::MAX,
        );
        let Ok(plan) = plan else {
            self.log.push("No viable path to target.".into());
            self.clear_player_walk();
            return;
        };
        let mut path = plan.path;
        if !path.is_empty() && path[0] == start {
            let _ = path.remove(0);
        }
        if path.is_empty() {
            self.clear_player_walk();
            return;
        }
        self.player_walk_path = path;
        self.player_walk_goal = Some(goal);
        if !plan.reached_goal && self.debug_overlay {
            self.log
                .push("Target unreachable; walking to closest reachable point.".into());
        }
    }

    fn step_player_walk(&mut self) {
        if !matches!(self.modes.current(), Some(GameMode::Exploration)) {
            self.clear_player_walk();
            return;
        }
        if self.player_walk_tick_cooldown > 0 {
            self.player_walk_tick_cooldown = self.player_walk_tick_cooldown.saturating_sub(1);
            return;
        }
        if self.player_walk_path.is_empty() {
            self.player_walk_goal = None;
            return;
        }
        let Some(current) = self.player_pos() else {
            self.clear_player_walk();
            return;
        };
        let waypoint = self.player_walk_path[0];
        if waypoint == current {
            let _ = self.player_walk_path.remove(0);
            if self.player_walk_path.is_empty() {
                self.player_walk_goal = None;
            }
            return;
        }
        let Some(next) = first_step_on_line(current, waypoint) else {
            self.replan_walk_goal();
            return;
        };
        let dx = next.x - current.x;
        let dy = next.y - current.y;
        let moved = self.try_move_player_step(dx, dy);
        if moved {
            if next == waypoint {
                let _ = self.player_walk_path.remove(0);
            }
            self.player_walk_tick_cooldown = self.player_stats().map_or(0, |stats| {
                services::pacing::visual_step_cooldown_ticks_from_speed(stats.speed)
            });
            if self.player_walk_path.is_empty() {
                self.player_walk_goal = None;
            }
            return;
        }
        self.replan_walk_goal();
    }

    fn replan_walk_goal(&mut self) {
        let goal = self.player_walk_goal;
        self.player_walk_path.clear();
        if let Some(goal) = goal {
            self.try_set_player_walk_goal(goal);
        }
    }

    fn clear_player_walk(&mut self) {
        self.player_walk_path.clear();
        self.player_walk_goal = None;
        self.player_walk_tick_cooldown = 0;
        self.pending_player_action = None;
    }

    pub(crate) fn screen_cell_to_world(&self, cell: MouseCell) -> Option<GridPos> {
        let world_rect = self.world_rect_for_viewport();
        let origin = self.world_screen_origin();
        screen_cell_to_world(cell, world_rect, origin, self.map.width, self.map.height)
    }

    pub(crate) fn world_rect_for_viewport(&self) -> Rect {
        let (world, _, _) = GameShellLayout::root_panels(self.viewport_w, self.viewport_h);
        world
    }

    #[must_use]
    pub(crate) fn world_view_needs_pan(&self) -> bool {
        let r = self.world_rect_for_viewport();
        map_larger_than_view(self.map.width, self.map.height, r.w, r.h)
    }

    fn world_screen_origin(&self) -> (i32, i32) {
        let r = self.world_rect_for_viewport();
        let Some(pid) = self.player_id() else {
            return (0, 0);
        };
        let Some(p) = self.entities.pos(pid) else {
            return (0, 0);
        };
        world_view_origin(
            p,
            self.view_pan_offset,
            self.map.width,
            self.map.height,
            r.w,
            r.h,
        )
    }

    pub(crate) fn nudge_view_pan(&mut self, dx: i32, dy: i32) {
        self.view_pan_offset.0 += dx;
        self.view_pan_offset.1 += dy;
    }

    pub(crate) fn register_world_pointer(&mut self, cell: MouseCell, world_r: Rect) {
        if world_r.contains(cell.x, cell.y) {
            self.last_world_pointer_cell = Some(cell);
        } else {
            self.last_world_pointer_cell = None;
        }
    }

    fn tick_viewport_edge_scroll(&mut self) {
        if !matches!(
            self.modes.current(),
            Some(GameMode::Exploration) | Some(GameMode::Combat(_))
        ) {
            self.last_world_pointer_cell = None;
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        let world_r = self.world_rect_for_viewport();
        let Some(cell) = self.last_world_pointer_cell else {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        };
        if !world_r.contains(cell.x, cell.y) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if !self.world_view_needs_pan() {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        let lx = i32::from(cell.x.saturating_sub(world_r.x));
        let ly = i32::from(cell.y.saturating_sub(world_r.y));
        let (pdx, pdy) = edge_scroll_pan_delta(lx, ly, world_r.w, world_r.h);
        if (pdx, pdy) == (0, 0) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if self.viewport_edge_scroll_cooldown > 0 {
            self.viewport_edge_scroll_cooldown =
                self.viewport_edge_scroll_cooldown.saturating_sub(1);
            return;
        }
        self.nudge_view_pan(pdx, pdy);
        self.viewport_edge_scroll_cooldown = EDGE_SCROLL_COOLDOWN_TICKS;
    }

    fn try_pickup_ground_items(&mut self, player: EntityId) {
        let Some(p) = self.entities.pos(player) else {
            return;
        };
        let occupants = self.entities.occupants_at(p.x, p.y);
        for eid in occupants {
            if eid == player {
                continue;
            }
            if self.entities.npc_kind[eid.0 as usize].is_some() {
                continue;
            }
            if self.entities.is_container[eid.0 as usize] {
                continue;
            }
            let Some(stack) = self.entities.item[eid.0 as usize].clone() else {
                continue;
            };
            self.narrative.inventory.add(stack.id.clone(), stack.count);
            if let Err(e) = self.content.runtime_hooks.on_item_picked(
                stack.id.as_str(),
                &mut self.narrative,
                &mut self.log,
            ) {
                self.log.push(format!("Pickup hook failed: {e:?}"));
            }
            self.log
                .push(format!("Picked up {} x{}.", stack.id, stack.count));
            self.entities.despawn(eid);
        }
    }

    fn start_dialogue(&mut self, npc: EntityId) {
        let kind = self.entities.npc_kind[npc.0 as usize]
            .clone()
            .unwrap_or_default();
        let tree = self
            .content
            .dialogues
            .get(kind.as_str())
            .copied()
            .unwrap_or(self.content.default_dialogue);
        let start_node =
            self.content
                .runtime_hooks
                .dialogue_start_node(kind.as_str(), tree, &self.narrative);
        self.push_dialogue_mode(npc, kind, start_node, tree);
    }

    fn start_dialogue_at_node(&mut self, npc: EntityId, node_id: &str) {
        let kind = self.entities.npc_kind[npc.0 as usize]
            .clone()
            .unwrap_or_default();
        let tree = self
            .content
            .dialogues
            .get(kind.as_str())
            .copied()
            .unwrap_or(self.content.default_dialogue);
        let start_node = tree
            .node_index(node_id)
            .or_else(|| tree.node_index("hub"))
            .unwrap_or(0);
        self.push_dialogue_mode(npc, kind, start_node, tree);
    }

    fn push_dialogue_mode(
        &mut self,
        npc: EntityId,
        dialogue_id: String,
        node_index: usize,
        tree: &'static crate::content::DialogueTree,
    ) {
        self.modes.push(GameMode::Dialogue {
            npc_entity: npc,
            dialogue_id,
            node_index,
            choice_cursor: 0,
        });
        self.apply_dialogue_node_effects(tree, node_index);
    }

    fn start_item_transfer(&mut self, container: EntityId) {
        self.narrative
            .container_inventories
            .entry(container.0)
            .or_default();
        self.modes.push(GameMode::ItemTransfer {
            container,
            focus: TransferFocus::Player,
            cursor_player: 0,
            cursor_container: 0,
        });
    }

    /// Demo wooden chest at (89, 31): weapons and arrows for playtesting ranged combat.
    fn seed_demo_weapon_chest(&mut self) {
        const WX: i32 = 89;
        const WY: i32 = 31;
        let Some(cid) = self.entities.first_container_at(WX, WY) else {
            return;
        };
        let mut inv = Inventory::default();
        inv.add("iron_sword", 1);
        inv.add("hunting_bow", 1);
        inv.add("arrow", 24);
        self.narrative.container_inventories.insert(cid.0, inv);
    }

    pub fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            if let InputEvent::Mouse { cell, .. } = ev {
                self.last_mouse_cell = Some(*cell);
            }
            if let Some(gi) = self.resolve_game_input(ev) {
                modes::route(self, gi);
            }
        }
        services::combat::step_npc_combat_ai(self);
        self.step_npc_exploration_ai();
        self.step_player_walk();
        self.pump_pending_player_action();
        self.pump_pending_forced_dialogue();
        self.tick_viewport_edge_scroll();
        let combat_state = self
            .modes
            .current()
            .and_then(|m| {
                if let GameMode::Combat(cs) = m {
                    Some(cs.clone())
                } else {
                    None
                }
            });
        self.tick_effects(combat_state.as_ref());
        self.fireball_cooldown_ticks = self.fireball_cooldown_ticks.saturating_sub(1);
        self.surface_tick = self.surface_tick.wrapping_add(1);
    }

    fn pump_pending_forced_dialogue(&mut self) {
        if !matches!(self.modes.current(), Some(GameMode::Exploration)) {
            return;
        }
        if let Some(p) = self.pending_forced_dialogue.take() {
            if self.entities.is_alive(p.npc) {
                self.start_dialogue_at_node(p.npc, p.node_id.as_str());
            }
        }
    }

    fn schedule_training_spar_epilogue(&mut self, state: &CombatState) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let EncounterOutcomePolicy::TrainingSpar { trainer } = state.profile.outcome_policy else {
            return;
        };
        if trainer == pid {
            return;
        }
        let Some(ps) = self.entities.stats(pid) else {
            return;
        };
        let Some(ts) = self.entities.stats(trainer) else {
            return;
        };
        let node_id = self
            .content
            .runtime_hooks
            .training_spar_epilogue_node(ps.hp, ts.hp);
        self.pending_forced_dialogue = Some(PendingForcedDialogue {
            npc: trainer,
            node_id: node_id.into(),
        });
    }

    fn hostile_trigger_met(&self, eid: EntityId, trigger: HostileTriggerDef) -> bool {
        let Some(pp) = self.player_pos() else {
            return false;
        };
        let Some(ep) = self.entities.pos(eid) else {
            return false;
        };
        match trigger {
            HostileTriggerDef::PlayerWithinChebyshev { range } => {
                crate::math::chebyshev(pp, ep) <= i32::from(range)
            }
        }
    }

    fn step_npc_exploration_ai(&mut self) {
        if !matches!(self.modes.current(), Some(GameMode::Exploration)) {
            self.npc_exploration_ai_tick_cooldown = 0;
            return;
        }
        if services::combat::detect_hostile_encounters(self) {
            self.npc_exploration_ai_tick_cooldown = 0;
            return;
        }
        if self.npc_exploration_ai_tick_cooldown > 0 {
            self.npc_exploration_ai_tick_cooldown =
                self.npc_exploration_ai_tick_cooldown.saturating_sub(1);
            return;
        }

        let mut moved_any = false;
        for i in 0..self.entities.alive.len() {
            if !self.entities.alive[i] {
                continue;
            }
            let eid = EntityId(i as u32);
            if self.player_id() == Some(eid) {
                continue;
            }
            let Some(kind) = self.entities.npc_kind.get(i).and_then(|k| k.as_deref()) else {
                continue;
            };
            let Some(bp) = self.content.blueprint(kind) else {
                continue;
            };
            let Some(from) = self.entities.pos(eid) else {
                continue;
            };
            let mut brain = self.entities.npc_brain.get(i).copied().unwrap_or_default();
            let next = match bp.behavior.routine {
                NpcRoutineDef::Idle => None,
                routine => crate::ai::exploration::next_exploration_step(
                    eid,
                    from,
                    routine,
                    &mut brain,
                    &self.map,
                    &self.entities,
                    &mut self.rng_seed,
                ),
            };
            if let Some(slot) = self.entities.npc_brain.get_mut(i) {
                *slot = brain;
            }
            let Some(target) = next else {
                continue;
            };
            if self.entities.can_move_to(
                self.map.blocks_movement(target.x, target.y),
                target,
                Some(eid),
            ) {
                self.entities.set_pos(eid, target);
                moved_any = true;
            }
        }
        self.npc_exploration_ai_tick_cooldown = if moved_any {
            NPC_EXPLORATION_AI_COOLDOWN_TICKS
        } else {
            0
        };
    }

    fn resolve_game_input(&self, ev: &InputEvent) -> Option<GameInput> {
        match ev {
            InputEvent::Key(ch) => {
                let layer = key_layer_for_mode(self.modes.current());
                self.key_map.resolve(layer, *ch).map(GameInput::Command)
            }
            InputEvent::Mouse { .. } => Some(GameInput::Raw(ev.clone())),
            InputEvent::Resize { .. } => None,
        }
    }

    pub(crate) fn handle_menu(&mut self, ev: GameInput, selected: usize) {
        modes::menu::handle(self, ev, selected);
    }

    pub(crate) fn handle_explore(&mut self, ev: GameInput) {
        modes::exploration::handle(self, ev);
    }

    pub(crate) fn toggle_turn_based(&mut self) {
        services::combat::toggle_turn_based(self);
    }

    fn relation_to_player(&self, target: EntityId) -> Relation {
        services::relation::relation_to_player(self, target)
    }

    fn action_target_pos(&self, command: services::actions::ActionCommand) -> Option<GridPos> {
        match command {
            services::actions::ActionCommand::Talk { target }
            | services::actions::ActionCommand::OpenContainer { target }
            | services::actions::ActionCommand::EngageCombat { target, .. }
            | services::actions::ActionCommand::Attack { target } => self.entities.pos(target),
            services::actions::ActionCommand::MoveTo { target } => Some(target),
        }
    }

    fn action_requirements_met(
        &self,
        actor: EntityId,
        command: services::actions::ActionCommand,
    ) -> bool {
        let req = services::actions::requirements_for(command);
        if !matches!(req.target, services::actions::TargetRequirement::None) {
            match command {
                services::actions::ActionCommand::Talk { target }
                | services::actions::ActionCommand::OpenContainer { target }
                | services::actions::ActionCommand::EngageCombat { target, .. }
                | services::actions::ActionCommand::Attack { target } => match req.target {
                    services::actions::TargetRequirement::EntityAlive => {
                        if !self.entities.is_alive(target) {
                            return false;
                        }
                    }
                    services::actions::TargetRequirement::Container => {
                        if !self
                            .entities
                            .is_container
                            .get(target.0 as usize)
                            .copied()
                            .unwrap_or(false)
                        {
                            return false;
                        }
                    }
                    services::actions::TargetRequirement::HostileToPlayer => {
                        if !services::relation::is_hostile_to_player(self, target) {
                            return false;
                        }
                    }
                    services::actions::TargetRequirement::None => {}
                },
                services::actions::ActionCommand::MoveTo { .. } => {}
            }
        }
        let Some(actor_pos) = self.entities.pos(actor) else {
            return false;
        };
        match req.range {
            services::actions::RangeRequirement::Adjacent => self
                .action_target_pos(command)
                .is_some_and(|target| crate::math::chebyshev(actor_pos, target) <= 1),
            services::actions::RangeRequirement::TalkRadius => {
                self.action_target_pos(command).is_some_and(|target| {
                    crate::math::manhattan(actor_pos, target)
                        <= services::hover::TALK_RANGE_MANHATTAN
                })
            }
            services::actions::RangeRequirement::OccupyTile => self
                .action_target_pos(command)
                .is_some_and(|target| actor_pos == target),
        }
    }

    fn execute_player_action_immediate(
        &mut self,
        actor: EntityId,
        command: services::actions::ActionCommand,
    ) -> bool {
        if !self.action_requirements_met(actor, command) {
            return false;
        }
        match command {
            services::actions::ActionCommand::Talk { target } => {
                self.start_dialogue(target);
                true
            }
            services::actions::ActionCommand::OpenContainer { target } => {
                self.start_item_transfer(target);
                true
            }
            services::actions::ActionCommand::EngageCombat { target, profile } => {
                services::combat::start_combat_encounter(
                    self,
                    vec![actor, target],
                    profile,
                    "Combat started. LMB attack/move, RMB march, Enter/Space pass, f flee, Esc or q quit.",
                );
                true
            }
            services::actions::ActionCommand::MoveTo { target } => {
                self.try_set_player_walk_goal(target);
                true
            }
            services::actions::ActionCommand::Attack { .. } => false,
        }
    }

    fn queue_or_execute_player_action(
        &mut self,
        request: services::actions::ActionRequest,
    ) -> services::actions::ActionStopReason {
        if self.execute_player_action_immediate(request.actor, request.command) {
            self.pending_player_action = None;
            return services::actions::ActionStopReason::ReachedRange;
        }
        let Some(goal) = self.action_target_pos(request.command) else {
            return services::actions::ActionStopReason::Interrupted;
        };
        self.try_set_player_walk_goal(goal);
        if self.player_walk_path.is_empty() {
            return services::actions::ActionStopReason::NoPath;
        }
        self.pending_player_action = Some(PendingPlayerAction {
            command: request.command,
        });
        services::actions::ActionStopReason::Blocked
    }

    fn pump_pending_player_action(&mut self) {
        if !matches!(self.modes.current(), Some(GameMode::Exploration)) {
            self.pending_player_action = None;
            return;
        }
        let Some(pending) = self.pending_player_action else {
            return;
        };
        let Some(pid) = self.player_id() else {
            self.pending_player_action = None;
            return;
        };
        if self.execute_player_action_immediate(pid, pending.command) {
            self.pending_player_action = None;
        }
    }

    pub(crate) fn try_exploration_primary_click(&mut self, cell: MouseCell) {
        if !matches!(self.modes.current(), Some(GameMode::Exploration)) {
            return;
        }
        let Some(wp) = self.screen_cell_to_world(cell) else {
            return;
        };
        let Some(pid) = self.player_id() else {
            return;
        };

        for &eid in &self.entities.occupants_at(wp.x, wp.y) {
            if eid == pid {
                continue;
            }
            if self.entities.is_container[eid.0 as usize] {
                let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
                    initiator: services::actions::ActionInitiator::Player,
                    actor: pid,
                    command: services::actions::ActionCommand::OpenContainer { target: eid },
                });
                return;
            }
        }

        let Some(target) = self.entities.first_npc_at(wp.x, wp.y) else {
            return;
        };
        if target == pid {
            return;
        }
        let Some(kind) = self.entities.npc_kind[target.0 as usize].as_deref() else {
            return;
        };
        let Some(bp) = self.content.blueprint(kind) else {
            return;
        };
        if matches!(self.relation_to_player(target), Relation::Hostile) {
            let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
                initiator: services::actions::ActionInitiator::Player,
                actor: pid,
                command: services::actions::ActionCommand::EngageCombat {
                    target,
                    profile: EncounterProfile {
                        ruleset: CombatRuleset::Lethal,
                        outcome_policy: EncounterOutcomePolicy::None,
                    },
                },
            });
            return;
        }
        if bp.dialogue_id.is_some() {
            let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
                initiator: services::actions::ActionInitiator::Player,
                actor: pid,
                command: services::actions::ActionCommand::Talk { target },
            });
            return;
        }

        let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
            initiator: services::actions::ActionInitiator::Player,
            actor: pid,
            command: services::actions::ActionCommand::MoveTo { target: wp },
        });
    }

    fn start_training_spar(&mut self, trainer: EntityId) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let participants = vec![pid, trainer];
        self.start_combat_encounter(
            participants,
            EncounterProfile {
                ruleset: CombatRuleset::NonLethalSpar,
                outcome_policy: EncounterOutcomePolicy::TrainingSpar { trainer },
            },
            "Training spar started. Non-lethal rules are active.",
        );
    }

    fn start_combat_encounter(
        &mut self,
        participants: Vec<EntityId>,
        profile: EncounterProfile,
        message: &str,
    ) {
        if participants.len() < 2 {
            self.log
                .push("Need at least two actors to start combat.".into());
            return;
        }
        let state = CombatState::from_participants(
            participants,
            &self.entities,
            self.map.width,
            self.map.height,
            &mut self.rng_seed,
            profile,
        );
        self.modes.push(GameMode::Combat(state));
        self.log.push(message.into());
    }

    pub(crate) fn handle_dialogue(&mut self, ev: GameInput) {
        modes::dialogue::handle(self, ev);
    }

    fn apply_dialogue_continue(
        &mut self,
        tree: &'static crate::content::DialogueTree,
        exit_sentinel: usize,
    ) {
        let Some(GameMode::Dialogue {
            dialogue_id,
            node_index,
            ..
        }) = self.modes.current().cloned()
        else {
            return;
        };
        let Some(node) = tree.nodes.get(node_index) else {
            let _ = self.modes.pop();
            return;
        };
        if node.id == "_intro" || node.id == "_greet" {
            self.narrative
                .mark_dialogue_intro_seen(dialogue_id.as_str());
        }
        let Some(next) = node.auto_next else {
            return;
        };
        if next == exit_sentinel {
            let _ = self.modes.pop();
            return;
        }
        if let Some(GameMode::Dialogue {
            node_index: ni,
            choice_cursor: cc,
            ..
        }) = self.modes.current_mut()
        {
            *ni = next;
            *cc = 0;
        }
        self.apply_dialogue_node_effects(tree, next);
    }

    pub(crate) fn apply_dialogue_choice(
        &mut self,
        tree: &'static crate::content::DialogueTree,
        exit_sentinel: usize,
    ) {
        let Some(GameMode::Dialogue {
            dialogue_id: _,
            npc_entity,
            node_index,
            choice_cursor,
            ..
        }) = self.modes.current().cloned()
        else {
            return;
        };
        let Some(node) = tree.nodes.get(node_index) else {
            let _ = self.modes.pop();
            return;
        };
        let visible = self.dialogue_visible_choice_indices(node);
        let Some(raw_idx) = visible.get(choice_cursor).copied() else {
            return;
        };
        let Some(choice) = node.choices.get(raw_idx) else {
            return;
        };
        if let Err(e) = self.narrative.apply_effects(&mut self.log, choice.effects) {
            self.log.push(format!("Dialogue effect failed: {e:?}"));
            return;
        }
        if let Some(effects_fn) = choice.effects_fn {
            if let Err(e) = effects_fn(&mut self.narrative, &mut self.log) {
                self.log.push(format!("Dialogue custom effect failed: {e}"));
                return;
            }
        }
        let trigger_training_spar =
            matches!(choice.action, Some(DialogueAction::StartTrainingSpar));
        let next = choice.next;
        if next == exit_sentinel {
            let _ = self.modes.pop();
            if trigger_training_spar {
                self.start_training_spar(npc_entity);
            }
            return;
        }
        if let Some(GameMode::Dialogue {
            node_index: ni,
            choice_cursor: cc,
            ..
        }) = self.modes.current_mut()
        {
            *ni = next;
            *cc = 0;
        }
        self.apply_dialogue_node_effects(tree, next);
        if trigger_training_spar {
            let _ = self.modes.pop();
            self.start_training_spar(npc_entity);
        }
    }

    fn apply_dialogue_node_effects(
        &mut self,
        tree: &'static crate::content::DialogueTree,
        node_index: usize,
    ) {
        let Some(node) = tree.nodes.get(node_index) else {
            return;
        };
        if node.effects.is_empty() {
            return;
        }
        if let Err(e) = self.narrative.apply_effects(&mut self.log, node.effects) {
            self.log.push(format!("Dialogue node effect failed: {e:?}"));
        }
    }

    pub(crate) fn handle_journal(&mut self, ev: GameInput) {
        modes::journal::handle(self, ev);
    }

    pub(crate) fn handle_inventory(&mut self, ev: GameInput) {
        modes::inventory::handle(self, ev);
    }

    pub(crate) fn handle_item_transfer(&mut self, ev: GameInput) {
        modes::transfer::handle(self, ev);
    }

    pub(crate) fn handle_combat(&mut self, ev: GameInput, state: CombatState) {
        modes::combat::handle(self, ev, state);
    }

    fn attack_style_for_entity(&self, actor: EntityId) -> AttackStyle {
        if self.player_id() != Some(actor) {
            return AttackStyle::Unarmed;
        }
        let Some(id) = self.narrative.main_hand_item_id() else {
            return AttackStyle::Unarmed;
        };
        let Some(def) = self.content.item_catalog().get(id) else {
            return AttackStyle::Unarmed;
        };
        match def.weapon {
            None => AttackStyle::Unarmed,
            Some(WeaponKind::Melee {
                to_hit,
                damage_bonus,
            }) => AttackStyle::Melee {
                to_hit,
                damage_bonus,
            },
            Some(WeaponKind::RangedBow {
                to_hit,
                damage_bonus,
                range,
            }) => {
                let has_arrows = self.narrative.quiver_count_for_ranged("arrow") > 0;
                if has_arrows {
                    AttackStyle::Bow {
                        to_hit,
                        damage_bonus,
                        range,
                    }
                } else {
                    AttackStyle::Unarmed
                }
            }
        }
    }

    fn combat_apply_attack(&mut self, state: &mut CombatState, target: EntityId) {
        let Some(actor) = state.current_actor() else {
            return;
        };
        let style = self.attack_style_for_entity(actor);
        let map_blocks = |x: i32, y: i32| self.map.blocks_movement(x, y);
        let narrative = if self.player_id() == Some(actor) {
            Some(&mut self.narrative)
        } else {
            None
        };
        let report = state.apply_action(
            CombatAction::Attack { target, style },
            &mut self.entities,
            &mut self.rng_seed,
            map_blocks,
            Some(&self.map),
            narrative,
        );
        self.apply_combat_report(state, report);
    }

    fn combat_player_can_basic_attack_target(
        &self,
        state: &CombatState,
        player: EntityId,
        target: EntityId,
    ) -> bool {
        if !state.contains_actor(target) {
            return false;
        }
        let Some(pp) = self.entities.pos(player) else {
            return false;
        };
        let Some(tp) = self.entities.pos(target) else {
            return false;
        };
        let d = crate::math::chebyshev(pp, tp);
        match self.attack_style_for_entity(player) {
            AttackStyle::Bow { range, .. } => d >= 1 && d <= i32::from(range),
            AttackStyle::Unarmed | AttackStyle::Melee { .. } => d == 1,
        }
    }

    pub(crate) fn combat_try_move(&mut self, state: &mut CombatState, dx: i32, dy: i32) {
        let Some(actor) = state.current_actor() else {
            return;
        };
        let Some(p) = self.entities.pos(actor) else {
            return;
        };
        let target = GridPos {
            x: p.x + dx,
            y: p.y + dy,
        };
        let map_blocks = |x, y| self.map.blocks_movement(x, y);
        let report = state.apply_action(
            CombatAction::Move { target },
            &mut self.entities,
            &mut self.rng_seed,
            map_blocks,
            Some(&self.map),
            None,
        );
        self.apply_combat_report(state, report);
    }

    pub(crate) fn combat_try_attack_target(&mut self, state: &mut CombatState, target: EntityId) {
        let Some(actor) = state.current_actor() else {
            return;
        };
        let Some(p) = self.entities.pos(actor) else {
            return;
        };
        let Some(tp) = self.entities.pos(target) else {
            return;
        };
        let style = self.attack_style_for_entity(actor);
        let dist = crate::math::chebyshev(p, tp);
        let in_range = match style {
            AttackStyle::Unarmed | AttackStyle::Melee { .. } => dist == 1,
            AttackStyle::Bow { range, .. } => dist >= 1 && dist <= i32::from(range),
        };
        if !in_range {
            return;
        }
        self.combat_apply_attack(state, target);
    }

    fn execute_combat_action_command(
        &mut self,
        state: &mut CombatState,
        command: services::actions::ActionCommand,
    ) {
        match command {
            services::actions::ActionCommand::Attack { target } => {
                self.combat_try_attack_target(state, target);
            }
            services::actions::ActionCommand::MoveTo { target } => {
                let Some(pid) = self.player_id() else {
                    return;
                };
                let _ = self.combat_march_toward(state, pid, target, None, 64);
            }
            _ => {}
        }
    }

    /// March the current player toward `goal`, optionally stopping when within `max_chebyshev`
    /// tiles (Chebyshev) of `stop_near_entity`. Returns `true` if combat ended mid-march.
    fn combat_march_toward(
        &mut self,
        state: &mut CombatState,
        player: EntityId,
        goal: GridPos,
        stop_near_entity: Option<(EntityId, i32)>,
        max_steps: u32,
    ) -> bool {
        for _ in 0..max_steps {
            if !matches!(self.modes.current(), Some(GameMode::Combat(_))) {
                return true;
            }
            let Some(actor) = state.current_actor() else {
                return true;
            };
            if actor != player {
                return false;
            }
            let Some(from) = self.entities.pos(player) else {
                return false;
            };
            if let Some((finish_id, max_d)) = stop_near_entity {
                if let Some(tp) = self.entities.pos(finish_id) {
                    if crate::math::chebyshev(from, tp) <= max_d {
                        return false;
                    }
                }
            } else if from == goal {
                return false;
            }
            if state.current_ap_units().unwrap_or(0) < MOVE_ORTHOGONAL_COST_UNITS {
                break;
            }
            let Ok(plan) = plan_path(
                &self.map,
                &self.entities,
                from,
                goal,
                Some(player),
                true,
                u32::MAX,
            ) else {
                break;
            };
            let Some(waypoint) = plan.path.get(1).copied() else {
                break;
            };
            let Some(next) = first_step_on_line(from, waypoint) else {
                break;
            };
            let report = state.apply_action(
                CombatAction::Move { target: next },
                &mut self.entities,
                &mut self.rng_seed,
                |x, y| self.map.blocks_movement(x, y),
                Some(&self.map),
                None,
            );
            let applied = report.applied;
            let ended = report.end_combat;
            self.apply_combat_report(state, report);
            if ended {
                return true;
            }
            if !applied {
                break;
            }
        }
        false
    }

    pub(crate) fn combat_rmb_march_toward(&mut self, state: &mut CombatState, cell: MouseCell) {
        let Some(pid) = self.player_id() else {
            return;
        };
        if state.current_actor() != Some(pid) {
            return;
        }
        let Some(goal) = self.screen_cell_to_world(cell) else {
            return;
        };
        self.execute_combat_action_command(
            state,
            services::actions::ActionCommand::MoveTo { target: goal },
        );
    }

    pub(crate) fn combat_try_primary_click(&mut self, state: &mut CombatState, cell: MouseCell) {
        let Some(pid) = self.player_id() else {
            return;
        };
        if state.current_actor() != Some(pid) {
            return;
        }
        let Some(wp) = self.screen_cell_to_world(cell) else {
            return;
        };

        if let EncounterOutcomePolicy::TrainingSpar { trainer: tid } = state.profile.outcome_policy
        {
            let Some(trainer_pos) = self.entities.pos(tid) else {
                return;
            };
            let stop_dist = match self.attack_style_for_entity(pid) {
                AttackStyle::Bow { range, .. } => i32::from(range),
                _ => 1,
            };
            if self.combat_march_toward(state, pid, trainer_pos, Some((tid, stop_dist)), 64) {
                return;
            }
            if !matches!(self.modes.current(), Some(GameMode::Combat(_))) {
                return;
            }
            if state.current_actor() != Some(pid) {
                return;
            }
            if self.combat_player_can_basic_attack_target(state, pid, tid)
                && state.current_ap_units().unwrap_or(0) >= ATTACK_COST_UNITS
            {
                self.execute_combat_action_command(
                    state,
                    services::actions::ActionCommand::Attack { target: tid },
                );
            }
            return;
        }

        for &eid in &self.entities.occupants_at(wp.x, wp.y) {
            if eid == pid || !state.contains_actor(eid) {
                continue;
            }
            if !self.entities.is_alive(eid) {
                continue;
            }
            let Some(epos) = self.entities.pos(eid) else {
                continue;
            };
            let stop_dist = match self.attack_style_for_entity(pid) {
                AttackStyle::Bow { range, .. } => i32::from(range),
                _ => 1,
            };
            if self.combat_march_toward(state, pid, epos, Some((eid, stop_dist)), 64) {
                return;
            }
            if !matches!(self.modes.current(), Some(GameMode::Combat(_))) {
                return;
            }
            if state.current_actor() != Some(pid) {
                return;
            }
            if self.combat_player_can_basic_attack_target(state, pid, eid)
                && state.current_ap_units().unwrap_or(0) >= ATTACK_COST_UNITS
            {
                self.execute_combat_action_command(
                    state,
                    services::actions::ActionCommand::Attack { target: eid },
                );
            }
            return;
        }

        self.execute_combat_action_command(
            state,
            services::actions::ActionCommand::MoveTo { target: wp },
        );
    }

    fn apply_combat_report(
        &mut self,
        state: &CombatState,
        report: crate::combat::CombatActionReport,
    ) {
        if let Some(msg) = report.message {
            self.log.push(msg);
        }
        if let Some(hit) = report.pending_hit {
            self.spawn_attack_effect(hit);
        }
        if report.end_combat {
            services::combat::finish_combat(self, state);
        }
        self.refresh_fow();
    }

    /// Spawn the visual projectile / melee flash for a pending hit and store the deferred damage.
    fn spawn_attack_effect(&mut self, hit: PendingHit) {
        use projectile::{arrow_glyph, Projectile, ARROW_COLOR, MELEE_FLASH_COLOR};

        // Resolve attacker and target positions for the visual.
        let actor = self
            .modes
            .current()
            .and_then(|m| {
                if let GameMode::Combat(cs) = m {
                    cs.current_actor()
                } else {
                    None
                }
            })
            // Fall back to the entity the hit originated from (best-effort).
            .unwrap_or(EntityId(0));

        let from = self.entities.pos(actor).unwrap_or_default();
        let to = self.entities.pos(hit.target).unwrap_or(from);

        let (glyph, color, is_ranged, total_ticks) = if hit.delay_ticks > MELEE_HIT_TICKS {
            // Ranged: direction-aware arrow, flight time proportional to distance.
            let dist = {
                let dx = (to.x - from.x).abs();
                let dy = (to.y - from.y).abs();
                dx.max(dy) as u8
            };
            let ticks = PROJECTILE_MIN_TICKS.max(dist.saturating_mul(PROJECTILE_TICKS_PER_CELL));
            (arrow_glyph(from, to), ARROW_COLOR, true, ticks)
        } else {
            // Melee: fixed-position flash at the target cell.
            ('*', MELEE_FLASH_COLOR, false, MELEE_HIT_TICKS)
        };

        self.active_projectiles.push(Projectile {
            from,
            to,
            ticks_elapsed: 0,
            total_ticks,
            glyph,
            color,
            is_ranged,
        });
        self.pending_hits.push(hit);
    }

    /// Advance all in-flight projectiles, fire pending hits, and expire area effects.
    ///
    /// Called once per [`Game::step`] tick, unconditionally.
    pub(crate) fn tick_effects(&mut self, state: Option<&CombatState>) {
        self.tick_area_effects();

        // Tick projectiles.
        for p in &mut self.active_projectiles {
            p.ticks_elapsed = p.ticks_elapsed.saturating_add(1);
        }
        self.active_projectiles.retain(|p| !p.is_expired());

        if self.pending_hits.is_empty() {
            return;
        }
        // Tick pending hits; collect the ones that fire this tick.
        let mut fired: Vec<PendingHit> = Vec::new();
        for hit in &mut self.pending_hits {
            if hit.delay_ticks == 0 {
                // Already fired on a previous tick (shouldn't happen, but guard anyway).
                continue;
            }
            hit.delay_ticks -= 1;
            if hit.delay_ticks == 0 {
                fired.push(hit.clone());
            }
        }
        self.pending_hits.retain(|h| h.delay_ticks > 0);

        let mut end_combat = false;
        for hit in fired {
            self.log.push(hit.resolved_message.clone());
            if hit.hit && hit.damage > 0 {
                if let Some(stats) = self.entities.stats_mut(hit.target) {
                    stats.hp = stats.hp.saturating_sub(hit.damage);
                    if stats.hp == 0 && hit.lethal {
                        self.entities.despawn(hit.target);
                        end_combat = true;
                    }
                }
            }
        }

        if end_combat {
            if let Some(cs) = state.cloned() {
                services::combat::finish_combat(self, &cs);
            }
        }
    }

    // ── Area effects ─────────────────────────────────────────────────────────

    /// Spawn a world-space area effect.
    ///
    /// `remaining_ticks = u32::MAX` creates a permanent effect (e.g. level-defined fire);
    /// any smaller value expires after that many game ticks.
    pub fn trigger_area_effect(
        &mut self,
        effect: crate::render::area_effects::AreaEffect,
        remaining_ticks: u32,
    ) {
        self.active_area_effects.push(effects::ActiveAreaEffect {
            effect,
            remaining_ticks,
        });
    }

    /// Tick down and expire finite-lifetime area effects. Called from [`Self::tick_effects`].
    fn tick_area_effects(&mut self) {
        for ae in &mut self.active_area_effects {
            if ae.remaining_ticks != u32::MAX {
                ae.remaining_ticks = ae.remaining_ticks.saturating_sub(1);
            }
        }
        self.active_area_effects
            .retain(|ae| ae.remaining_ticks > 0);
    }

    /// Leave combat from the UI (Esc); message is logged before combat state is torn down.
    pub(crate) fn finish_combat_player_quit(&mut self, state: &CombatState) {
        self.log.push("You leave the fight.".into());
        services::combat::finish_combat(self, state);
    }

    pub fn compose(
        &mut self,
        fb: &mut FrameBuffer,
        world_rect: Rect,
        hud_rect: Rect,
        log_rect: Rect,
    ) {
        view::compose(self, fb, world_rect, hud_rect, log_rect);
    }

    pub(crate) fn handle_game_over(&mut self, ev: GameInput) {
        modes::game_over::handle(self, ev);
    }

    pub fn snapshot(&self) -> crate::save::WorldSnapshot {
        crate::save::WorldSnapshot {
            map: self.map.clone(),
            entities: self.entities.clone(),
            narrative: self.narrative.clone(),
            rng_seed: self.rng_seed,
        }
    }

    pub fn save_game(&self) -> Result<crate::save::SaveGameV1, String> {
        Ok(crate::save::SaveGameV1::new(
            self.snapshot(),
            self.modes.clone(),
        ))
    }

    pub fn apply_save(&mut self, mut s: crate::save::SaveGameV1) -> Result<(), String> {
        if s.schema_version > crate::save::SAVE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported save version {} (max {})",
                s.schema_version,
                crate::save::SAVE_SCHEMA_VERSION
            ));
        }
        if s.schema_version < 8 {
            s.world.narrative.migrate_legacy_equipment_into_stacks();
        }
        self.map = s.world.map;
        self.map_visual_seed = derive_visual_seed_from_map(&self.map);
        self.map.rebuild_display_cache(self.map_visual_seed);
        rebuild_atmosphere_bake(&self.map, &mut self.atmosphere_bake);
        self.entities = s.world.entities;
        self.narrative = s.world.narrative;
        self.rng_seed = s.world.rng_seed;
        self.modes = s.modes;
        self.view_pan_offset = (0, 0);
        self.last_world_pointer_cell = None;
        self.viewport_edge_scroll_cooldown = 0;
        self.active_projectiles.clear();
        self.pending_hits.clear();
        self.active_area_effects.clear();
        self.fireball_cooldown_ticks = 0;
        let n = (self.map.width as usize) * (self.map.height as usize);
        self.explored.resize(n, false);
        self.visible.resize(n, false);
        self.refresh_fow();
        Ok(())
    }

    /// Write `SaveGameV1` RON to `path` (for quick iteration; callers may use app dirs later).
    pub fn save_to_path(&self, path: &str) -> Result<(), String> {
        let sg = self.save_game()?;
        let raw = crate::save::save_to_ron(&sg).map_err(|e| e.to_string())?;
        fs::write(path, raw).map_err(|e| e.to_string())
    }

    pub fn load_from_path(&mut self, path: &str) -> Result<(), String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let sg = crate::save::save_from_ron(&raw).map_err(|e| e.to_string())?;
        self.apply_save(sg)?;
        self.log.push(format!("Loaded save from {path}."));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{unseen_fog_glyph, Game, GameCommand, GameInput, GameMode};
    use crate::combat::{CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile};
    use crate::entity::GridPos;
    use crate::input::InputBatch;

    #[test]
    fn fog_glyph_weights_sum_to_256() {
        let s: u16 = super::FOG_GLYPH_WEIGHTS.iter().map(|(w, _)| *w).sum();
        assert_eq!(s, 256);
    }

    #[test]
    fn unseen_fog_glyph_deterministic_from_palette() {
        let seed = 0xC0FFEE_u64;
        assert_eq!(unseen_fog_glyph(7, -2, seed), unseen_fog_glyph(7, -2, seed));
        let c = unseen_fog_glyph(100, 200, seed);
        assert!(
            matches!(c, ' ' | '░' | '▒' | '▓' | '·' | ':' | ','),
            "unexpected glyph {c:?}"
        );
        let mut seed_changes_glyph = false;
        for i in 0..40_i32 {
            if unseen_fog_glyph(i, i.wrapping_mul(7), 1)
                != unseen_fog_glyph(i, i.wrapping_mul(7), 2)
            {
                seed_changes_glyph = true;
                break;
            }
        }
        assert!(
            seed_changes_glyph,
            "map_visual_seed should re-phase the fog texture for some cells"
        );
    }

    #[test]
    fn dialogue_hides_unmet_required_choices() {
        let game = Game::new_bootstrapped(80, 30);
        let tree = game.content.dialogues.get("guide").copied().unwrap();
        let node = tree
            .node_index("hub")
            .and_then(|idx| tree.nodes.get(idx))
            .expect("guide hub node must exist");
        let visible = game.dialogue_visible_choice_indices(node);
        assert_eq!(visible, vec![0, 1, 4]);
    }

    #[test]
    fn dialogue_shows_choice_when_requirements_met() {
        let mut game = Game::new_bootstrapped(80, 30);
        game.narrative.inventory.add("cellar_key", 1);
        game.narrative.journal_set_status(
            crate::game_content::quests::QUEST_GUIDE_FETCH,
            crate::content::QuestJournalStatus::InProgress,
        );
        game.narrative.journal_set_status(
            crate::game_content::quests::QUEST_VILLAGER_HELP,
            crate::content::QuestJournalStatus::Completed,
        );
        let tree = game.content.dialogues.get("guide").copied().unwrap();
        let node = tree
            .node_index("hub")
            .and_then(|idx| tree.nodes.get(idx))
            .expect("guide hub node must exist");
        let visible = game.dialogue_visible_choice_indices(node);
        assert_eq!(visible, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn trainer_dialogue_can_start_training_spar() {
        let mut game = Game::new_bootstrapped(80, 30);
        game.modes.stack = vec![GameMode::Exploration];
        let trainer = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("trainer"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("trainer entity should exist in demo level");
        game.start_dialogue(trainer);
        game.handle_dialogue(GameInput::Command(GameCommand::Confirm));
        game.handle_dialogue(GameInput::Command(GameCommand::Confirm));
        let Some(GameMode::Combat(cs)) = game.modes.current().cloned() else {
            panic!("trainer spar should enter combat mode");
        };
        assert!(matches!(cs.profile.ruleset, CombatRuleset::NonLethalSpar));
    }

    #[test]
    fn training_spar_restores_hp_on_end() {
        let mut game = Game::new_bootstrapped(80, 30);
        game.modes.stack = vec![GameMode::Exploration];
        let trainer = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("trainer"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("trainer entity should exist in demo level");
        game.start_training_spar(trainer);
        let Some(GameMode::Combat(cs)) = game.modes.current().cloned() else {
            panic!("combat mode expected");
        };
        for id in &cs.initiative {
            if let Some(stats) = game.entities.stats_mut(*id) {
                stats.hp = 1;
            }
        }
        crate::game::services::combat::finish_combat(&mut game, &cs);
        for id in &cs.initiative {
            let stats = game
                .entities
                .stats(*id)
                .expect("combatant stats must exist");
            assert_eq!(stats.hp, stats.max_hp);
        }
    }

    #[test]
    fn start_game_from_menu_after_player_death_restarts_world() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player = game.player_id().expect("player must exist");
        game.entities.despawn(player);
        game.modes.stack = vec![GameMode::MainMenu { selected: 0 }];
        game.handle_menu(GameInput::Command(GameCommand::Confirm), 0);
        assert!(matches!(game.modes.current(), Some(GameMode::Exploration)));
        assert!(
            game.player_id()
                .is_some_and(|pid| game.entities.is_alive(pid)),
            "new game should respawn a living player"
        );
    }

    #[test]
    fn lethal_combat_player_death_enters_game_over() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player = game.player_id().expect("player must exist");
        let trainer = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("trainer"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("trainer entity should exist");
        game.entities.set_pos(player, GridPos { x: 10, y: 10 });
        game.entities.set_pos(trainer, GridPos { x: 11, y: 10 });
        let mut rng = 1;
        let cs = CombatState::from_participants(
            vec![trainer, player],
            &game.entities,
            game.map.width,
            game.map.height,
            &mut rng,
            EncounterProfile {
                ruleset: CombatRuleset::Lethal,
                outcome_policy: EncounterOutcomePolicy::None,
            },
        );
        game.modes.stack = vec![GameMode::Exploration];
        game.modes.push(GameMode::Combat(cs.clone()));
        game.entities.despawn(player);
        crate::game::services::combat::finish_combat(&mut game, &cs);
        assert!(matches!(game.modes.current(), Some(GameMode::GameOver)));
    }

    #[test]
    fn player_walk_goal_advances_over_ticks() {
        let mut game = Game::new_bootstrapped(80, 30);
        game.modes.stack = vec![GameMode::Exploration];
        let start = game.player_pos().expect("player position should exist");
        let goal = crate::entity::GridPos {
            x: start.x + 3,
            y: start.y,
        };
        game.try_set_player_walk_goal(goal);
        for _ in 0..48 {
            game.step(&InputBatch::default());
        }
        assert_eq!(game.player_pos(), Some(goal));
    }

    #[test]
    fn npc_combat_ai_attacks_adjacent_player() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player = game.player_id().expect("player must exist");
        let wolf = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("wolf"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("wolf entity should exist");
        game.entities.set_pos(player, GridPos { x: 10, y: 10 });
        game.entities.set_pos(wolf, GridPos { x: 11, y: 10 });
        let mut rng = 1;
        let mut cs = CombatState::from_participants(
            vec![wolf, player],
            &game.entities,
            game.map.width,
            game.map.height,
            &mut rng,
            EncounterProfile {
                ruleset: CombatRuleset::Lethal,
                outcome_policy: EncounterOutcomePolicy::None,
            },
        );
        cs.turn_index = cs
            .initiative
            .iter()
            .position(|id| *id == wolf)
            .expect("wolf should be in initiative");
        cs.ap_remaining[cs.turn_index] = 100;
        game.modes.stack = vec![GameMode::Combat(cs)];
        let before = game.entities.stats(player).expect("player stats").hp;
        // First step: NPC commits attack (AP consumed, pending hit queued).
        game.step(&InputBatch::default());
        // Drain all pending hits by stepping until none remain.
        for _ in 0..20 {
            if game.pending_hits.is_empty() {
                break;
            }
            game.step(&InputBatch::default());
        }
        let after = game.entities.stats(player).map(|s| s.hp).unwrap_or(0);
        assert!(after < before, "npc should attack adjacent player (damage after delay)");
    }

    #[test]
    fn npc_combat_ai_passes_when_adjacent_and_ap_insufficient_to_attack() {
        use crate::combat::ATTACK_COST_UNITS;

        let mut game = Game::new_bootstrapped(80, 30);
        let player = game.player_id().expect("player must exist");
        let wolf = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("wolf"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("wolf entity should exist");
        game.entities.set_pos(player, GridPos { x: 10, y: 10 });
        game.entities.set_pos(wolf, GridPos { x: 11, y: 10 });
        let mut rng = 1;
        let mut cs = CombatState::from_participants(
            vec![wolf, player],
            &game.entities,
            game.map.width,
            game.map.height,
            &mut rng,
            EncounterProfile {
                ruleset: CombatRuleset::Lethal,
                outcome_policy: EncounterOutcomePolicy::None,
            },
        );
        let wolf_turn = cs
            .initiative
            .iter()
            .position(|id| *id == wolf)
            .expect("wolf should be in initiative");
        cs.turn_index = wolf_turn;
        cs.ap_remaining[wolf_turn] = ATTACK_COST_UNITS - 1;
        game.modes.stack = vec![GameMode::Combat(cs)];
        let hp_before = game.entities.stats(player).expect("player stats").hp;
        game.step(&InputBatch::default());
        let hp_after = game.entities.stats(player).expect("player stats").hp;
        assert_eq!(
            hp_before, hp_after,
            "npc must not attack when AP is below attack cost"
        );
        let Some(GameMode::Combat(cs2)) = game.modes.current() else {
            panic!("expected combat to continue");
        };
        assert_eq!(
            cs2.current_actor(),
            Some(player),
            "wolf should pass the turn when unable to attack"
        );
    }

    #[test]
    fn npc_combat_ai_move_pacing_skips_tick_while_cooldown() {
        let mut game = Game::new_bootstrapped(80, 30);
        let player = game.player_id().expect("player must exist");
        let wolf = game
            .entities
            .npc_kind
            .iter()
            .enumerate()
            .find(|(_, kind)| kind.as_deref() == Some("wolf"))
            .map(|(idx, _)| crate::entity::EntityId(idx as u32))
            .expect("wolf entity should exist");
        game.entities.set_pos(player, GridPos { x: 10, y: 10 });
        game.entities.set_pos(wolf, GridPos { x: 14, y: 10 });
        let mut rng = 1;
        let mut cs = CombatState::from_participants(
            vec![wolf, player],
            &game.entities,
            game.map.width,
            game.map.height,
            &mut rng,
            EncounterProfile {
                ruleset: CombatRuleset::Lethal,
                outcome_policy: EncounterOutcomePolicy::None,
            },
        );
        cs.turn_index = cs
            .initiative
            .iter()
            .position(|id| *id == wolf)
            .expect("wolf should be in initiative");
        cs.ap_remaining[cs.turn_index] = 100;
        game.modes.stack = vec![GameMode::Combat(cs)];
        game.step(&InputBatch::default());
        let after_first = game.entities.pos(wolf).expect("wolf position");
        let mut same_ticks = 0u32;
        while game.entities.pos(wolf) == Some(after_first) {
            game.step(&InputBatch::default());
            same_ticks += 1;
            assert!(
                same_ticks < 32,
                "wolf should take another step after visual pacing cooldown"
            );
        }
        assert!(
            same_ticks >= 2,
            "expected at least one pacing tick before second move (speed 7 => 3-tick cooldown at ~60 Hz)"
        );
    }

    #[test]
    fn area_effect_expires_after_lifetime() {
        use crate::entity::GridPos;
        use crate::render::area_effects::{AreaEffect, AreaEffectKind};

        let mut game = Game::new_bootstrapped(80, 30);
        game.trigger_area_effect(
            AreaEffect {
                center: GridPos { x: 5, y: 5 },
                radius: 2,
                strength: 150,
                kind: AreaEffectKind::Fire,
                phase: 0,
            },
            3, // 3 ticks lifetime
        );
        assert_eq!(game.active_area_effects.len(), 1, "effect should be present after trigger");

        game.step(&InputBatch::default()); // tick 1 → remaining = 2
        assert_eq!(game.active_area_effects.len(), 1);
        game.step(&InputBatch::default()); // tick 2 → remaining = 1
        assert_eq!(game.active_area_effects.len(), 1);
        game.step(&InputBatch::default()); // tick 3 → remaining = 0, removed
        assert_eq!(game.active_area_effects.len(), 0, "effect should be removed after lifetime expires");
    }

    #[test]
    fn area_effect_permanent_when_max_ticks() {
        use crate::entity::GridPos;
        use crate::render::area_effects::{AreaEffect, AreaEffectKind};

        let mut game = Game::new_bootstrapped(80, 30);
        game.trigger_area_effect(
            AreaEffect {
                center: GridPos { x: 5, y: 5 },
                radius: 1,
                strength: 100,
                kind: AreaEffectKind::MagicAura {
                    color: crate::render::Color::rgb(100, 50, 200),
                },
                phase: 0,
            },
            u32::MAX,
        );
        for _ in 0..60 {
            game.step(&InputBatch::default());
        }
        assert_eq!(game.active_area_effects.len(), 1, "permanent effect should not expire");
    }
}
