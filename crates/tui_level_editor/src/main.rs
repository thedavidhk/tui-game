//! Level editor: `LevelFile` paint/spawns, custom tile defs, resize, named save.
//!
//! Hot-reload: if the `.ron` on disk changes, the editor reloads when there are no unsaved edits;
//! otherwise it prompts (Y discard + reload, N/Esc keep). Reload only applies after RON parse and
//! `validate_level` succeed; failures keep the previous level and show an error.

use std::env;
use std::fs;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton as CMouseButton,
        MouseEventKind as CMouseKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    QueueableCommand,
};
use tui_game_core::content::ContentPack;
use tui_game_core::game_content;
use tui_game_core::input::{
    InputBatch, InputEvent, Key, KeyChord, MouseButton, MouseCell, MouseEventKind,
};
use tui_game_core::level::{
    derive_visual_seed, level_from_ron, level_to_ron, EntitySpawn, LevelFile, PlayerSpawn,
};
use tui_game_core::rect::Rect;
use tui_game_core::render::{
    encode_frame_delta, encode_frame_full, Cell, Color, FrameBuffer, Style,
};
use tui_game_core::ui::{
    cell_in_axis_rect, cell_in_brush, cell_local_in_rect, centered_rect, draw_bordered_panel,
    draw_text_block, draw_text_field, for_each_in_brush, for_each_in_rect,
    viewport_scroll::{edge_scroll_pan_delta, EDGE_SCROLL_COOLDOWN_TICKS},
    TextField, TextFieldOutput, TextFilter, PRESET_COLORS,
};
use tui_game_core::world::{def_is_animated, resolve_animated, TileDef, TileDisplayCell, TileId};
use tui_game_core::EntityBlueprint;

/// Fixed width for the right-hand palette / help column.
const EDITOR_SIDEBAR_WIDTH: u16 = 28;

/// `mtime` + `len` so two writes in the same second still register as different when size changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    modified_ns: Option<u128>,
}

impl FileFingerprint {
    const MISSING: Self = Self {
        len: 0,
        modified_ns: None,
    };

    fn from_path(path: &Path) -> Option<Self> {
        let m = fs::metadata(path).ok()?;
        let len = m.len();
        let modified_ns = m.modified().ok().and_then(|st| {
            st.duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_nanos())
        });
        Some(Self { len, modified_ns })
    }
}

struct Editor {
    path: PathBuf,
    level: LevelFile,
    content: ContentPack,
    cursor_x: i32,
    cursor_y: i32,
    current_tile: TileId,
    /// Index into `content.entity_blueprints` when placing spawns.
    spawn_blueprint_idx: usize,
    mode: Mode,
    status: String,
    dialog: Option<Dialog>,
    /// Last framebuffer size (updated in `compose`).
    viewport_w: u16,
    viewport_h: u16,
    /// Chebyshev brush radius in cells (0 = single cell).
    brush_radius: u8,
    /// Shift–left drag rectangle: anchor corner until mouse up.
    rect_drag_start: Option<(i32, i32)>,
    last_paint_cell: Option<(i32, i32)>,
    /// Hit targets for the current frame (sidebar rows).
    sidebar_hits: Vec<(SidebarHit, Rect)>,
    /// Map cell under the mouse (level coords), when over the map and no modal dialog.
    hover_map_cell: Option<(i32, i32)>,
    /// Top-left level coordinate of the visible map window.
    view_origin_x: i32,
    view_origin_y: i32,
    last_mouse_cell: Option<MouseCell>,
    viewport_edge_scroll_cooldown: u16,
    /// Baked static tile visuals (parallel to `level.tiles`).
    tile_display: Vec<TileDisplayCell>,
    map_visual_seed: u64,
    surface_tick: u64,
    /// True after any edit not yet written with Ctrl+S / save dialog.
    dirty: bool,
    /// Last known on-disk fingerprint we treated as "in sync" (load, save, reload, or dismiss prompt).
    last_disk_fingerprint: FileFingerprint,
    /// Rate-limit disk reads for hot-reload.
    last_hot_reload_poll: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    PaintTiles,
    PlaceSpawns,
    EraseSpawns,
    /// Single-cell marker: where the runtime spawns the player (`LevelFile.player_spawn`).
    SetPlayerSpawn,
}

#[derive(Clone, Copy, Debug)]
enum SidebarHit {
    Terrain(usize),
    Entity(usize),
    PlayerSpawn,
}

enum Dialog {
    SavePath {
        field: TextField,
    },
    LevelTitle {
        field: TextField,
    },
    Resize {
        w: TextField,
        h: TextField,
        focus: u8,
    },
    NewTerrain {
        name: TextField,
        glyph: TextField,
        solid: bool,
        color_idx: usize,
        focus: u8,
    },
    /// External file changed while the in-memory level has unsaved edits.
    HotReloadUnsaved,
}

impl Editor {
    fn default_level() -> LevelFile {
        let tile_defs = game_content::embedded_demo_level().tile_defs;
        let floor_tile = tile_defs
            .iter()
            .find(|d| !d.blocks_movement)
            .map_or(0, |d| d.id);
        let wall_tile = tile_defs
            .iter()
            .find(|d| d.blocks_movement)
            .map_or(floor_tile, |d| d.id);
        let w = 24u16;
        let h = 16u16;
        let n = (w as usize) * (h as usize);
        let mut tiles = vec![floor_tile; n];
        for x in 0..w {
            tiles[x as usize] = wall_tile;
            tiles[(h as usize - 1) * w as usize + x as usize] = wall_tile;
        }
        for y in 0..h {
            tiles[y as usize * w as usize] = wall_tile;
            tiles[y as usize * w as usize + (w as usize - 1)] = wall_tile;
        }
        LevelFile {
            schema_version: LevelFile::SCHEMA,
            name: "untitled".into(),
            width: w,
            height: h,
            tiles,
            tile_defs,
            spawns: vec![EntitySpawn {
                kind: "guide".into(),
                x: 10,
                y: 8,
                glyph_override: None,
                name_override: None,
                fg_override: None,
            }],
            player_spawn: Some(PlayerSpawn { x: 12, y: 8 }),
            visual_seed: None,
        }
    }

    fn load_or_new(path: &PathBuf) -> Self {
        let (level, status) = if path.exists() {
            match fs::read_to_string(path) {
                Ok(s) => match level_from_ron(&s) {
                    Ok(l) => (l, format!("Loaded {}", path.display())),
                    Err(e) => (
                        Self::default_level(),
                        format!("Parse error: {e}; new level"),
                    ),
                },
                Err(e) => (Self::default_level(), format!("Read error: {e}; new level")),
            }
        } else {
            (
                Self::default_level(),
                format!("New level ({} missing)", path.display()),
            )
        };
        let content = game_content::content_pack();
        let _ = content.validate();
        let mut status = status;
        if let Err(e) = content.validate_level(&level) {
            status.push_str(&format!(" | Check: {e}"));
        }
        let spawn_blueprint_idx = 0;
        let map_visual_seed = level
            .visual_seed
            .unwrap_or_else(|| derive_visual_seed(&level));
        let last_disk_fingerprint = FileFingerprint::from_path(path).unwrap_or(FileFingerprint::MISSING);
        let mut ed = Self {
            path: path.clone(),
            level,
            content,
            cursor_x: 4,
            cursor_y: 4,
            current_tile: 0,
            spawn_blueprint_idx,
            mode: Mode::PaintTiles,
            status,
            dialog: None,
            viewport_w: 80,
            viewport_h: 24,
            brush_radius: 0,
            rect_drag_start: None,
            last_paint_cell: None,
            sidebar_hits: Vec::new(),
            hover_map_cell: None,
            view_origin_x: 0,
            view_origin_y: 0,
            last_mouse_cell: None,
            viewport_edge_scroll_cooldown: 0,
            tile_display: Vec::new(),
            map_visual_seed,
            surface_tick: 0,
            dirty: false,
            last_disk_fingerprint,
            last_hot_reload_poll: Instant::now(),
        };
        ed.rebuild_tile_display_full();
        ed
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn refresh_disk_fingerprint(&mut self) {
        self.last_disk_fingerprint = FileFingerprint::from_path(&self.path).unwrap_or(FileFingerprint::MISSING);
    }

    /// Parse and validate `path` without mutating editor state.
    fn load_level_from_disk(path: &Path, content: &ContentPack) -> Result<LevelFile, String> {
        let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let level = level_from_ron(&raw).map_err(|e| format!("RON parse: {e}"))?;
        content
            .validate_level(&level)
            .map_err(|e| format!("level check: {e}"))?;
        Ok(level)
    }

    fn apply_reloaded_level(&mut self, new_level: LevelFile) {
        self.level = new_level;
        self.map_visual_seed = self
            .level
            .visual_seed
            .unwrap_or_else(|| derive_visual_seed(&self.level));
        self.dirty = false;
        self.refresh_disk_fingerprint();
        self.cursor_x = self
            .cursor_x
            .clamp(0, self.level.width as i32 - 1);
        self.cursor_y = self
            .cursor_y
            .clamp(0, self.level.height as i32 - 1);
        if self.level.tile_defs.is_empty() {
            self.current_tile = 0;
        } else if !self.level.tile_defs.iter().any(|d| d.id == self.current_tile) {
            self.current_tile = self.level.tile_defs[0].id;
        }
        let n = self.content.entity_blueprints.len();
        if n > 0 {
            self.spawn_blueprint_idx = self.spawn_blueprint_idx.min(n - 1);
        } else {
            self.spawn_blueprint_idx = 0;
        }
        self.clamp_editor_view();
        self.ensure_cursor_visible();
        self.rebuild_tile_display_full();
    }

    fn try_hot_reload_replace(&mut self) -> Result<(), String> {
        let new_level = Self::load_level_from_disk(&self.path, &self.content)?;
        self.apply_reloaded_level(new_level);
        self.status = format!("Hot-reloaded {}", self.path.display());
        Ok(())
    }

    /// Called every frame from the main loop. Reloads when the file changes on disk if safe.
    fn poll_hot_reload(&mut self) {
        const INTERVAL: Duration = Duration::from_millis(250);
        if self.last_hot_reload_poll.elapsed() < INTERVAL {
            return;
        }
        self.last_hot_reload_poll = Instant::now();
        if self.dialog.is_some() {
            return;
        }
        let Some(disk_fp) = FileFingerprint::from_path(&self.path) else {
            return;
        };
        if disk_fp == self.last_disk_fingerprint {
            return;
        }
        if !self.dirty {
            match self.try_hot_reload_replace() {
                Ok(()) => {}
                Err(e) => {
                    self.status = format!("Hot-reload skipped: {e}");
                    self.last_disk_fingerprint = disk_fp;
                }
            }
            return;
        }
        self.dialog = Some(Dialog::HotReloadUnsaved);
        self.status =
            "File changed on disk (unsaved edits). Y: reload & discard   N/Esc: keep editing."
                .into();
    }

    fn rebuild_tile_display_full(&mut self) {
        let Ok(mut m) = self.level.to_map() else {
            self.tile_display.clear();
            return;
        };
        m.rebuild_display_cache(self.map_visual_seed);
        self.tile_display = m.display;
    }

    fn save(&mut self) -> Result<(), String> {
        self.content
            .validate_level(&self.level)
            .map_err(|e| e.to_string())?;
        let s = level_to_ron(&self.level).map_err(|e| e.to_string())?;
        fs::write(&self.path, s).map_err(|e| e.to_string())?;
        self.dirty = false;
        self.refresh_disk_fingerprint();
        self.status = format!("Saved {}", self.path.display());
        Ok(())
    }

    fn cycle_tile_palette(&mut self, delta: i32) {
        if self.level.tile_defs.is_empty() {
            return;
        }
        let n = self.level.tile_defs.len() as i32;
        let pos = self
            .level
            .tile_defs
            .iter()
            .position(|d| d.id == self.current_tile)
            .unwrap_or(0) as i32;
        let next = (pos + delta).rem_euclid(n) as usize;
        self.current_tile = self.level.tile_defs[next].id;
    }

    fn cycle_spawn_blueprint(&mut self, delta: i32) {
        let n = self.content.entity_blueprints.len() as i32;
        if n == 0 {
            return;
        }
        self.spawn_blueprint_idx = (self.spawn_blueprint_idx as i32 + delta).rem_euclid(n) as usize;
    }

    fn current_tile_def(&self) -> Option<&TileDef> {
        self.level
            .tile_defs
            .iter()
            .find(|d| d.id == self.current_tile)
    }

    fn current_spawn_blueprint(&self) -> Option<&'static EntityBlueprint> {
        self.content.entity_blueprints.get(self.spawn_blueprint_idx)
    }

    fn spawn_glyph(&self, spawn: &EntitySpawn) -> char {
        spawn.glyph_override.unwrap_or_else(|| {
            self.content
                .blueprint(spawn.kind.as_str())
                .map_or('?', |bp| bp.default_glyph)
        })
    }

    fn spawn_fg(&self, spawn: &EntitySpawn) -> Color {
        spawn.fg_override.unwrap_or_else(|| {
            self.content
                .blueprint(spawn.kind.as_str())
                .map_or(Color::rgb(255, 160, 80), |bp| bp.default_fg.to_render_color())
        })
    }

    fn resize_level(&mut self, nw: u16, nh: u16) {
        let ow = self.level.width as usize;
        let oh = self.level.height as usize;
        let mut new_tiles = vec![0u16; nw as usize * nh as usize];
        for y in 0..nh as usize {
            for x in 0..nw as usize {
                let t = if x < ow && y < oh {
                    self.level.tiles[y * ow + x]
                } else {
                    0
                };
                new_tiles[y * nw as usize + x] = t;
            }
        }
        self.level.width = nw;
        self.level.height = nh;
        self.level.tiles = new_tiles;
        self.mark_dirty();
        self.level
            .spawns
            .retain(|s| s.x >= 0 && s.y >= 0 && (s.x as u16) < nw && (s.y as u16) < nh);
        if let Some(ps) = self.level.player_spawn {
            if ps.x < 0
                || ps.y < 0
                || (ps.x as u16) >= nw
                || (ps.y as u16) >= nh
            {
                self.level.player_spawn = None;
            }
        }
        self.cursor_x = self.cursor_x.clamp(0, nw as i32 - 1);
        self.cursor_y = self.cursor_y.clamp(0, nh as i32 - 1);
        self.clamp_editor_view();
        self.ensure_cursor_visible();
        self.rebuild_tile_display_full();
    }

    fn sidebar_screen_width(&self) -> u16 {
        EDITOR_SIDEBAR_WIDTH
            .min(self.viewport_w.saturating_sub(4))
            .max(10)
    }

    fn map_area_rect(&self) -> Rect {
        let sw = self.sidebar_screen_width();
        let mw = self.viewport_w.saturating_sub(sw).max(1);
        Rect::new(0, 0, mw, self.viewport_h)
    }

    fn sidebar_rect(&self) -> Rect {
        let map = self.map_area_rect();
        let sw = self.viewport_w.saturating_sub(map.w).max(1);
        Rect::new(map.right(), map.y, sw, map.h)
    }

    fn clamp_editor_view(&mut self) {
        let map = self.map_area_rect();
        let vw = map.w as i32;
        let vh = map.h as i32;
        let mw = self.level.width as i32;
        let mh = self.level.height as i32;
        let max_ox = (mw - vw).max(0);
        let max_oy = (mh - vh).max(0);
        self.view_origin_x = self.view_origin_x.clamp(0, max_ox);
        self.view_origin_y = self.view_origin_y.clamp(0, max_oy);
    }

    fn editor_map_needs_scroll(&self) -> bool {
        let map = self.map_area_rect();
        self.level.width as i32 > map.w as i32 || self.level.height as i32 > map.h as i32
    }

    fn ensure_cursor_visible(&mut self) {
        let map = self.map_area_rect();
        let vw = map.w as i32;
        let vh = map.h as i32;
        let cx = self.cursor_x;
        let cy = self.cursor_y;
        if cx < self.view_origin_x {
            self.view_origin_x = cx;
        }
        if cy < self.view_origin_y {
            self.view_origin_y = cy;
        }
        if cx >= self.view_origin_x + vw {
            self.view_origin_x = cx - vw + 1;
        }
        if cy >= self.view_origin_y + vh {
            self.view_origin_y = cy - vh + 1;
        }
        self.clamp_editor_view();
    }

    fn idle_viewport_tick(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        let map = self.map_area_rect();
        let Some(cell) = self.last_mouse_cell else {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        };
        if !map.contains(cell.x, cell.y) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if !self.editor_map_needs_scroll() {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        let lx = i32::from(cell.x.saturating_sub(map.x));
        let ly = i32::from(cell.y.saturating_sub(map.y));
        let (pdx, pdy) = edge_scroll_pan_delta(lx, ly, map.w, map.h);
        if (pdx, pdy) == (0, 0) {
            self.viewport_edge_scroll_cooldown = 0;
            return;
        }
        if self.viewport_edge_scroll_cooldown > 0 {
            self.viewport_edge_scroll_cooldown =
                self.viewport_edge_scroll_cooldown.saturating_sub(1);
            return;
        }
        self.view_origin_x += pdx;
        self.view_origin_y += pdy;
        self.clamp_editor_view();
        self.viewport_edge_scroll_cooldown = EDGE_SCROLL_COOLDOWN_TICKS;
    }

    fn next_tile_id(&self) -> TileId {
        self.level
            .tile_defs
            .iter()
            .map(|d| d.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            match ev {
                InputEvent::Key(chord) => {
                    if self.handle_dialog(chord) {
                        continue;
                    }
                    self.step_main_key(chord);
                }
                InputEvent::Mouse { .. } => {
                    if self.dialog.is_some() {
                        continue;
                    }
                    self.step_main_mouse(ev);
                }
                InputEvent::Resize { .. } => {}
            }
        }
    }

    fn sidebar_pick(&self, cell: MouseCell) -> Option<SidebarHit> {
        self.sidebar_hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(cell.x, cell.y))
            .map(|(h, _)| *h)
    }

    fn set_tile_clamped(&mut self, tx: i32, ty: i32, tile: TileId) {
        let w = self.level.width as i32;
        let h = self.level.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            return;
        }
        let i = ty as usize * self.level.width as usize + tx as usize;
        if i < self.level.tiles.len() {
            self.level.tiles[i] = tile;
            self.mark_dirty();
        }
    }

    fn apply_paint_brush(&mut self, cx: i32, cy: i32) {
        let t = self.current_tile;
        for_each_in_brush(cx, cy, self.brush_radius, |tx, ty| {
            self.set_tile_clamped(tx, ty, t);
        });
        self.rebuild_tile_display_full();
    }

    fn fill_rect_tiles(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        if self.mode != Mode::PaintTiles {
            return;
        }
        let t = self.current_tile;
        for_each_in_rect(x0, y0, x1, y1, |tx, ty| {
            self.set_tile_clamped(tx, ty, t);
        });
        self.status = format!("Filled tiles ({x0},{y0})—({x1},{y1}).");
        self.mark_dirty();
        self.rebuild_tile_display_full();
    }

    fn cell_has_spawn(&self, tx: i32, ty: i32) -> bool {
        self.level.spawns.iter().any(|s| s.x == tx && s.y == ty)
    }

    /// Remove every spawn whose cell lies in the brush around `(cx, cy)`.
    fn remove_spawns_in_brush(&mut self, cx: i32, cy: i32) -> usize {
        let r = self.brush_radius;
        let before = self.level.spawns.len();
        self.level
            .spawns
            .retain(|s| !cell_in_brush(s.x, s.y, cx, cy, r));
        let removed = before.saturating_sub(self.level.spawns.len());
        if removed > 0 {
            self.mark_dirty();
        }
        removed
    }

    /// Remove every spawn in the inclusive axis-aligned rectangle.
    fn remove_spawns_in_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) -> usize {
        let before = self.level.spawns.len();
        self.level
            .spawns
            .retain(|s| !cell_in_axis_rect(s.x, s.y, x0, y0, x1, y1));
        let removed = before.saturating_sub(self.level.spawns.len());
        if removed > 0 {
            self.mark_dirty();
        }
        removed
    }

    fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::PaintTiles => Mode::PlaceSpawns,
            Mode::PlaceSpawns => Mode::EraseSpawns,
            Mode::EraseSpawns => Mode::SetPlayerSpawn,
            Mode::SetPlayerSpawn => Mode::PaintTiles,
        };
        self.status = format!("Mode: {:?}", self.mode);
    }

    fn set_player_spawn_at(&mut self, tx: i32, ty: i32) {
        let w = self.level.width as i32;
        let h = self.level.height as i32;
        let x = tx.clamp(0, w - 1);
        let y = ty.clamp(0, h - 1);
        self.cursor_x = x;
        self.cursor_y = y;
        self.level.player_spawn = Some(PlayerSpawn { x, y });
        self.mark_dirty();
        self.status = format!("Player spawn set to ({x},{y}).");
        self.ensure_cursor_visible();
    }

    fn clear_player_spawn(&mut self) {
        self.level.player_spawn = None;
        self.mark_dirty();
        self.status = "Player spawn cleared (game will use map center).".into();
    }

    fn place_spawn_at(&mut self, tx: i32, ty: i32) {
        let Some(bp) = self.current_spawn_blueprint() else {
            self.status = "No entity blueprints in content pack.".into();
            return;
        };
        self.cursor_x = tx.clamp(0, self.level.width as i32 - 1);
        self.cursor_y = ty.clamp(0, self.level.height as i32 - 1);
        self.level.spawns.push(EntitySpawn {
            kind: bp.kind.to_string(),
            x: self.cursor_x,
            y: self.cursor_y,
            glyph_override: None,
            name_override: None,
            fg_override: None,
        });
        self.mark_dirty();
        self.status = format!(
            "Spawn {} at ({}, {}).",
            bp.kind, self.cursor_x, self.cursor_y
        );
        self.ensure_cursor_visible();
    }

    fn step_main_mouse(&mut self, ev: &InputEvent) {
        let InputEvent::Mouse {
            kind,
            cell,
            shift,
            ctrl: _,
            alt: _,
            ..
        } = ev
        else {
            return;
        };
        self.last_mouse_cell = Some(*cell);
        let map_rect = self.map_area_rect();

        self.hover_map_cell = cell_local_in_rect(*cell, map_rect).map(|(lx, ly)| {
            (
                self.view_origin_x + lx,
                self.view_origin_y + ly,
            )
        });

        if matches!(kind, MouseEventKind::Moved) {
            return;
        }

        if matches!(kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
            if cell_local_in_rect(*cell, map_rect).is_some() {
                if matches!(kind, MouseEventKind::ScrollUp) {
                    self.brush_radius = (self.brush_radius + 1).min(4);
                } else {
                    self.brush_radius = self.brush_radius.saturating_sub(1);
                }
                self.status = format!("Brush radius {}", self.brush_radius);
            }
            return;
        }

        if let Some(hit) = self.sidebar_pick(*cell) {
            if let MouseEventKind::Down(MouseButton::Left) = kind {
                match hit {
                    SidebarHit::Terrain(i) => {
                        if let Some(d) = self.level.tile_defs.get(i) {
                            self.current_tile = d.id;
                            self.mode = Mode::PaintTiles;
                            self.status = format!("Brush: {} ({})", d.name, d.id);
                        }
                    }
                    SidebarHit::Entity(i) => {
                        if i < self.content.entity_blueprints.len() {
                            self.spawn_blueprint_idx = i;
                            self.mode = Mode::PlaceSpawns;
                            let bp = &self.content.entity_blueprints[i];
                            self.status = format!("Place: {}", bp.kind);
                        }
                    }
                    SidebarHit::PlayerSpawn => {
                        self.mode = Mode::SetPlayerSpawn;
                        self.status = "Player spawn: click map or Space at cursor.".into();
                    }
                }
            }
            return;
        }

        let Some((lx, ly)) = cell_local_in_rect(*cell, map_rect) else {
            return;
        };
        let tx = self.view_origin_x + lx;
        let ty = self.view_origin_y + ly;

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if *shift {
                    self.rect_drag_start = Some((tx, ty));
                    self.last_paint_cell = None;
                } else {
                    self.rect_drag_start = None;
                    match self.mode {
                        Mode::PaintTiles => {
                            self.apply_paint_brush(tx, ty);
                            self.status = format!("Paint at ({tx},{ty}) r{}.", self.brush_radius);
                        }
                        Mode::PlaceSpawns => self.place_spawn_at(tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_brush(tx, ty);
                            self.status = format!(
                                "Removed {n} spawn(s) at ({tx},{ty}) r{}.",
                                self.brush_radius
                            );
                        }
                        Mode::SetPlayerSpawn => self.set_player_spawn_at(tx, ty),
                    }
                    self.last_paint_cell = Some((tx, ty));
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.rect_drag_start.is_some() {
                    return;
                }
                if self.last_paint_cell != Some((tx, ty)) {
                    match self.mode {
                        Mode::PaintTiles => {
                            self.apply_paint_brush(tx, ty);
                            self.status = format!("Paint drag ({tx},{ty}).");
                        }
                        Mode::PlaceSpawns => self.place_spawn_at(tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_brush(tx, ty);
                            if n > 0 {
                                self.status = format!("Removed {n} spawn(s) (drag).");
                            }
                        }
                        Mode::SetPlayerSpawn => self.set_player_spawn_at(tx, ty),
                    }
                    self.last_paint_cell = Some((tx, ty));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((sx, sy)) = self.rect_drag_start.take() {
                    match self.mode {
                        Mode::PaintTiles => self.fill_rect_tiles(sx, sy, tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_rect(sx, sy, tx, ty);
                            self.status = format!(
                                "Removed {n} spawn(s) in rectangle ({sx},{sy})—({tx},{ty})."
                            );
                        }
                        Mode::PlaceSpawns | Mode::SetPlayerSpawn => {}
                    }
                }
                self.last_paint_cell = None;
            }
            _ => {}
        }
    }

    fn handle_dialog(&mut self, chord: &KeyChord) -> bool {
        if chord.ctrl && matches!(chord.key, Key::Char('q')) {
            self.dialog = None;
            self.status = "QUIT".into();
            return true;
        }
        if matches!(&self.dialog, Some(Dialog::HotReloadUnsaved)) {
            if matches!(chord.key, Key::Char('y') | Key::Char('Y')) {
                self.dialog = None;
                match self.try_hot_reload_replace() {
                    Ok(()) => {}
                    Err(e) => {
                        self.status = format!("Hot-reload failed: {e}");
                        self.mark_dirty();
                        self.refresh_disk_fingerprint();
                    }
                }
                return true;
            }
            if matches!(
                chord.key,
                Key::Char('n') | Key::Char('N') | Key::Esc
            ) {
                self.dialog = None;
                self.refresh_disk_fingerprint();
                self.status = "Kept in-memory edits; ignored this disk revision.".into();
                return true;
            }
            return true;
        }
        if matches!(chord.key, Key::Enter)
            && !chord.ctrl
            && matches!(&self.dialog, Some(Dialog::NewTerrain { .. }))
        {
            if let Some(Dialog::NewTerrain {
                name,
                glyph,
                solid,
                color_idx,
                ..
            }) = self.dialog.take()
            {
                let gch = glyph.text.chars().next().unwrap_or('.');
                let mut n = name.text.trim().to_string();
                if n.is_empty() {
                    n = "terrain".into();
                }
                let fg = PRESET_COLORS[color_idx % PRESET_COLORS.len()];
                let id = self.next_tile_id();
                let def = TileDef {
                    id,
                    glyph: gch,
                    blocks_movement: solid,
                    blocks_sight: solid,
                    name: n.clone(),
                    fg,
                    connect_mask: 0,
                    surface: None,
                };
                self.level.tile_defs.push(def);
                self.current_tile = id;
                self.status = format!("Added tile id {id} ({n}).");
                self.mark_dirty();
                self.rebuild_tile_display_full();
                return true;
            }
        }
        let Some(d) = self.dialog.as_mut() else {
            return false;
        };
        match d {
            Dialog::SavePath { field } => {
                if matches!(chord.key, Key::Enter) && !chord.ctrl {
                    let s = field.text.trim();
                    if s.is_empty() {
                        self.status = "Filename cannot be empty.".into();
                    } else {
                        let mut p = PathBuf::from(s);
                        if p.extension().is_none()
                            || p.extension() == Some(std::ffi::OsStr::new(""))
                        {
                            p.set_extension("ron");
                        }
                        self.path = p;
                        if let Err(e) = self.save() {
                            self.status = e;
                        }
                        self.dialog = None;
                    }
                    return true;
                }
                match field.apply_key(chord) {
                    TextFieldOutput::Cancel => self.dialog = None,
                    TextFieldOutput::Tab => {}
                    TextFieldOutput::Edited => {}
                }
                true
            }
            Dialog::LevelTitle { field } => {
                if matches!(chord.key, Key::Enter) && !chord.ctrl {
                    self.level.name = field.text.trim().to_string();
                    if self.level.name.is_empty() {
                        self.level.name = "untitled".into();
                    }
                    self.status = format!("Level name: {}", self.level.name);
                    self.mark_dirty();
                    self.dialog = None;
                    return true;
                }
                match field.apply_key(chord) {
                    TextFieldOutput::Cancel => self.dialog = None,
                    TextFieldOutput::Tab => {}
                    TextFieldOutput::Edited => {}
                }
                true
            }
            Dialog::Resize { w, h, focus } => {
                if matches!(chord.key, Key::Tab) && !chord.ctrl {
                    *focus = (*focus + 1) % 2;
                    return true;
                }
                if matches!(chord.key, Key::Enter) && !chord.ctrl {
                    let parse = |t: &TextField| -> Option<u16> {
                        let n: u32 = t.text.trim().parse().ok()?;
                        if (3..=512).contains(&n) {
                            Some(n as u16)
                        } else {
                            None
                        }
                    };
                    match (parse(w), parse(h)) {
                        (Some(nw), Some(nh)) => {
                            self.resize_level(nw, nh);
                            self.status = format!("Resized to {nw}x{nh}.");
                            self.dialog = None;
                        }
                        _ => {
                            self.status = "Width/height must be integers from 3 to 512.".into();
                        }
                    }
                    return true;
                }
                let active = if *focus == 0 { w } else { h };
                match active.apply_key(chord) {
                    TextFieldOutput::Cancel => self.dialog = None,
                    TextFieldOutput::Tab => *focus = (*focus + 1) % 2,
                    TextFieldOutput::Edited => {}
                }
                true
            }
            Dialog::NewTerrain {
                name,
                glyph,
                solid,
                color_idx,
                focus,
            } => {
                if matches!(chord.key, Key::Tab) && !chord.ctrl {
                    *focus = (*focus + 1) % 4;
                    return true;
                }
                match *focus {
                    0 => match name.apply_key(chord) {
                        TextFieldOutput::Cancel => self.dialog = None,
                        TextFieldOutput::Tab => *focus = (*focus + 1) % 4,
                        TextFieldOutput::Edited => {}
                    },
                    1 => match glyph.apply_key(chord) {
                        TextFieldOutput::Cancel => self.dialog = None,
                        TextFieldOutput::Tab => *focus = (*focus + 1) % 4,
                        TextFieldOutput::Edited => {}
                    },
                    2 => match chord.key {
                        Key::Char(' ') if !chord.ctrl => {
                            *solid = !*solid;
                        }
                        Key::Esc => self.dialog = None,
                        Key::Tab if !chord.ctrl => *focus = (*focus + 1) % 4,
                        _ => {}
                    },
                    3 => match chord.key {
                        Key::Left if !chord.ctrl => {
                            *color_idx = color_idx.saturating_sub(1);
                        }
                        Key::Right if !chord.ctrl => {
                            *color_idx = (*color_idx + 1).min(PRESET_COLORS.len() - 1);
                        }
                        Key::Esc => self.dialog = None,
                        Key::Tab if !chord.ctrl => *focus = (*focus + 1) % 4,
                        _ => {}
                    },
                    _ => *focus = 0,
                }
                true
            }
            Dialog::HotReloadUnsaved => true,
        }
    }

    fn step_main_key(&mut self, chord: &KeyChord) {
        match chord {
            KeyChord {
                key: Key::Tab,
                ctrl: false,
                ..
            } => {
                self.cycle_mode();
            }
            KeyChord {
                key: Key::Char('=') | Key::Char('+'),
                ctrl: false,
                ..
            } => {
                self.brush_radius = (self.brush_radius + 1).min(4);
                self.status = format!("Brush radius {}", self.brush_radius);
            }
            KeyChord {
                key: Key::Char('-') | Key::Char('_'),
                ctrl: false,
                ..
            } => {
                self.brush_radius = self.brush_radius.saturating_sub(1);
                self.status = format!("Brush radius {}", self.brush_radius);
            }
            KeyChord {
                key: Key::Esc,
                ctrl: false,
                ..
            } => {
                self.rect_drag_start = None;
                self.last_paint_cell = None;
            }
            KeyChord {
                key: Key::Char('s'),
                ctrl: true,
                ..
            } => {
                if let Err(e) = self.save() {
                    self.status = e;
                }
            }
            KeyChord {
                key: Key::Char('m'),
                ctrl: false,
                ..
            } => {
                self.cycle_mode();
            }
            KeyChord {
                key: Key::Char('p'),
                ctrl: false,
                ..
            } => {
                self.mode = Mode::SetPlayerSpawn;
                self.status = "Player spawn: click map or Space at cursor.".into();
            }
            KeyChord {
                key: Key::Char(' '),
                ctrl: false,
                ..
            } => match self.mode {
                Mode::PaintTiles => {
                    self.apply_paint_brush(self.cursor_x, self.cursor_y);
                    self.status = format!(
                        "Paint at ({},{}) r{}.",
                        self.cursor_x, self.cursor_y, self.brush_radius
                    );
                }
                Mode::PlaceSpawns => {
                    let Some(bp) = self.current_spawn_blueprint() else {
                        self.status = "No entity blueprints in content pack.".into();
                        return;
                    };
                    self.level.spawns.push(EntitySpawn {
                        kind: bp.kind.to_string(),
                        x: self.cursor_x,
                        y: self.cursor_y,
                        glyph_override: None,
                        name_override: None,
                        fg_override: None,
                    });
                    self.mark_dirty();
                    self.status = format!(
                        "Spawn {} at ({}, {}).",
                        bp.kind, self.cursor_x, self.cursor_y
                    );
                }
                Mode::EraseSpawns => {
                    let n = self.remove_spawns_in_brush(self.cursor_x, self.cursor_y);
                    self.status = format!(
                        "Removed {n} spawn(s) at ({},{}) r{}.",
                        self.cursor_x, self.cursor_y, self.brush_radius
                    );
                }
                Mode::SetPlayerSpawn => {
                    self.set_player_spawn_at(self.cursor_x, self.cursor_y);
                }
            },
            KeyChord {
                key: Key::Backspace,
                ctrl: false,
                ..
            } if self.mode == Mode::SetPlayerSpawn => {
                self.clear_player_spawn();
            },
            KeyChord {
                key: Key::Char('[') | Key::Char('k'),
                ctrl: false,
                ..
            } => match self.mode {
                Mode::PaintTiles => self.cycle_tile_palette(-1),
                Mode::PlaceSpawns => self.cycle_spawn_blueprint(-1),
                Mode::EraseSpawns | Mode::SetPlayerSpawn => {}
            },
            KeyChord {
                key: Key::Char(']') | Key::Char('j'),
                ctrl: false,
                ..
            } => match self.mode {
                Mode::PaintTiles => self.cycle_tile_palette(1),
                Mode::PlaceSpawns => self.cycle_spawn_blueprint(1),
                Mode::EraseSpawns | Mode::SetPlayerSpawn => {}
            },
            KeyChord {
                key: Key::F(2),
                ctrl: false,
                ..
            } => {
                let initial = self.path.to_string_lossy().into_owned();
                self.dialog = Some(Dialog::SavePath {
                    field: TextField::new(96, initial, TextFilter::Text),
                });
            }
            KeyChord {
                key: Key::F(3),
                ctrl: false,
                ..
            } => {
                self.dialog = Some(Dialog::LevelTitle {
                    field: TextField::new(64, self.level.name.as_str(), TextFilter::Text),
                });
            }
            KeyChord {
                key: Key::F(4),
                ctrl: false,
                ..
            } => {
                self.dialog = Some(Dialog::Resize {
                    w: TextField::new(5, format!("{}", self.level.width), TextFilter::Digits),
                    h: TextField::new(5, format!("{}", self.level.height), TextFilter::Digits),
                    focus: 0,
                });
            }
            KeyChord {
                key: Key::F(5),
                ctrl: false,
                ..
            } => {
                self.dialog = Some(Dialog::NewTerrain {
                    name: TextField::new(32, "", TextFilter::Text),
                    glyph: TextField::new(1, "", TextFilter::Text),
                    solid: true,
                    color_idx: 0,
                    focus: 0,
                });
            }
            KeyChord {
                key: Key::Up,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_y -= 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Down,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_y += 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Left,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_x -= 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Right,
                ctrl: false,
                ..
            } if self.editor_map_needs_scroll() => {
                self.view_origin_x += 1;
                self.clamp_editor_view();
            }
            KeyChord {
                key: Key::Char('w'),
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y - 1).max(0);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Up,
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y - 1).max(0);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Char('s'),
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y + 1).min(self.level.height as i32 - 1);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Down,
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y + 1).min(self.level.height as i32 - 1);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Char('a'),
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x - 1).max(0);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Left,
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x - 1).max(0);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Char('d'),
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x + 1).min(self.level.width as i32 - 1);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Right,
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x + 1).min(self.level.width as i32 - 1);
                self.ensure_cursor_visible();
            }
            KeyChord {
                key: Key::Char('q'),
                ctrl: true,
                ..
            } => {
                self.status = "QUIT".into();
            }
            _ => {}
        }
    }

    fn compose(&mut self, fb: &mut FrameBuffer) {
        self.viewport_w = fb.width;
        self.viewport_h = fb.height;
        self.clamp_editor_view();
        self.surface_tick = self.surface_tick.wrapping_add(1);
        self.sidebar_hits.clear();

        let fg = Color::rgb(210, 210, 200);
        let bg = Color::rgb(12, 12, 18);
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.set(
                    x,
                    y,
                    Cell {
                        ch: ' ',
                        fg,
                        bg,
                        style: Style::default(),
                    },
                );
            }
        }
        let map = self.map_area_rect();
        let ox = map.x;
        let oy = map.y;
        let vw = map.w as usize;
        let vh = map.h as usize;
        let vo_x = self.view_origin_x;
        let vo_y = self.view_origin_y;
        let lw = self.level.width as i32;
        let lh = self.level.height as i32;

        for j in 0..vh {
            for i in 0..vw {
                let tx = vo_x + i as i32;
                let ty = vo_y + j as i32;
                let sx = ox.saturating_add(i as u16);
                let sy = oy.saturating_add(j as u16);
                if tx < 0 || ty < 0 || tx >= lw || ty >= lh {
                    fb.set(
                        sx,
                        sy,
                        Cell {
                            ch: ' ',
                            fg,
                            bg,
                            style: Style::default(),
                        },
                    );
                    continue;
                }
                let wi = self.level.width as usize;
                let idx = ty as usize * wi + tx as usize;
                let tid = self.level.tiles[idx];
                let def = self.level.tile_defs.iter().find(|d| d.id == tid);
                let baked = self.tile_display.get(idx).copied();
                let (ch, tile_fg) = match def {
                    Some(d) if def_is_animated(d) => {
                        let r = resolve_animated(
                            d,
                            tx,
                            ty,
                            self.surface_tick,
                            self.map_visual_seed,
                        );
                        (r.ch, r.fg)
                    }
                    _ => {
                        let d = baked.unwrap_or(TileDisplayCell {
                            ch: '?',
                            fg: Color::rgb(200, 190, 170),
                        });
                        (d.ch, d.fg)
                    }
                };
                let mut c = Cell {
                    ch,
                    fg: tile_fg,
                    bg,
                    style: Style::default(),
                };
                if self.dialog.is_none() {
                    if let Some((hx, hy)) = self.hover_map_cell {
                        let txi = tx;
                        let tyi = ty;
                        let mut lift: u8 = 0;
                        match self.mode {
                            Mode::PaintTiles => {
                                if cell_in_brush(txi, tyi, hx, hy, self.brush_radius) {
                                    lift = lift.max(14);
                                }
                                if let Some((sx, sy)) = self.rect_drag_start {
                                    if cell_in_axis_rect(txi, tyi, sx, sy, hx, hy) {
                                        lift = lift.max(20);
                                    }
                                }
                            }
                            Mode::PlaceSpawns => {
                                if txi == hx && tyi == hy {
                                    lift = lift.max(14);
                                }
                            }
                            Mode::SetPlayerSpawn => {
                                if txi == hx && tyi == hy {
                                    lift = lift.max(16);
                                }
                            }
                            Mode::EraseSpawns => {
                                if cell_in_brush(txi, tyi, hx, hy, self.brush_radius) {
                                    let mut l: u8 = 12;
                                    if self.cell_has_spawn(txi, tyi) {
                                        l = l.max(28);
                                    }
                                    lift = lift.max(l);
                                }
                                if let Some((sx, sy)) = self.rect_drag_start {
                                    if cell_in_axis_rect(txi, tyi, sx, sy, hx, hy) {
                                        let mut l: u8 = 10;
                                        if self.cell_has_spawn(txi, tyi) {
                                            l = l.max(26);
                                        }
                                        lift = lift.max(l);
                                    }
                                }
                            }
                        }
                        if lift > 0 {
                            c.bg = c.bg.lighten(lift);
                        }
                    }
                }
                if tx == self.cursor_x && ty == self.cursor_y {
                    c.style.bold = true;
                    c.fg = Color::rgb(255, 255, 120);
                }
                fb.set(sx, sy, c);
            }
        }
        for s in &self.level.spawns {
            if s.x >= 0
                && s.y >= 0
                && (s.x as u16) < self.level.width
                && (s.y as u16) < self.level.height
            {
                if s.x < vo_x
                    || s.y < vo_y
                    || s.x >= vo_x + vw as i32
                    || s.y >= vo_y + vh as i32
                {
                    continue;
                }
                let mut spawn_bg = bg;
                if self.dialog.is_none() && self.mode == Mode::EraseSpawns {
                    if let Some((hx, hy)) = self.hover_map_cell {
                        if cell_in_brush(s.x, s.y, hx, hy, self.brush_radius) {
                            spawn_bg = spawn_bg.lighten(18);
                        }
                        if let Some((sx, sy)) = self.rect_drag_start {
                            if cell_in_axis_rect(s.x, s.y, sx, sy, hx, hy) {
                                spawn_bg = spawn_bg.lighten(14);
                            }
                        }
                    }
                }
                let c = Cell {
                    ch: self.spawn_glyph(s),
                    fg: self.spawn_fg(s),
                    bg: spawn_bg,
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                };
                let px = ox.saturating_add((s.x - vo_x) as u16);
                let py = oy.saturating_add((s.y - vo_y) as u16);
                fb.set(px, py, c);
            }
        }
        if let Some(ps) = self.level.player_spawn {
            if ps.x >= 0
                && ps.y >= 0
                && (ps.x as u16) < self.level.width
                && (ps.y as u16) < self.level.height
            {
                if ps.x >= vo_x
                    && ps.y >= vo_y
                    && ps.x < vo_x + vw as i32
                    && ps.y < vo_y + vh as i32
                {
                    let px = ox.saturating_add((ps.x - vo_x) as u16);
                    let py = oy.saturating_add((ps.y - vo_y) as u16);
                    let mut spawn_bg = bg;
                    if self.dialog.is_none() && self.mode == Mode::SetPlayerSpawn {
                        if let Some((hx, hy)) = self.hover_map_cell {
                            if ps.x == hx && ps.y == hy {
                                spawn_bg = spawn_bg.lighten(20);
                            }
                        }
                    }
                    let c = Cell {
                        ch: '@',
                        fg: Color::rgb(120, 220, 255),
                        bg: spawn_bg,
                        style: Style {
                            bold: true,
                            dim: false,
                            underline: false,
                        },
                    };
                    fb.set(px, py, c);
                }
            }
        }
        self.compose_sidebar(fb, self.sidebar_rect());

        if let Some(ref d) = self.dialog {
            self.draw_dialog_layer(fb, d);
        }
    }

    fn compose_sidebar(&mut self, fb: &mut FrameBuffer, help: Rect) {
        let inner = Rect::new(
            help.x.saturating_add(1),
            help.y.saturating_add(1),
            help.w.saturating_sub(2),
            help.h.saturating_sub(2),
        );
        let wlim = inner.right().saturating_sub(inner.x) as usize;
        let mut y = inner.y;
        let row = |s: &str| trunc_visual(s, wlim);
        Self::sidebar_plain(fb, inner, &mut y, &row(&self.status));
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!("File: {}", self.path.display())),
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!("Level: {}", self.level.name)),
        );
        Self::sidebar_plain(fb, inner, &mut y, "");
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "WASD cursor  Arrows: pan (large map)  Tab/m: mode",
        );
        Self::sidebar_plain(fb, inner, &mut y, "p: player start   Space/L: act  +/- wheel: r");
        Self::sidebar_plain(fb, inner, &mut y, "Shift+L drag: rect (tiles or spawns)");
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "Sidebar: terrain / entity pick (paint|place)",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            "F2-F5 dialogs  C-S save  Esc clear drag  ext. edit: hot-reload",
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(&format!("Mode: {:?}  r{}", self.mode, self.brush_radius)),
        );
        Self::sidebar_plain(
            fb,
            inner,
            &mut y,
            &row(if self.dirty {
                "Edits: unsaved (hot-reload asks before replace)"
            } else {
                "Edits: saved / clean (external edits auto-reload)"
            }),
        );
        Self::sidebar_plain(fb, inner, &mut y, "");

        Self::sidebar_plain(fb, inner, &mut y, "-- Terrain --");
        let n_terrains = self.level.tile_defs.len();
        for ti in 0..n_terrains {
            let def = self.level.tile_defs[ti].clone();
            let sel = def.id == self.current_tile && self.mode == Mode::PaintTiles;
            self.sidebar_tile_row(fb, inner, &mut y, ti, &def, sel);
        }
        if self.mode == Mode::PaintTiles {
            if let Some(d) = self.current_tile_def() {
                let brush = format!(
                    "> Brush id {} glyph '{}' {} {}",
                    d.id,
                    d.glyph,
                    if d.solid() { "solid" } else { "open" },
                    row(&d.name)
                );
                Self::sidebar_plain(fb, inner, &mut y, &row(&brush));
            }
        }
        Self::sidebar_plain(fb, inner, &mut y, "");

        Self::sidebar_plain(fb, inner, &mut y, "-- Entities --");
        if self.content.entity_blueprints.is_empty() {
            Self::sidebar_plain(fb, inner, &mut y, "(no blueprints)");
        } else {
            for (i, bp) in self.content.entity_blueprints.iter().enumerate() {
                let sel = i == self.spawn_blueprint_idx && self.mode == Mode::PlaceSpawns;
                self.sidebar_entity_row(fb, inner, &mut y, i, bp, sel);
            }
        }
        if self.mode == Mode::PlaceSpawns {
            if let Some(bp) = self.current_spawn_blueprint() {
                let hook = bp
                    .dialogue_id
                    .map(|d| format!(" dialogue:{d}"))
                    .unwrap_or_default();
                let place = format!(
                    "> Place kind:{} glyph:{} {}{}",
                    bp.kind, bp.default_glyph, bp.display_name, hook
                );
                Self::sidebar_plain(fb, inner, &mut y, &row(&place));
                Self::sidebar_plain(fb, inner, &mut y, &row(bp.description));
            }
        }
        if self.mode == Mode::EraseSpawns {
            Self::sidebar_plain(
                fb,
                inner,
                &mut y,
                &row("> Erase: brush removes all spawns in footprint"),
            );
            Self::sidebar_plain(
                fb,
                inner,
                &mut y,
                &row("  Shift+L rect clears spawns in box"),
            );
        }
        Self::sidebar_plain(fb, inner, &mut y, "");
        Self::sidebar_plain(fb, inner, &mut y, "-- Player start --");
        self.sidebar_player_spawn_row(fb, inner, &mut y);
        if self.mode == Mode::SetPlayerSpawn {
            Self::sidebar_plain(
                fb,
                inner,
                &mut y,
                &row("> Click map or Space: set   Backspace: clear"),
            );
            if let Some(ps) = self.level.player_spawn {
                Self::sidebar_plain(
                    fb,
                    inner,
                    &mut y,
                    &row(&format!("  Saved: ({},{})", ps.x, ps.y)),
                );
            } else {
                Self::sidebar_plain(
                    fb,
                    inner,
                    &mut y,
                    &row("  (none — game uses map center)"),
                );
            }
        }
    }

    fn sidebar_player_spawn_row(&mut self, fb: &mut FrameBuffer, inner: Rect, y: &mut u16) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let mark_fg = Color::rgb(120, 220, 255);
        let mut x = inner.x;
        let mut put = |ch: char, fg: Color| -> bool {
            if x >= right {
                return false;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            true
        };
        let sel = self.mode == Mode::SetPlayerSpawn;
        let _ = put(if sel { '>' } else { ' ' }, meta_fg);
        let _ = put('@', mark_fg);
        let _ = put(' ', meta_fg);
        for ch in "Player spawn".chars() {
            let _ = put(ch, meta_fg);
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits.push((
            SidebarHit::PlayerSpawn,
            Rect::new(inner.x, row_y, row_w, 1),
        ));
        *y = y.saturating_add(1);
    }

    fn sidebar_plain(fb: &mut FrameBuffer, inner: Rect, y: &mut u16, text: &str) {
        if *y >= inner.bottom() {
            return;
        }
        let fg = Color::rgb(210, 205, 195);
        let bg = Color::rgb(18, 16, 22);
        let mut x = inner.x;
        for ch in text.chars() {
            if x >= inner.right() {
                break;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
        }
        *y = y.saturating_add(1);
    }

    fn sidebar_tile_row(
        &mut self,
        fb: &mut FrameBuffer,
        inner: Rect,
        y: &mut u16,
        terrain_idx: usize,
        def: &TileDef,
        selected: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let mut x = inner.x;
        let mut put = |ch: char, fg: Color| -> bool {
            if x >= right {
                return false;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            true
        };
        let _ = put(if selected { '>' } else { ' ' }, meta_fg);
        for ch in format!("{:>3} ", def.id).chars() {
            let _ = put(ch, meta_fg);
        }
        let _ = put(def.glyph, def.fg);
        let _ = put(' ', meta_fg);
        for ch in (if def.solid() { "solid " } else { "open  " }).chars() {
            let _ = put(ch, meta_fg);
        }
        for ch in trunc_visual(&def.name, 28).chars() {
            if !put(ch, meta_fg) {
                break;
            }
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits.push((
            SidebarHit::Terrain(terrain_idx),
            Rect::new(inner.x, row_y, row_w, 1),
        ));
        *y = y.saturating_add(1);
    }

    fn sidebar_entity_row(
        &mut self,
        fb: &mut FrameBuffer,
        inner: Rect,
        y: &mut u16,
        entity_idx: usize,
        bp: &EntityBlueprint,
        selected: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
        let row_y = *y;
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let gcol = bp.default_fg.to_render_color();
        let mut x = inner.x;
        let mut put = |ch: char, fg: Color| -> bool {
            if x >= right {
                return false;
            }
            fb.set(
                x,
                *y,
                Cell {
                    ch,
                    fg,
                    bg,
                    style: Style::default(),
                },
            );
            x = x.saturating_add(1);
            true
        };
        let _ = put(if selected { '>' } else { ' ' }, meta_fg);
        for ch in format!("{} ", bp.kind).chars() {
            let _ = put(ch, meta_fg);
        }
        let _ = put(bp.default_glyph, gcol);
        let _ = put(' ', meta_fg);
        for ch in trunc_visual(bp.display_name, 22).chars() {
            if !put(ch, meta_fg) {
                break;
            }
        }
        let row_w = inner.w.min(right.saturating_sub(inner.x));
        self.sidebar_hits.push((
            SidebarHit::Entity(entity_idx),
            Rect::new(inner.x, row_y, row_w, 1),
        ));
        *y = y.saturating_add(1);
    }

    fn draw_dialog_layer(&self, fb: &mut FrameBuffer, d: &Dialog) {
        let dim = Cell {
            ch: ' ',
            fg: Color::rgb(100, 100, 110),
            bg: Color::rgb(8, 8, 12),
            style: Style {
                bold: false,
                dim: true,
                underline: false,
            },
        };
        fb.fill_rect(Rect::new(0, 0, fb.width, fb.height), dim);

        match d {
            Dialog::SavePath { field } => {
                let r = centered_rect(fb, 64, 7);
                draw_bordered_panel(fb, r, " Save as ");
                let iy = r.y + 2;
                draw_text_field(
                    fb,
                    r.x + 2,
                    iy,
                    r.w.saturating_sub(6),
                    "Path: ",
                    field,
                    true,
                );
                let hint = Rect::new(r.x + 2, iy + 2, r.w.saturating_sub(4), 2);
                draw_text_block(
                    fb,
                    hint,
                    &[String::from(
                        "Enter: save & close   Esc: cancel   (.ron added if no extension)",
                    )],
                );
            }
            Dialog::LevelTitle { field } => {
                let r = centered_rect(fb, 56, 7);
                draw_bordered_panel(fb, r, " Level title ");
                let iy = r.y + 2;
                draw_text_field(
                    fb,
                    r.x + 2,
                    iy,
                    r.w.saturating_sub(6),
                    "Name: ",
                    field,
                    true,
                );
                let hint = Rect::new(r.x + 2, iy + 2, r.w.saturating_sub(4), 1);
                draw_text_block(fb, hint, &[String::from("Enter: apply   Esc: cancel")]);
            }
            Dialog::Resize { w, h, focus } => {
                let r = centered_rect(fb, 44, 10);
                draw_bordered_panel(fb, r, " Map size ");
                let iy = r.y + 2;
                draw_text_field(fb, r.x + 2, iy, 8, "W: ", w, *focus == 0);
                draw_text_field(fb, r.x + 2, iy + 1, 8, "H: ", h, *focus == 1);
                let hint = Rect::new(r.x + 2, iy + 3, r.w.saturating_sub(4), 3);
                draw_text_block(
                    fb,
                    hint,
                    &[
                        "Tab: switch field".into(),
                        "Enter: apply (3..512)".into(),
                        "Esc: cancel".into(),
                    ],
                );
            }
            Dialog::NewTerrain {
                name,
                glyph,
                solid,
                color_idx,
                focus,
            } => {
                let r = centered_rect(fb, 58, 16);
                draw_bordered_panel(fb, r, " New terrain ");
                let iy = r.y + 2;
                draw_text_field(fb, r.x + 2, iy, 28, "Name: ", name, *focus == 0);
                draw_text_field(fb, r.x + 2, iy + 1, 4, "Glyph: ", glyph, *focus == 1);
                let solid_line = format!(
                    "{}Solid (blocks move & sight): {}",
                    if *focus == 2 { "> " } else { "  " },
                    if *solid { "yes" } else { "no " }
                );
                draw_text_block(
                    fb,
                    Rect::new(r.x + 2, iy + 2, r.w.saturating_sub(4), 1),
                    &[solid_line],
                );
                let fg = PRESET_COLORS[*color_idx % PRESET_COLORS.len()];
                let swatch = format!(
                    "{}Color [{}]: preview ",
                    if *focus == 3 { "> " } else { "  " },
                    color_idx
                );
                let mut x = r.x + 2;
                let sy = iy + 3;
                for ch in swatch.chars() {
                    if x >= r.right().saturating_sub(3) {
                        break;
                    }
                    fb.set(
                        x,
                        sy,
                        Cell {
                            ch,
                            fg: Color::rgb(210, 205, 195),
                            bg: Color::rgb(18, 16, 22),
                            style: Style::default(),
                        },
                    );
                    x = x.saturating_add(1);
                }
                fb.set(
                    x,
                    sy,
                    Cell {
                        ch: '#',
                        fg,
                        bg: Color::rgb(18, 16, 22),
                        style: Style {
                            bold: true,
                            dim: false,
                            underline: false,
                        },
                    },
                );
                let palette_y = iy + 4;
                let mut px = r.x + 2;
                for (i, c) in PRESET_COLORS.iter().enumerate() {
                    if px >= r.right().saturating_sub(1) {
                        break;
                    }
                    let mark = if i == *color_idx { '█' } else { '▒' };
                    fb.set(
                        px,
                        palette_y,
                        Cell {
                            ch: mark,
                            fg: *c,
                            bg: Color::rgb(10, 10, 14),
                            style: Style::default(),
                        },
                    );
                    px = px.saturating_add(1);
                }
                let hint = Rect::new(r.x + 2, iy + 6, r.w.saturating_sub(4), 6);
                draw_text_block(
                    fb,
                    hint,
                    &[
                        "Tab: cycle fields (name, glyph, solid, color)".into(),
                        "Space on Solid: toggle".into(),
                        "Arrows on Color: prev/next preset".into(),
                        "Enter: add   Esc: cancel".into(),
                        "Stored as RGB in the level file (truecolor).".into(),
                    ],
                );
            }
            Dialog::HotReloadUnsaved => {
                let r = centered_rect(fb, 72, 10);
                draw_bordered_panel(fb, r, " File changed on disk ");
                let body = Rect::new(r.x + 2, r.y + 2, r.w.saturating_sub(4), r.h.saturating_sub(4));
                draw_text_block(
                    fb,
                    body,
                    &[
                        "The .ron file was modified outside this editor.".into(),
                        "You have unsaved changes here.".into(),
                        String::new(),
                        "Y — reload from disk (discard local edits)".into(),
                        "N / Esc — keep editing; ignore this revision".into(),
                    ],
                );
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.status == "QUIT"
    }
}

fn trunc_visual(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    s.chars().take(max_cols).collect()
}

fn main() -> std::io::Result<()> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/levels/demo_level.ron"));
    let mut ed = Editor::load_or_new(&path);

    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        event::EnableMouseCapture,
    )?;

    let (mut tw, mut th) = crossterm::terminal::size()?;
    let mut fb = FrameBuffer::new(tw, th);
    let mut full = true;

    while !ed.should_quit() {
        ed.idle_viewport_tick();
        ed.poll_hot_reload();
        ed.compose(&mut fb);
        let use_full = full;
        full = false;
        let buf = if use_full {
            encode_frame_full(&fb)
        } else {
            encode_frame_delta(&fb).0
        };
        fb.commit_frame();
        stdout.queue(crossterm::cursor::MoveTo(0, 0))?;
        if use_full {
            stdout.queue(Clear(ClearType::All))?;
        }
        stdout.write_all(&buf)?;
        stdout.flush()?;

        if event::poll(Duration::from_millis(16))? {
            let mut batch = InputBatch::default();
            loop {
                match event::read()? {
                    Event::Key(k) => {
                        if let Some(ev) = map_key(k) {
                            batch.push(ev);
                        }
                    }
                    Event::Mouse(m) => {
                        if let Some(ev) = map_mouse(m) {
                            batch.push(ev);
                        }
                    }
                    Event::Resize(w, h) => {
                        tw = w;
                        th = h;
                        fb.resize(tw, th);
                        full = true;
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
            ed.step(&batch);
        }
    }

    execute!(
        stdout,
        event::DisableMouseCapture,
        Show,
        LeaveAlternateScreen
    )?;
    disable_raw_mode()?;
    Ok(())
}

fn map_mouse(m: event::MouseEvent) -> Option<InputEvent> {
    let cell = MouseCell {
        x: m.column,
        y: m.row,
    };
    let kind = match m.kind {
        CMouseKind::Down(b) => MouseEventKind::Down(match b {
            CMouseButton::Left => MouseButton::Left,
            CMouseButton::Right => MouseButton::Right,
            CMouseButton::Middle => MouseButton::Middle,
        }),
        CMouseKind::Up(b) => MouseEventKind::Up(match b {
            CMouseButton::Left => MouseButton::Left,
            CMouseButton::Right => MouseButton::Right,
            CMouseButton::Middle => MouseButton::Middle,
        }),
        CMouseKind::Drag(b) => MouseEventKind::Drag(match b {
            CMouseButton::Left => MouseButton::Left,
            CMouseButton::Right => MouseButton::Right,
            CMouseButton::Middle => MouseButton::Middle,
        }),
        CMouseKind::ScrollUp => MouseEventKind::ScrollUp,
        CMouseKind::ScrollDown => MouseEventKind::ScrollDown,
        CMouseKind::ScrollLeft | CMouseKind::ScrollRight => return None,
        CMouseKind::Moved => MouseEventKind::Moved,
    };
    Some(InputEvent::Mouse {
        kind,
        cell,
        column: m.column,
        shift: m.modifiers.contains(KeyModifiers::SHIFT),
        ctrl: m.modifiers.contains(KeyModifiers::CONTROL),
        alt: m.modifiers.contains(KeyModifiers::ALT),
    })
}

fn map_key(k: KeyEvent) -> Option<InputEvent> {
    let chord = KeyChord {
        key: match k.code {
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Enter => Key::Enter,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Tab => Key::Tab,
            KeyCode::Esc => Key::Esc,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::Delete => Key::Delete,
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::F(n) => Key::F(n.min(12)),
            _ => return None,
        },
        ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
        alt: k.modifiers.contains(KeyModifiers::ALT),
        shift: k.modifiers.contains(KeyModifiers::SHIFT),
    };
    Some(InputEvent::Key(chord))
}
