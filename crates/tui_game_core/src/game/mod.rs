//! Top-level game state, mode stack, and stepping.

mod modes;

use std::fs;

use serde::{Deserialize, Serialize};

use crate::combat::CombatState;
use crate::content::{ContentPack, DemoQuestPhase};
use crate::entity::{EntityArena, EntityId, GridPos};
use crate::game_content;
use crate::input::{InputBatch, InputEvent, Key, KeyChord, MouseButton, MouseEventKind};
use crate::item::{Inventory, ItemCategory, ItemStack};
use crate::level::LevelFile;
use crate::narrative::NarrativeState;
use crate::rect::Rect;
use crate::render::{Cell, Color, FrameBuffer, FrameSample, Style};
use crate::ui::hit::{UiHitState, UiHitTarget};
use crate::world::{compute_visible, merge_explored, MapGrid, TileTable};

const FOW_RADIUS: i32 = 8;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFocus {
    Player,
    Container,
}

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
    Inventory {
        cursor: usize,
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
}

impl Game {
    pub fn new_bootstrapped(viewport_w: u16, viewport_h: u16) -> Self {
        let content = game_content::content_pack();
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
            None,
            false,
        );
        entities.set_player(player);
        entities.spawn(
            GridPos { x: 10, y: 8 },
            'g',
            "Guide".into(),
            true,
            Some("guide".into()),
            None,
            false,
        );
        entities.spawn(
            GridPos { x: 6, y: 5 },
            ',',
            "Key".into(),
            false,
            None,
            Some(ItemStack::new("cellar_key", 1)),
            false,
        );
        entities.spawn(
            GridPos { x: 8, y: 5 },
            '□',
            "Chest".into(),
            true,
            None,
            None,
            true,
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
            log: vec!["Welcome. WASD move, E interact, I inventory, F1 debug.".into()],
            menu_items: vec!["Start game", "Quit"],
            quit_requested: false,
            ui_hits: UiHitState::default(),
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
        let content = game_content::content_pack();
        content.validate().map_err(|e| e.to_string())?;
        content
            .validate_level(level)
            .map_err(|e| e.to_string())?;
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
            let npc = bp
                .dialogue_id
                .map(std::string::ToString::to_string);
            let item = bp.world_item.map(|id| ItemStack::new(id, 1));
            let blocks_movement = if item.is_some() {
                false
            } else if bp.is_container {
                true
            } else {
                npc.is_some()
            };
            entities.spawn(
                GridPos { x: s.x, y: s.y },
                s.glyph,
                s.name.clone(),
                blocks_movement,
                npc,
                item,
                bp.is_container,
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
            None,
            false,
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
            ui_hits: UiHitState::default(),
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
        if let Some(occ) = self.entities.first_npc_at(nx, ny) {
            if occ != pid {
                self.start_dialogue(occ);
                return;
            }
        }
        for oid in self.entities.occupants_at(nx, ny) {
            if oid == pid {
                continue;
            }
            if self.entities.blocks_movement[oid.0 as usize] {
                return;
            }
        }
        self.entities.set_pos(pid, GridPos { x: nx, y: ny });
        self.log.push(format!("Move to ({}, {}).", nx, ny));
        self.try_pickup_ground_items(pid);
        self.refresh_fow();
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
            if stack.id == "cellar_key" {
                match self.narrative.quests {
                    DemoQuestPhase::ReturnedKey | DemoQuestPhase::Done => {}
                    _ => self.narrative.quests = DemoQuestPhase::HasCellarKey,
                }
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
        self.modes.push(GameMode::Dialogue {
            npc_entity: npc,
            dialogue_id: kind.clone(),
            node_index: 0,
            choice_cursor: 0,
        });
        self.log.push(format!("Talking ({}).", kind));
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
        self
            .log
            .push("Transfer: Tab/hl side, jk rows, Enter move stack, Esc close.".into());
    }

    pub fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            self.handle_event(ev.clone());
        }
    }

    fn handle_event(&mut self, ev: InputEvent) {
        modes::route(self, ev);
    }

    pub(crate) fn handle_menu(&mut self, ev: InputEvent, selected: usize) {
        match ev {
            InputEvent::Mouse {
                kind: MouseEventKind::Down(MouseButton::Left),
                cell,
                ..
            } => {
                if let Some(UiHitTarget::MainMenuItem(i)) = self.ui_hits.pick(cell) {
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

    pub(crate) fn handle_explore(&mut self, ev: InputEvent) {
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
                key: Key::Char('i'), ..
            }) => {
                self.modes.push(GameMode::Inventory { cursor: 0 });
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

    pub(crate) fn try_interact(&mut self) {
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
                if let Some(occ) = self.entities.first_npc_at(nx, ny) {
                    self.start_dialogue(occ);
                    return;
                }
                if let Some(chest) = self.entities.first_container_at(nx, ny) {
                    self.start_item_transfer(chest);
                    return;
                }
            }
        }
        self.log.push("Nothing to interact with nearby.".into());
    }

    pub(crate) fn try_start_combat(&mut self) {
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

    pub(crate) fn handle_dialogue(&mut self, ev: InputEvent) {
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
                if let Some(UiHitTarget::DialogueChoice(i)) = self.ui_hits.pick(cell) {
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

    pub(crate) fn apply_dialogue_choice(
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
        if let Err(msg) = self.narrative.check_requires(choice.requires) {
            self.log.push(msg);
            return;
        }
        if let Err(e) = self
            .narrative
            .apply_effects(&mut self.log, choice.effects)
        {
            self.log
                .push(format!("Dialogue effect failed: {e:?}"));
            return;
        }
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
    }

    pub(crate) fn handle_inventory(&mut self, ev: InputEvent) {
        let n = self.narrative.inventory.stacks.len();
        match ev {
            InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
                let _ = self.modes.pop();
            }
            InputEvent::Key(KeyChord {
                key: Key::Up | Key::Char('k'),
                ..
            }) => {
                if let Some(GameMode::Inventory { cursor }) = self.modes.current_mut() {
                    *cursor = cursor.saturating_sub(1);
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Down | Key::Char('j'),
                ..
            }) => {
                if let Some(GameMode::Inventory { cursor }) = self.modes.current_mut() {
                    let max = n.saturating_sub(1);
                    *cursor = (*cursor + 1).min(max);
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('u'),
                ..
            }) => {
                let Some(GameMode::Inventory { cursor }) = self.modes.current().cloned() else {
                    return;
                };
                if n == 0 {
                    return;
                }
                let idx = cursor.min(n.saturating_sub(1));
                let Some(stack) = self.narrative.inventory.stacks.get(idx) else {
                    return;
                };
                let id_owned = stack.id.clone();
                let catlog = self.content.item_catalog();
                let Some(def) = catlog.get(id_owned.as_str()) else {
                    self.log.push(format!(
                        "{}: unknown item.",
                        catlog.display_name(id_owned.as_str())
                    ));
                    return;
                };
                match def.category {
                    ItemCategory::Consumable => {
                        let name = def.name;
                        if self.narrative.inventory.try_remove(&id_owned, 1).is_ok() {
                            self.log
                                .push(format!("Used {name} (no effect yet)."));
                        }
                    }
                    _ => self.log.push("That item is not consumable (u).".into()),
                }
                if let Some(GameMode::Inventory { cursor }) = self.modes.current_mut() {
                    *cursor = (*cursor).min(self.narrative.inventory.stacks.len().saturating_sub(1));
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Char('e'),
                ..
            }) => {
                let Some(GameMode::Inventory { cursor }) = self.modes.current().cloned() else {
                    return;
                };
                if n == 0 {
                    return;
                }
                let idx = cursor.min(n.saturating_sub(1));
                let Some(stack) = self.narrative.inventory.stacks.get(idx) else {
                    return;
                };
                let id_owned = stack.id.clone();
                let catlog = self.content.item_catalog();
                let Some(def) = catlog.get(id_owned.as_str()) else {
                    self.log.push(format!(
                        "{}: unknown item.",
                        catlog.display_name(id_owned.as_str())
                    ));
                    return;
                };
                match def.category {
                    ItemCategory::Equippable(slot) => {
                        if self.narrative.inventory.try_remove(&id_owned, 1).is_err() {
                            self.log.push("Could not equip.".into());
                            return;
                        }
                        if let Some(prev) = self
                            .narrative
                            .equipment
                            .insert(slot, id_owned.clone())
                        {
                            self.narrative.inventory.add(prev, 1);
                        }
                        self.log.push(format!("Equipped {} (stub).", def.name));
                    }
                    _ => self.log.push("That item is not equippable (e).".into()),
                }
                if let Some(GameMode::Inventory { cursor }) = self.modes.current_mut() {
                    *cursor = (*cursor).min(self.narrative.inventory.stacks.len().saturating_sub(1));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_item_transfer(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::Key(KeyChord { key: Key::Esc, .. }) => {
                let _ = self.modes.pop();
            }
            InputEvent::Key(KeyChord {
                key: Key::Tab | Key::Char('h') | Key::Char('l'),
                ..
            }) => {
                if let Some(GameMode::ItemTransfer { focus, .. }) = self.modes.current_mut() {
                    *focus = match *focus {
                        TransferFocus::Player => TransferFocus::Container,
                        TransferFocus::Container => TransferFocus::Player,
                    };
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Up | Key::Char('k'),
                ..
            }) => {
                if let Some(GameMode::ItemTransfer {
                    focus,
                    cursor_player,
                    cursor_container,
                    container,
                }) = self.modes.current_mut()
                {
                    let pn = self.narrative.inventory.stacks.len();
                    let cn = self
                        .narrative
                        .container_inventories
                        .entry(container.0)
                        .or_default()
                        .stacks
                        .len();
                    match focus {
                        TransferFocus::Player => {
                            *cursor_player = cursor_player
                                .saturating_sub(1)
                                .min(pn.saturating_sub(1));
                        }
                        TransferFocus::Container => {
                            *cursor_container = cursor_container
                                .saturating_sub(1)
                                .min(cn.saturating_sub(1));
                        }
                    }
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Down | Key::Char('j'),
                ..
            }) => {
                if let Some(GameMode::ItemTransfer {
                    focus,
                    cursor_player,
                    cursor_container,
                    container,
                }) = self.modes.current_mut()
                {
                    let pn = self.narrative.inventory.stacks.len();
                    let cn = self
                        .narrative
                        .container_inventories
                        .entry(container.0)
                        .or_default()
                        .stacks
                        .len();
                    match focus {
                        TransferFocus::Player => {
                            let max = pn.saturating_sub(1);
                            *cursor_player = (*cursor_player + 1).min(max);
                        }
                        TransferFocus::Container => {
                            let max = cn.saturating_sub(1);
                            *cursor_container = (*cursor_container + 1).min(max);
                        }
                    }
                }
            }
            InputEvent::Key(KeyChord {
                key: Key::Enter,
                ..
            }) => {
                let Some(GameMode::ItemTransfer {
                    container,
                    focus,
                    cursor_player,
                    cursor_container,
                }) = self.modes.current().cloned()
                else {
                    return;
                };
                {
                    let inv = &mut self.narrative.inventory;
                    let ce = self
                        .narrative
                        .container_inventories
                        .entry(container.0)
                        .or_default();
                    match focus {
                        TransferFocus::Player => {
                            if cursor_player < inv.stacks.len() {
                                let _ = Inventory::try_move_stack_index(inv, ce, cursor_player);
                            }
                        }
                        TransferFocus::Container => {
                            if cursor_container < ce.stacks.len() {
                                let _ = Inventory::try_move_stack_index(ce, inv, cursor_container);
                            }
                        }
                    }
                }
                if let Some(GameMode::ItemTransfer {
                    cursor_player: cp,
                    cursor_container: cc,
                    container: cid,
                    ..
                }) = self.modes.current_mut()
                {
                    let pn = self.narrative.inventory.stacks.len();
                    let cn = self
                        .narrative
                        .container_inventories
                        .get(&cid.0)
                        .map(|c| c.stacks.len())
                        .unwrap_or(0);
                    *cp = (*cp).min(pn.saturating_sub(1));
                    *cc = (*cc).min(cn.saturating_sub(1));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_combat(&mut self, ev: InputEvent, state: CombatState) {
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
        self.ui_hits.clear();
        self.compose_world(fb, world_rect);
        crate::ui::draw_bordered_panel(fb, hud_rect, "Status");
        let inner = Rect::new(hud_rect.x + 1, hud_rect.y + 1, hud_rect.w.saturating_sub(2), hud_rect.h.saturating_sub(2));
        let lines = vec![
            format!("Mode: {}", self.mode_label()),
            format!("Quest: {:?}", self.narrative.quests),
            "I: inventory  E: talk/chest  C: combat".into(),
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
                &mut self.ui_hits,
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
                crate::ui::draw_dialogue(fb, dr, node, choice_cursor, &mut self.ui_hits);
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

        if let Some(GameMode::Inventory { cursor }) = self.modes.current() {
            self.compose_inventory_overlay(fb, *cursor);
        }
        if let Some(GameMode::ItemTransfer {
            container,
            focus,
            cursor_player,
            cursor_container,
        }) = self.modes.current()
        {
            self.compose_item_transfer_overlay(
                fb,
                *container,
                *focus,
                *cursor_player,
                *cursor_container,
            );
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
            Some(GameMode::Inventory { .. }) => "inventory",
            Some(GameMode::ItemTransfer { .. }) => "transfer",
            Some(GameMode::Combat(_)) => "combat",
            None => "none",
        }
    }

    fn compose_inventory_overlay(&self, fb: &mut FrameBuffer, cursor: usize) {
        let (left, right) = crate::ui::layout::split_horizontal_outer(
            fb.width,
            fb.height,
            2,
            3,
            3,
            2,
            18,
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
                detail.extend(crate::ui::wrap::wrap_words(
                    def.description,
                    line_w.max(12),
                ));
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
        let (left, right) = crate::ui::layout::split_horizontal_outer(
            fb.width,
            fb.height,
            2,
            2,
            3,
            2,
            18,
        );
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
