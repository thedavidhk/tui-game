//! Top-level game state, mode stack, and stepping.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::combat::CombatState;
use crate::content::{ContentPack, DemoQuestPhase};
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::input::{
    hit_rect_index, InputBatch, InputEvent, Key, KeyChord, MouseButton, MouseEventKind,
};
use crate::level::LevelFile;
use crate::render::{Cell, Color, FrameBuffer, FrameSample, Style};
use crate::rect::Rect;
use crate::world::{compute_visible, merge_explored, MapGrid, TileTable};

const FOW_RADIUS: i32 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    MainMenu { selected: usize },
    Exploration,
    Dialogue {
        npc_entity: EntityId,
        dialogue_id: String,
        node_index: usize,
        choice_cursor: usize,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeState {
    pub quests: DemoQuestPhase,
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
    /// Last-frame hit targets for main menu rows (keyboard fallback always works).
    pub menu_mouse: Vec<Rect>,
    pub dialogue_mouse: Vec<Rect>,
    pub last_perf: Option<FrameSample>,
}

impl Game {
    pub fn new_bootstrapped(viewport_w: u16, viewport_h: u16) -> Self {
        let content = ContentPack::demo();
        let _ = content.validate();

        let table = TileTable::default_pack();
        let mut map = MapGrid::filled(24, 16, 0, table);
        // Simple room with walls
        for x in 0..map.width {
            map.set_tile(x as i32, 0, 1);
            map.set_tile(x as i32, map.height as i32 - 1, 1);
        }
        for y in 0..map.height {
            map.set_tile(0, y as i32, 1);
            map.set_tile(map.width as i32 - 1, y as i32, 1);
        }

        let mut entities = EntityArena::new();
        let player = entities.spawn(
            GridPos { x: 4, y: 4 },
            '@',
            "You".into(),
            false,
            None,
        );
        entities.set_player(player);
        entities.spawn(
            GridPos { x: 10, y: 8 },
            'g',
            "Guide".into(),
            true,
            Some("guide".into()),
        );

        let n = (map.width as usize) * (map.height as usize);
        let mut game = Self {
            modes: GameModeStack {
                stack: vec![GameMode::MainMenu { selected: 0 }],
            },
            map,
            entities,
            explored: vec![false; n],
            visible: vec![false; n],
            narrative: NarrativeState::default(),
            content,
            rng_seed: 1,
            debug_overlay: false,
            viewport_w,
            viewport_h,
            log: vec!["Welcome. Arrows/WASD move, E interact, F1 debug, Q quit menu.".into()],
            menu_items: vec!["Start game", "Quit"],
            quit_requested: false,
            menu_mouse: Vec::new(),
            dialogue_mouse: Vec::new(),
            last_perf: None,
        };
        game.refresh_fow();
        game
    }

    pub fn from_level_file(
        level: &LevelFile,
        viewport_w: u16,
        viewport_h: u16,
    ) -> Result<Self, String> {
        let content = ContentPack::demo();
        content.validate().map_err(|e| e.to_string())?;
        let map = level.to_map()?;
        let n = (map.width as usize) * (map.height as usize);
        let mut entities = EntityArena::new();
        for s in &level.spawns {
            let npc = if s.kind == "guide" {
                Some("guide".into())
            } else {
                None
            };
            entities.spawn(
                GridPos { x: s.x, y: s.y },
                s.glyph,
                s.name.clone(),
                true,
                npc,
            );
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
        );
        entities.set_player(player);

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
            menu_mouse: Vec::new(),
            dialogue_mouse: Vec::new(),
            last_perf: None,
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
        let Some(pid) = self.player_id() else {
            return;
        };
        let Some(p) = self.entities.pos(pid) else {
            return;
        };
        let nx = p.x + dx;
        let ny = p.y + dy;
        if !self.map.in_bounds(nx, ny) {
            return;
        }
        if self.map.blocks_movement(nx, ny) {
            return;
        }
        if let Some(occ) = self.entities.occupant_at(nx, ny) {
            if self.entities.npc_kind[occ.0 as usize].is_some() {
                self.start_dialogue(occ);
                return;
            }
            if self.entities.blocks_movement[occ.0 as usize] {
                return;
            }
        }
        self.entities.set_pos(pid, GridPos { x: nx, y: ny });
        self.log.push(format!("Move to ({}, {}).", nx, ny));
        self.refresh_fow();
    }

    fn start_dialogue(&mut self, npc: EntityId) {
        let kind = self.entities.npc_kind[npc.0 as usize]
            .clone()
            .unwrap_or_default();
        self.modes.push(GameMode::Dialogue {
            npc_entity: npc,
            dialogue_id: kind.clone(),
            node_index: 0,
            choice_cursor: 0,
        });
        self.log.push(format!("Talking ({}).", kind));
    }

    pub fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            self.handle_event(ev.clone());
        }
    }

    fn handle_event(&mut self, ev: InputEvent) {
        match self.modes.current().cloned() {
            None => {}
            Some(GameMode::MainMenu { selected }) => self.handle_menu(ev, selected),
            Some(GameMode::Exploration) => self.handle_explore(ev),
            Some(GameMode::Dialogue { .. }) => self.handle_dialogue(ev),
            Some(GameMode::Combat(ref c)) => self.handle_combat(ev, c.clone()),
        }
    }

    fn handle_menu(&mut self, ev: InputEvent, selected: usize) {
        match ev {
            InputEvent::Mouse {
                kind: MouseEventKind::Down(MouseButton::Left),
                cell,
                ..
            } => {
                if let Some(i) = hit_rect_index(cell, &self.menu_mouse) {
                    if i < self.menu_items.len() {
                        if let Some(GameMode::MainMenu { selected: s }) = self.modes.current_mut() {
                            *s = i;
                        }
                    }
                }
            }
            InputEvent::Key(k) => {
                if k.key == Key::Char('q') || k.key == Key::Esc {
                    // stay on menu; quit only via selection
                }
                if matches!(k.key, Key::Up | Key::Char('k')) {
                    let sel = selected.saturating_sub(1);
                    if let Some(GameMode::MainMenu { selected: s }) = self.modes.current_mut() {
                        *s = sel;
                    }
                }
                if matches!(k.key, Key::Down | Key::Char('j')) {
                    let n = self.menu_items.len();
                    let sel = (selected + 1).min(n.saturating_sub(1));
                    if let Some(GameMode::MainMenu { selected: s }) = self.modes.current_mut() {
                        *s = sel;
                    }
                }
                if matches!(k.key, Key::Enter) {
                    match selected {
                        0 => {
                            self.modes.stack = vec![GameMode::Exploration];
                            self.log.push("Entered world.".into());
                            self.refresh_fow();
                        }
                        1 => {
                            self.quit_requested = true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_explore(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::Key(KeyChord { key: Key::F(1), .. }) => {
                self.debug_overlay = !self.debug_overlay;
            }
            InputEvent::Key(KeyChord { key: Key::F(5), .. }) => {
                match self.save_to_path("save.ron") {
                    Ok(()) => self.log.push("Saved save.ron (F5).".into()),
                    Err(e) => self.log.push(format!("Save failed: {e}")),
                }
            }
            InputEvent::Key(KeyChord { key: Key::F(9), .. }) => {
                match self.load_from_path("save.ron") {
                    Ok(()) => {}
                    Err(e) => self.log.push(format!("Load failed: {e}")),
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('e'), ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Enter, ..
            }) => self.try_interact(),
            InputEvent::Key(KeyChord {
                key: Key::Char('c'),
                ..
            }) => self.try_start_combat(),
            InputEvent::Key(KeyChord {
                key: Key::Char('w'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Up, ..
            }) => self.try_move_player(0, -1),
            InputEvent::Key(KeyChord {
                key: Key::Char('s'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Down, ..
            }) => self.try_move_player(0, 1),
            InputEvent::Key(KeyChord {
                key: Key::Char('a'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Left, ..
            }) => self.try_move_player(-1, 0),
            InputEvent::Key(KeyChord {
                key: Key::Char('d'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Right, ..
            }) => self.try_move_player(1, 0),
            _ => {}
        }
    }

    fn try_interact(&mut self) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let Some(p) = self.entities.pos(pid) else {
            return;
        };
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = p.x + dx;
                let ny = p.y + dy;
                if let Some(occ) = self.entities.occupant_at(nx, ny) {
                    if self.entities.npc_kind[occ.0 as usize].is_some() {
                        self.start_dialogue(occ);
                        return;
                    }
                }
            }
        }
        self.log.push("No one to talk to nearby.".into());
    }

    fn try_start_combat(&mut self) {
        let Some(pid) = self.player_id() else {
            return;
        };
        let Some(p) = self.entities.pos(pid) else {
            return;
        };
        let mut others = Vec::new();
        if let Some(occ) = self.entities.occupant_at(p.x, p.y + 1) {
            if occ != pid {
                others.push(occ);
            }
        }
        if others.is_empty() {
            self.log.push("Combat: stand south of an entity and press c.".into());
            return;
        }
        let mut ini = vec![pid];
        ini.extend(others);
        self.modes.push(GameMode::Combat(CombatState::from_participants(
            ini,
            self.map.width,
            self.map.height,
        )));
        self.log.push("Combat started (stub). Tab: end turn, f: flee.".into());
    }

    fn handle_dialogue(&mut self, ev: InputEvent) {
        let (dialogue_id, node_index) = match self.modes.current() {
            Some(GameMode::Dialogue {
                dialogue_id,
                node_index,
                ..
            }) => (dialogue_id.clone(), *node_index),
            _ => return,
        };
        let tree = self
            .content
            .dialogues
            .get(dialogue_id.as_str())
            .copied()
            .unwrap_or(self.content.guide_dialogue);
        let Some(node) = tree.nodes.get(node_index) else {
            let _ = self.modes.pop();
            return;
        };
        let exit_sentinel = tree.nodes.len();

        match ev {
            InputEvent::Mouse {
                kind: MouseEventKind::Down(MouseButton::Left),
                cell,
                ..
            } => {
                if let Some(i) = hit_rect_index(cell, &self.dialogue_mouse) {
                    if let Some(GameMode::Dialogue { choice_cursor: c, .. }) =
                        self.modes.current_mut()
                    {
                        let max = node.choices.len().saturating_sub(1);
                        *c = i.min(max);
                    }
                    self.apply_dialogue_choice(tree, exit_sentinel);
                }
            }
            InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
                let _ = self.modes.pop();
            }
            InputEvent::Key(KeyChord {
                key: Key::Up | Key::Char('k'),
                ..
            }) => {
                if let Some(GameMode::Dialogue { choice_cursor: c, .. }) =
                    self.modes.current_mut()
                {
                    *c = c.saturating_sub(1);
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Down | Key::Char('j'),
                ..
            }) => {
                if let Some(GameMode::Dialogue { choice_cursor: c, .. }) =
                    self.modes.current_mut()
                {
                    let max = node.choices.len().saturating_sub(1);
                    *c = (*c + 1).min(max);
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Enter | Key::Char(' '),
                ..
            }) => {
                self.apply_dialogue_choice(tree, exit_sentinel);
            }
            InputEvent::Key(KeyChord {
                key: Key::Char(c), ..
            }) if c.is_ascii_digit() => {
                let d = (c as u8).saturating_sub(b'1') as usize;
                if d < node.choices.len() {
                    if let Some(GameMode::Dialogue { choice_cursor: c, .. }) =
                        self.modes.current_mut()
                    {
                        *c = d;
                    }
                    self.apply_dialogue_choice(tree, exit_sentinel);
                }
            }
            _ => {}
        }
    }

    fn apply_dialogue_choice(
        &mut self,
        tree: &'static crate::content::DialogueTree,
        exit_sentinel: usize,
    ) {
        let Some(GameMode::Dialogue {
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
        let Some(choice) = node.choices.get(choice_cursor) else {
            return;
        };
        let next = choice.next;
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
        if next == 1 {
            self.narrative.quests = DemoQuestPhase::TalkedToGuide;
            self.log.push("You listened.".into());
        }
    }

    fn handle_combat(&mut self, ev: InputEvent, state: CombatState) {
        let mut end = false;
        let mut next = state.clone();
        match ev {
            InputEvent::Key(KeyChord {
                key: Key::Tab | Key::Char(' '),
                ..
            }) => {
                next.end_turn();
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('f'),
                ..
            }) => {
                end = true;
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('w'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Up, ..
            }) => {
                self.combat_try_move(&mut next, 0, -1);
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('s'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Down, ..
            }) => {
                self.combat_try_move(&mut next, 0, 1);
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('a'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Left, ..
            }) => {
                self.combat_try_move(&mut next, -1, 0);
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('d'),
                ..
            })
            | InputEvent::Key(KeyChord {
                key: Key::Right, ..
            }) => {
                self.combat_try_move(&mut next, 1, 0);
            }
            InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
                end = true;
            }
            _ => {}
        }
        if end {
            let _ = self.modes.pop();
            self.log.push("Combat ended.".into());
            return;
        }
        if let Some(GameMode::Combat(cs)) = self.modes.current_mut() {
            *cs = next;
        }
    }

    fn combat_try_move(&mut self, state: &mut CombatState, dx: i32, dy: i32) {
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
        let ended = state.apply_action(
            crate::combat::CombatAction::Move { target },
            &mut self.entities,
            map_blocks,
        );
        if ended {
            let _ = self.modes.pop();
            self.log.push("Combat ended.".into());
        }
    }

    pub fn compose(
        &mut self,
        fb: &mut FrameBuffer,
        world_rect: Rect,
        hud_rect: Rect,
        log_rect: Rect,
    ) {
        self.menu_mouse.clear();
        self.dialogue_mouse.clear();
        self.compose_world(fb, world_rect);
        crate::ui::draw_bordered_panel(fb, hud_rect, "Status");
        let inner = Rect::new(hud_rect.x + 1, hud_rect.y + 1, hud_rect.w.saturating_sub(2), hud_rect.h.saturating_sub(2));
        let lines = vec![
            format!("Mode: {}", self.mode_label()),
            format!("Quest: {:?}", self.narrative.quests),
            "E: talk  C: combat".into(),
            "F1: debug  F5/F9: save/load".into(),
        ];
        crate::ui::draw_text_block(fb, inner, &lines);

        crate::ui::draw_bordered_panel(fb, log_rect, "Log");
        let log_inner = Rect::new(log_rect.x + 1, log_rect.y + 1, log_rect.w.saturating_sub(2), log_rect.h.saturating_sub(2));
        let tail: Vec<String> = self.log.iter().rev().take(6).cloned().collect();
        let rev: Vec<String> = tail.into_iter().rev().collect();
        crate::ui::draw_log(fb, log_inner, &rev, &mut Vec::new());

        if let Some(GameMode::MainMenu { selected }) = self.modes.current().cloned() {
            let menu_r = Rect::new(2, 2, 30, 10);
            crate::ui::draw_menu(
                fb,
                menu_r,
                "Main menu",
                &self.menu_items,
                selected,
                &mut self.menu_mouse,
            );
        }

        if let Some(GameMode::Dialogue {
            ref dialogue_id,
            node_index,
            choice_cursor,
            ..
        }) = self.modes.current().cloned()
        {
            let tree = self
                .content
                .dialogues
                .get(dialogue_id.as_str())
                .copied()
                .unwrap_or(self.content.guide_dialogue);
            if let Some(node) = tree.nodes.get(node_index) {
                let dr = Rect::new(2, fb.height.saturating_sub(12), fb.width.saturating_sub(4), 10);
                crate::ui::draw_dialogue(fb, dr, node, choice_cursor, &mut self.dialogue_mouse);
            }
        }

        if let Some(GameMode::Combat(ref c)) = self.modes.current() {
            let cr = Rect::new(2, 10, 40, 6);
            let who = c
                .current_actor()
                .map(|id| self.entities.name.get(id.0 as usize).cloned().unwrap_or_default())
                .unwrap_or_default();
            let lines = vec![
                "Combat (stub)".into(),
                format!("Turn: {}", who),
                "Move WASD, Tab pass, F flee".into(),
            ];
            crate::ui::draw_bordered_panel(fb, cr, "Combat");
            let inner = Rect::new(cr.x + 1, cr.y + 1, cr.w.saturating_sub(2), cr.h.saturating_sub(2));
            crate::ui::draw_text_block(fb, inner, &lines);
        }

        if self.debug_overlay {
            let dbg = Rect::new(2, fb.height.saturating_sub(8), fb.width.saturating_sub(4), 6);
            let dirty = fb.dirty_count();
            let enc = self
                .last_perf
                .map(|p| format!("encode_us {}", p.encode_nanos / 1000))
                .unwrap_or_else(|| "encode_us —".into());
            let lines = vec![
                format!("debug: dirty_cells(prev) ~{}", dirty),
                format!("map {}x{}", self.map.width, self.map.height),
                enc,
            ];
            crate::ui::draw_bordered_panel(fb, dbg, "Debug");
            let inner = Rect::new(dbg.x + 1, dbg.y + 1, dbg.w.saturating_sub(2), dbg.h.saturating_sub(2));
            crate::ui::draw_text_block(fb, inner, &lines);
        }
    }

    fn mode_label(&self) -> &'static str {
        match self.modes.current() {
            Some(GameMode::MainMenu { .. }) => "menu",
            Some(GameMode::Exploration) => "explore",
            Some(GameMode::Dialogue { .. }) => "dialogue",
            Some(GameMode::Combat(_)) => "combat",
            None => "none",
        }
    }

    fn compose_world(&self, fb: &mut FrameBuffer, area: Rect) {
        let Some(p) = self.player_pos() else {
            return;
        };
        let cam_w = area.w as i32;
        let cam_h = area.h as i32;
        let ox = p.x - cam_w / 2;
        let oy = p.y - cam_h / 2;

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
                    if vis {
                        cell.fg = Color::rgb(220, 220, 200);
                        cell.bg = Color::rgb(20, 18, 28);
                    } else {
                        cell.fg = Color::rgb(90, 85, 100);
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
            let mut c = Cell::default();
            c.ch = g;
            c.fg = Color::rgb(255, 200, 120);
            c.bg = Color::rgb(20, 18, 28);
            c.style = Style {
                bold: true,
                dim: false,
                underline: false,
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
