//! Top-level game state, mode stack, and stepping.

mod modes;
pub(crate) mod services;
mod view;

use std::fs;

use serde::{Deserialize, Serialize};

use crate::ai::combat::ChaseNearestPolicy;
use crate::ai::{AiIntent, CombatAiCtx, CombatDecisionPolicy};
use crate::combat::{
    CombatAction, CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile,
    ATTACK_COST_UNITS, MOVE_ORTHOGONAL_COST_UNITS,
};
use crate::content::{ContentPack, DialogueAction, QuestJournalStatus, Relation};
use crate::entity::{ActorStats, EntityArena, EntityId, GridPos};
use crate::game_content;
use crate::input::{InputBatch, InputEvent, MouseCell};
use crate::item::ItemStack;
use crate::level::LevelFile;
use crate::narrative::NarrativeState;
use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, FrameSample, Style};
use crate::ui::hit::UiHitState;
use crate::ui::layout::GameShellLayout;
use crate::ui::viewport_scroll::{
    edge_scroll_pan_delta, map_larger_than_view, screen_cell_to_world, world_view_origin,
    EDGE_SCROLL_COOLDOWN_TICKS,
};
use crate::world::{
    compute_visible, first_step_on_line, merge_explored, plan_path, plan_path_player_fow, MapGrid,
};

const FOW_RADIUS: i32 = 8;

#[derive(Clone, Debug)]
struct PendingForcedDialogue {
    npc: EntityId,
    node_id: String,
}

#[derive(Clone, Copy, Debug)]
struct PendingPlayerAction {
    command: services::actions::ActionCommand,
}

fn explored_muted_fg(base: Color) -> Color {
    const M: u32 = 38;
    const T0: u32 = 90;
    const T1: u32 = 85;
    const T2: u32 = 100;
    Color::rgb(
        (((base.r as u32) * M + T0 * (100 - M)) / 100).min(255) as u8,
        (((base.g as u32) * M + T1 * (100 - M)) / 100).min(255) as u8,
        (((base.b as u32) * M + T2 * (100 - M)) / 100).min(255) as u8,
    )
}

fn player_default_stats() -> ActorStats {
    ActorStats::from_full(24, 24, 7, 6, 6)
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
    pub last_perf: Option<FrameSample>,
    player_walk_path: Vec<GridPos>,
    player_walk_goal: Option<GridPos>,
    player_walk_tick_cooldown: u16,
    /// When > 0, NPC combat AI waits (same tick pacing as exploration auto-walk).
    npc_combat_ai_tick_cooldown: u16,
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
        let level = game_content::embedded_demo_level();
        let mut game = Self::from_level_file(&level, viewport_w, viewport_h)
            .expect("built-in default village level must load");
        game.modes = GameModeStack {
            stack: vec![GameMode::MainMenu { selected: 0 }],
        };
        game.rng_seed = 1;
        game.log = vec!["Welcome. WASD move, E interact, I inventory, J journal, F1 debug.".into()];
        game
    }

    pub fn from_level_file(
        level: &LevelFile,
        viewport_w: u16,
        viewport_h: u16,
    ) -> Result<Self, String> {
        let content = game_content::content_pack();
        content.validate().map_err(|e| e.to_string())?;
        content.validate_level(level).map_err(|e| e.to_string())?;
        let map = level.to_map()?;
        let n = (map.width as usize) * (map.height as usize);
        let mut entities = EntityArena::new();
        for s in &level.spawns {
            let bp = content.blueprint(s.kind.as_str()).ok_or_else(|| {
                format!(
                    "internal error: missing blueprint for spawn kind {:?}",
                    s.kind
                )
            })?;
            let npc = bp.dialogue_id.map(std::string::ToString::to_string);
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
                s.glyph,
                s.name.clone(),
                blocks_movement,
                npc,
                item,
                bp.is_container,
            );
            let stats = content.blueprint_stats(s.kind.as_str()).unwrap_or_default();
            entities.set_stats(eid, stats);
        }
        let player = entities.spawn(
            GridPos {
                x: (map.width / 2) as i32,
                y: (map.height / 2) as i32,
            },
            '@',
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
            last_perf: None,
            player_walk_path: Vec::new(),
            player_walk_goal: None,
            player_walk_tick_cooldown: 0,
            npc_combat_ai_tick_cooldown: 0,
            exploration_hover_cell: None,
            combat_hover_cell: None,
            last_world_pointer_cell: None,
            view_pan_offset: (0, 0),
            viewport_edge_scroll_cooldown: 0,
            pending_forced_dialogue: None,
            pending_player_action: None,
        };
        game.refresh_fow();
        Ok(game)
    }

    fn player_id(&self) -> Option<EntityId> {
        self.entities.player
    }

    fn player_pos(&self) -> Option<GridPos> {
        let pid = self.player_id()?;
        self.entities.pos(pid)
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
        compute_visible(&self.map, p.x, p.y, FOW_RADIUS, &mut self.visible);
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
                self.start_dialogue(occ);
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
            self.player_walk_tick_cooldown = self
                .player_id()
                .and_then(|id| self.entities.stats(id))
                .map_or(0, |stats| {
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
        let (world, _, _) =
            GameShellLayout::root_panels(self.viewport_w, self.viewport_h);
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
        let start_node = self
            .content
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

    pub fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            self.handle_event(ev.clone());
        }
        self.step_npc_combat_ai();
        self.step_player_walk();
        self.pump_pending_player_action();
        self.pump_pending_forced_dialogue();
        self.tick_viewport_edge_scroll();
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

    fn step_npc_combat_ai(&mut self) {
        let Some(GameMode::Combat(state)) = self.modes.current().cloned() else {
            self.npc_combat_ai_tick_cooldown = 0;
            return;
        };
        let Some(actor) = state.current_actor() else {
            return;
        };
        if self.player_id() == Some(actor) {
            self.npc_combat_ai_tick_cooldown = 0;
            return;
        }
        if self.npc_combat_ai_tick_cooldown > 0 {
            self.npc_combat_ai_tick_cooldown =
                self.npc_combat_ai_tick_cooldown.saturating_sub(1);
            return;
        }
        let mut next = state;
        let ai = ChaseNearestPolicy;
        let intent = ai.decide(
            actor,
            &CombatAiCtx {
                state: &next,
                map: &self.map,
                entities: &self.entities,
            },
        );
        let pace_after_success = matches!(
            &intent,
            AiIntent::Combat(CombatAction::Move { .. } | CombatAction::Attack { .. })
        );
        let report = match intent {
            AiIntent::Combat(action) => {
                next.apply_action(action, &mut self.entities, &mut self.rng_seed, |x, y| {
                    self.map.blocks_movement(x, y)
                })
            }
            AiIntent::Wait => next.apply_action(
                CombatAction::Pass,
                &mut self.entities,
                &mut self.rng_seed,
                |_x, _y| false,
            ),
        };
        if report.applied && pace_after_success {
            let speed = self
                .entities
                .stats(actor)
                .map_or(1, |stats| stats.speed);
            self.npc_combat_ai_tick_cooldown =
                services::pacing::visual_step_cooldown_ticks_from_speed(speed);
        }
        self.apply_combat_report(&next, report);
        if let Some(GameMode::Combat(cs)) = self.modes.current_mut() {
            *cs = next;
        }
    }

    fn handle_event(&mut self, ev: InputEvent) {
        modes::route(self, ev);
    }

    pub(crate) fn handle_menu(&mut self, ev: InputEvent, selected: usize) {
        modes::menu::handle(self, ev, selected);
    }

    pub(crate) fn handle_explore(&mut self, ev: InputEvent) {
        modes::exploration::handle(self, ev);
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
                .is_some_and(|target| services::hover::chebyshev(actor_pos, target) <= 1),
            services::actions::RangeRequirement::TalkRadius => self
                .action_target_pos(command)
                .is_some_and(|target| {
                    services::hover::manhattan(actor_pos, target) <= services::hover::TALK_RANGE_MANHATTAN
                }),
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
                self.start_combat_encounter(
                    vec![actor, target],
                    profile,
                    "Combat started. x attack, WASD move, Tab pass, f flee.",
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

    pub(crate) fn try_interact(&mut self) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let Some(p) = self.entities.pos(pid) else {
            return;
        };
        match services::interaction::probe_interaction(&self.entities, p, &self.content) {
            services::interaction::InteractionOutcome::Dialogue(occ) => {
                let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
                    initiator: services::actions::ActionInitiator::Player,
                    actor: pid,
                    command: services::actions::ActionCommand::Talk { target: occ },
                });
            }
            services::interaction::InteractionOutcome::Container(chest) => {
                let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
                    initiator: services::actions::ActionInitiator::Player,
                    actor: pid,
                    command: services::actions::ActionCommand::OpenContainer { target: chest },
                });
            }
            services::interaction::InteractionOutcome::None => {
                self.log.push("Nothing to interact with nearby.".into());
            }
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

    pub(crate) fn try_start_combat(&mut self) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let Some(p) = self.entities.pos(pid) else {
            return;
        };
        let mut target: Option<EntityId> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = p.x + dx;
                let ny = p.y + dy;
                let Some(occ) = self.entities.first_npc_at(nx, ny) else {
                    continue;
                };
                if occ == pid {
                    continue;
                }
                if self.entities.npc_kind[occ.0 as usize].is_none() {
                    continue;
                }
                if matches!(self.relation_to_player(occ), Relation::Hostile) {
                    target = Some(occ);
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        let Some(t) = target else {
            self.log.push("No hostile target nearby.".into());
            return;
        };
        let _ = self.queue_or_execute_player_action(services::actions::ActionRequest {
            initiator: services::actions::ActionInitiator::Player,
            actor: pid,
            command: services::actions::ActionCommand::EngageCombat {
                target: t,
                profile: EncounterProfile {
                    ruleset: CombatRuleset::Lethal,
                    outcome_policy: EncounterOutcomePolicy::None,
                },
            },
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

    pub(crate) fn handle_dialogue(&mut self, ev: InputEvent) {
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
        let trigger_training_spar = matches!(
            choice.action,
            Some(DialogueAction::StartTrainingSpar)
        );
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

    pub(crate) fn handle_journal(&mut self, ev: InputEvent) {
        modes::journal::handle(self, ev);
    }

    pub(crate) fn handle_inventory(&mut self, ev: InputEvent) {
        modes::inventory::handle(self, ev);
    }

    pub(crate) fn handle_item_transfer(&mut self, ev: InputEvent) {
        modes::transfer::handle(self, ev);
    }

    pub(crate) fn handle_combat(&mut self, ev: InputEvent, state: CombatState) {
        modes::combat::handle(self, ev, state);
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
        );
        self.apply_combat_report(state, report);
    }

    pub(crate) fn combat_try_attack(&mut self, state: &mut CombatState) {
        let Some(actor) = state.current_actor() else {
            return;
        };
        let Some(p) = self.entities.pos(actor) else {
            return;
        };
        let target = match services::combat::find_adjacent_target(&self.entities, state, actor, p) {
            services::combat::AttackTargetOutcome::Target(target) => target,
            services::combat::AttackTargetOutcome::NoAdjacentTarget => {
                self.log.push("No adjacent combat target.".into());
                return;
            }
        };
        let report = state.apply_action(
            CombatAction::Attack { target },
            &mut self.entities,
            &mut self.rng_seed,
            |_x, _y| false,
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
        let dx = (p.x - tp.x).abs();
        let dy = (p.y - tp.y).abs();
        if dx.max(dy) != 1 {
            return;
        }
        let report = state.apply_action(
            CombatAction::Attack { target },
            &mut self.entities,
            &mut self.rng_seed,
            |_x, _y| false,
        );
        self.apply_combat_report(state, report);
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

    /// March the current player toward `goal`, optionally stopping when Chebyshev-adjacent to
    /// `stop_when_adjacent_to`. Returns `true` if combat ended mid-march.
    fn combat_march_toward(
        &mut self,
        state: &mut CombatState,
        player: EntityId,
        goal: GridPos,
        stop_when_adjacent_to: Option<EntityId>,
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
            if let Some(finish_id) = stop_when_adjacent_to {
                if let Some(tp) = self.entities.pos(finish_id) {
                    if services::hover::chebyshev(from, tp) <= 1 {
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

        if let EncounterOutcomePolicy::TrainingSpar { trainer: tid } = state.profile.outcome_policy {
            let Some(trainer_pos) = self.entities.pos(tid) else {
                return;
            };
            if self.combat_march_toward(state, pid, trainer_pos, Some(tid), 64) {
                return;
            }
            if !matches!(self.modes.current(), Some(GameMode::Combat(_))) {
                return;
            }
            if state.current_actor() != Some(pid) {
                return;
            }
            let Some(pp) = self.entities.pos(pid) else {
                return;
            };
            let Some(tp) = self.entities.pos(tid) else {
                return;
            };
            if services::hover::chebyshev(pp, tp) <= 1
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
            if self.combat_march_toward(state, pid, epos, Some(eid), 64) {
                return;
            }
            if !matches!(self.modes.current(), Some(GameMode::Combat(_))) {
                return;
            }
            if state.current_actor() != Some(pid) {
                return;
            }
            let Some(pp) = self.entities.pos(pid) else {
                return;
            };
            let Some(ep) = self.entities.pos(eid) else {
                return;
            };
            if services::hover::chebyshev(pp, ep) <= 1
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
        if report.end_combat {
            self.finish_combat(state);
        }
    }

    fn finish_combat(&mut self, state: &CombatState) {
        self.npc_combat_ai_tick_cooldown = 0;
        self.combat_hover_cell = None;
        if matches!(
            state.profile.ruleset,
            CombatRuleset::NonLethalSpar | CombatRuleset::NonLethalBrawl
        ) {
            self.schedule_training_spar_epilogue(state);
            for id in &state.initiative {
                if let Some(stats) = self.entities.stats_mut(*id) {
                    stats.hp = stats.max_hp;
                }
            }
            self.log
                .push("Training fight ends. Everyone catches their breath.".into());
        }
        self.log.push("Combat ended.".into());
        let _ = self.modes.pop();
    }

    /// Leave combat from the UI (Esc); message is logged before combat state is torn down.
    pub(crate) fn finish_combat_player_quit(&mut self, state: &CombatState) {
        self.log.push("You leave the fight.".into());
        self.finish_combat(state);
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

    fn mode_label(&self) -> &'static str {
        match self.modes.current() {
            Some(GameMode::MainMenu { .. }) => "menu",
            Some(GameMode::Exploration) => "explore",
            Some(GameMode::Dialogue { .. }) => "dialogue",
            Some(GameMode::Inventory { .. }) => "inventory",
            Some(GameMode::Journal { .. }) => "journal",
            Some(GameMode::ItemTransfer { .. }) => "transfer",
            Some(GameMode::Combat(_)) => "combat",
            None => "none",
        }
    }

    fn compose_journal_overlay(&self, fb: &mut FrameBuffer, quest_cursor: usize) {
        fn status_label(s: QuestJournalStatus) -> &'static str {
            match s {
                QuestJournalStatus::InProgress => "In progress",
                QuestJournalStatus::Failed => "Failed",
                QuestJournalStatus::Completed => "Completed",
            }
        }

        let (left, right) = crate::ui::layout::OverlaySplitConfig::journal_or_inventory(
            fb.width,
            fb.height,
        );
        crate::ui::draw_bordered_panel(fb, left, "Quests");
        let inner_l = crate::ui::layout::panel_inner(left);
        let journals = &self.narrative.quest_journal;
        let n = journals.len();
        let mut rows: Vec<String> = Vec::new();
        if n == 0 {
            rows.push("(no entries yet)".into());
        } else {
            for (i, q) in journals.iter().enumerate() {
                let mark = if i == quest_cursor.min(n.saturating_sub(1)) {
                    "> "
                } else {
                    "  "
                };
                let tag = status_label(q.status);
                rows.push(format!("{}{} [{}]", mark, q.title, tag));
            }
        }
        rows.push("---".into());
        rows.push("j/k move  Esc close".into());
        crate::ui::draw_text_block(fb, inner_l, &rows);

        crate::ui::draw_bordered_panel(fb, right, "Entries");
        let inner_r = crate::ui::layout::panel_inner(right);
        let line_w = inner_r.w.saturating_sub(2) as usize;
        let mut detail: Vec<String> = Vec::new();
        if n == 0 {
            detail.push("Quest lines appear when you talk,".into());
            detail.push("pick up certain items, or advance".into());
            detail.push("a story beat.".into());
        } else {
            let q = &journals[quest_cursor.min(n.saturating_sub(1))];
            detail.push(format!("{} — {}", q.title, status_label(q.status)));
            detail.push(String::new());
            let mut entries: Vec<_> = q.entries.iter().collect();
            entries.sort_by_key(|e| e.seq);
            for e in entries {
                let line = format!("[{}] {}", e.seq, e.text);
                detail.extend(crate::ui::wrap::wrap_words(&line, line_w.max(12)));
                detail.push(String::new());
            }
            if q.entries.is_empty() {
                detail.push("(no log lines yet)".into());
            }
        }
        crate::ui::draw_text_block(fb, inner_r, &detail);
    }

    fn compose_inventory_overlay(&self, fb: &mut FrameBuffer, cursor: usize) {
        let (left, right) = crate::ui::layout::OverlaySplitConfig::journal_or_inventory(
            fb.width,
            fb.height,
        );
        crate::ui::draw_bordered_panel(fb, left, "Inventory");
        let cat = self.content.item_catalog();
        let stacks = &self.narrative.inventory.stacks;
        let n = stacks.len();
        let mut rows: Vec<String> = Vec::new();
        for (i, s) in stacks.iter().enumerate() {
            let mark = if n > 0 && i == cursor.min(n.saturating_sub(1)) {
                "> "
            } else {
                "  "
            };
            let label = cat.display_name(s.id.as_str());
            rows.push(format!("{}{} x{}", mark, label, s.count));
        }
        if rows.is_empty() {
            rows.push("(empty)".into());
        }
        rows.push("---".into());
        rows.push("u use  e equip  Esc close".into());
        let inner = crate::ui::layout::panel_inner(left);
        crate::ui::draw_text_block(fb, inner, &rows);

        crate::ui::draw_bordered_panel(fb, right, "Detail");
        let line_w = right.w.saturating_sub(2) as usize;
        let mut detail: Vec<String> = Vec::new();
        if let Some(s) = stacks.get(cursor.min(n.saturating_sub(1))) {
            if let Some(def) = cat.get(s.id.as_str()) {
                detail.push(def.name.to_string());
                detail.push(cat.category_line(s.id.as_str()));
                detail.push(String::new());
                detail.extend(crate::ui::wrap::wrap_words(def.description, line_w.max(12)));
            } else {
                detail.push(s.id.clone());
            }
        } else {
            detail.push("(no stacks)".into());
        }
        let r_inner = crate::ui::layout::panel_inner(right);
        crate::ui::draw_text_block(fb, r_inner, &detail);
    }

    fn compose_item_transfer_overlay(
        &self,
        fb: &mut FrameBuffer,
        container: EntityId,
        focus: TransferFocus,
        cursor_player: usize,
        cursor_container: usize,
    ) {
        let (left, right) =
            crate::ui::layout::OverlaySplitConfig::item_transfer(fb.width, fb.height);
        let cat = self.content.item_catalog();
        let cname = self
            .entities
            .name
            .get(container.0 as usize)
            .cloned()
            .unwrap_or_else(|| "Chest".into());
        let left_title = match focus {
            TransferFocus::Player => "You (*)",
            TransferFocus::Container => "You",
        };
        let right_title = match focus {
            TransferFocus::Container => format!("{cname} (*)"),
            TransferFocus::Player => cname,
        };
        crate::ui::draw_bordered_panel(fb, left, left_title);
        crate::ui::draw_bordered_panel(fb, right, right_title.as_str());

        let pn = self.narrative.inventory.stacks.len();
        let cont_stacks = self
            .narrative
            .container_inventories
            .get(&container.0)
            .map(|v| v.stacks.as_slice())
            .unwrap_or(&[]);
        let cn = cont_stacks.len();

        let mut pr: Vec<String> = Vec::new();
        for (i, s) in self.narrative.inventory.stacks.iter().enumerate() {
            let sel = if pn == 0 {
                0
            } else {
                cursor_player.min(pn.saturating_sub(1))
            };
            let mark = if matches!(focus, TransferFocus::Player) && i == sel {
                "> "
            } else {
                "  "
            };
            let label = cat.display_name(s.id.as_str());
            pr.push(format!("{}{} x{}", mark, label, s.count));
        }
        if pr.is_empty() {
            pr.push("(empty)".into());
        }
        let mut cr: Vec<String> = Vec::new();
        for (i, s) in cont_stacks.iter().enumerate() {
            let sel = if cn == 0 {
                0
            } else {
                cursor_container.min(cn.saturating_sub(1))
            };
            let mark = if matches!(focus, TransferFocus::Container) && i == sel {
                "> "
            } else {
                "  "
            };
            let label = cat.display_name(s.id.as_str());
            cr.push(format!("{}{} x{}", mark, label, s.count));
        }
        if cr.is_empty() {
            cr.push("(empty)".into());
        }
        pr.push("---".into());
        pr.push("Tab/hl: side  Enter: move".into());
        cr.push("---".into());
        cr.push("Tab/hl: side  Enter: move".into());

        let li = crate::ui::layout::panel_inner(left);
        let ri = crate::ui::layout::panel_inner(right);
        crate::ui::draw_text_block(fb, li, &pr);
        crate::ui::draw_text_block(fb, ri, &cr);
    }

    fn compose_world(&self, fb: &mut FrameBuffer, area: Rect) {
        let Some(_) = self.player_pos() else {
            return;
        };
        let cam_w = area.w as i32;
        let cam_h = area.h as i32;
        let (ox, oy) = self.world_screen_origin();

        for row in 0..area.h {
            for col in 0..area.w {
                let wx = ox + col as i32;
                let wy = oy + row as i32;
                let screen_x = area.x + col;
                let screen_y = area.y + row;
                let mut cell = Cell::default();
                if !self.map.in_bounds(wx, wy) {
                    cell.ch = ' ';
                    cell.bg = Color::rgb(10, 10, 20);
                    fb.set(screen_x, screen_y, cell);
                    continue;
                }
                let idx = wy as usize * self.map.width as usize + wx as usize;
                let seen = self.explored.get(idx).copied().unwrap_or(false);
                let vis = self.visible.get(idx).copied().unwrap_or(false);
                let tid = self.map.tile_at(wx, wy).unwrap_or(0);
                let def = self.map.table.def(tid);
                if seen {
                    let g = def.map(|d| d.glyph).unwrap_or('?');
                    cell.ch = g;
                    let base_fg = def.map(|d| d.fg).unwrap_or(Color::rgb(220, 220, 200));
                    if vis {
                        cell.fg = base_fg;
                        cell.bg = Color::rgb(20, 18, 28);
                    } else {
                        cell.fg = explored_muted_fg(base_fg);
                        cell.bg = Color::rgb(12, 12, 18);
                    }
                } else {
                    cell.ch = ' ';
                    cell.fg = Color::rgb(40, 40, 50);
                    cell.bg = Color::rgb(5, 5, 8);
                }
                fb.set(screen_x, screen_y, cell);
            }
        }

        // Entities on top
        for (i, alive) in self.entities.alive.iter().enumerate() {
            if !alive {
                continue;
            }
            let Some(ep) = self.entities.position[i] else {
                continue;
            };
            let wx = ep.x;
            let wy = ep.y;
            let sx = wx - ox;
            let sy = wy - oy;
            if sx < 0 || sy < 0 || sx >= cam_w || sy >= cam_h {
                continue;
            }
            let idx = wy as usize * self.map.width as usize + wx as usize;
            let vis = self.visible.get(idx).copied().unwrap_or(false);
            if !vis {
                continue;
            }
            let screen_x = area.x + sx as u16;
            let screen_y = area.y + sy as u16;
            let g = self.entities.glyph[i];
            let c = Cell {
                ch: g,
                fg: Color::rgb(255, 200, 120),
                bg: Color::rgb(20, 18, 28),
                style: Style {
                    bold: true,
                    dim: false,
                    underline: false,
                },
            };
            fb.set(screen_x, screen_y, c);
        }
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

    pub fn apply_save(&mut self, s: crate::save::SaveGameV1) -> Result<(), String> {
        if s.schema_version != crate::save::SAVE_SCHEMA_VERSION {
            return Err(format!("unsupported save version {}", s.schema_version));
        }
        self.map = s.world.map;
        self.entities = s.world.entities;
        self.narrative = s.world.narrative;
        self.rng_seed = s.world.rng_seed;
        self.modes = s.modes;
        self.view_pan_offset = (0, 0);
        self.last_world_pointer_cell = None;
        self.viewport_edge_scroll_cooldown = 0;
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
    use super::{Game, GameMode};
    use crate::combat::{CombatRuleset, CombatState, EncounterOutcomePolicy, EncounterProfile};
    use crate::entity::GridPos;
    use crate::input::{InputBatch, InputEvent, Key, KeyChord};

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
        game.handle_dialogue(InputEvent::Key(KeyChord {
            key: Key::Enter,
            shift: false,
            ctrl: false,
            alt: false,
        }));
        game.handle_dialogue(InputEvent::Key(KeyChord {
            key: Key::Enter,
            shift: false,
            ctrl: false,
            alt: false,
        }));
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
        game.finish_combat(&cs);
        for id in &cs.initiative {
            let stats = game
                .entities
                .stats(*id)
                .expect("combatant stats must exist");
            assert_eq!(stats.hp, stats.max_hp);
        }
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
        let mut cs = CombatState::from_participants(
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
        cs.turn_index = cs
            .initiative
            .iter()
            .position(|id| *id == trainer)
            .expect("trainer should be in initiative");
        cs.ap_remaining[cs.turn_index] = 100;
        game.modes.stack = vec![GameMode::Combat(cs)];
        let before = game.entities.stats(player).expect("player stats").hp;
        game.step(&InputBatch::default());
        let after = game.entities.stats(player).expect("player stats").hp;
        assert!(after < before, "npc should attack adjacent player");
    }

    #[test]
    fn npc_combat_ai_passes_when_adjacent_and_ap_insufficient_to_attack() {
        use crate::combat::ATTACK_COST_UNITS;

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
        let mut cs = CombatState::from_participants(
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
        let trainer_turn = cs
            .initiative
            .iter()
            .position(|id| *id == trainer)
            .expect("trainer should be in initiative");
        cs.turn_index = trainer_turn;
        cs.ap_remaining[trainer_turn] = ATTACK_COST_UNITS - 1;
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
            "trainer should pass the turn when unable to attack"
        );
    }

    #[test]
    fn npc_combat_ai_move_pacing_skips_tick_while_cooldown() {
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
        game.entities.set_pos(trainer, GridPos { x: 14, y: 10 });
        let mut rng = 1;
        let mut cs = CombatState::from_participants(
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
        cs.turn_index = cs
            .initiative
            .iter()
            .position(|id| *id == trainer)
            .expect("trainer should be in initiative");
        cs.ap_remaining[cs.turn_index] = 100;
        game.modes.stack = vec![GameMode::Combat(cs)];
        game.step(&InputBatch::default());
        let after_first = game.entities.pos(trainer).expect("trainer position");
        let mut same_ticks = 0u32;
        while game.entities.pos(trainer) == Some(after_first) {
            game.step(&InputBatch::default());
            same_ticks += 1;
            assert!(
                same_ticks < 32,
                "trainer should take another step after visual pacing cooldown"
            );
        }
        assert!(
            same_ticks >= 2,
            "expected at least one pacing tick before second move (speed 7 => 3-tick cooldown at ~60 Hz)"
        );
    }
}
