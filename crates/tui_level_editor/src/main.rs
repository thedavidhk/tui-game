//! Level editor: `LevelFile` paint/spawns, custom tile defs, resize, named save.

use std::env;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    QueueableCommand,
};
use tui_game_core::content::ContentPack;
use tui_game_core::game_content;
use tui_game_core::input::{InputBatch, InputEvent, Key, KeyChord};
use tui_game_core::level::{level_from_ron, level_to_ron, EntitySpawn, LevelFile};
use tui_game_core::rect::Rect;
use tui_game_core::render::{
    encode_frame_delta, encode_frame_full, Cell, Color, FrameBuffer, Style,
};
use tui_game_core::ui::{
    centered_rect, draw_bordered_panel, draw_text_block, draw_text_field, TextField,
    TextFieldOutput, TextFilter, PRESET_COLORS,
};
use tui_game_core::world::{TileDef, TileId, TileTable};
use tui_game_core::EntityBlueprint;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    PaintTiles,
    PlaceSpawns,
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
}

impl Editor {
    fn default_level() -> LevelFile {
        let w = 24u16;
        let h = 16u16;
        let n = (w as usize) * (h as usize);
        let mut tiles = vec![0u16; n];
        for x in 0..w {
            tiles[x as usize] = 1;
            tiles[(h as usize - 1) * w as usize + x as usize] = 1;
        }
        for y in 0..h {
            tiles[y as usize * w as usize] = 1;
            tiles[y as usize * w as usize + (w as usize - 1)] = 1;
        }
        LevelFile {
            schema_version: LevelFile::SCHEMA,
            name: "untitled".into(),
            width: w,
            height: h,
            tiles,
            tile_defs: TileTable::default_pack().defs,
            spawns: vec![EntitySpawn {
                kind: "guide".into(),
                x: 10,
                y: 8,
                glyph: 'g',
                name: "Guide".into(),
            }],
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
        Self {
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
        }
    }

    fn save(&mut self) -> Result<(), String> {
        self.content
            .validate_level(&self.level)
            .map_err(|e| e.to_string())?;
        let s = level_to_ron(&self.level).map_err(|e| e.to_string())?;
        fs::write(&self.path, s).map_err(|e| e.to_string())?;
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
        self.level
            .spawns
            .retain(|s| s.x >= 0 && s.y >= 0 && (s.x as u16) < nw && (s.y as u16) < nh);
        self.cursor_x = self.cursor_x.clamp(0, nw as i32 - 1);
        self.cursor_y = self.cursor_y.clamp(0, nh as i32 - 1);
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
            let InputEvent::Key(chord) = ev else {
                continue;
            };
            if self.handle_dialog(chord) {
                continue;
            }
            self.step_main(chord);
        }
    }

    fn handle_dialog(&mut self, chord: &KeyChord) -> bool {
        if chord.ctrl && matches!(chord.key, Key::Char('q')) {
            self.dialog = None;
            self.status = "QUIT".into();
            return true;
        }
        if matches!(chord.key, Key::Enter) && !chord.ctrl {
            if matches!(&self.dialog, Some(Dialog::NewTerrain { .. })) {
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
                    };
                    self.level.tile_defs.push(def);
                    self.current_tile = id;
                    self.status = format!("Added tile id {id} ({n}).");
                    return true;
                }
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
                        if (3..=256).contains(&n) {
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
                            self.status = "Width/height must be integers from 3 to 256.".into();
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
        }
    }

    fn step_main(&mut self, chord: &KeyChord) {
        match chord {
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
                self.mode = if self.mode == Mode::PaintTiles {
                    Mode::PlaceSpawns
                } else {
                    Mode::PaintTiles
                };
                self.status = format!("Mode: {:?}", self.mode);
            }
            KeyChord {
                key: Key::Char(' '),
                ctrl: false,
                ..
            } => {
                if self.mode == Mode::PaintTiles {
                    let i =
                        self.cursor_y as usize * self.level.width as usize + self.cursor_x as usize;
                    if i < self.level.tiles.len() {
                        self.level.tiles[i] = self.current_tile;
                    }
                } else {
                    let Some(bp) = self.current_spawn_blueprint() else {
                        self.status = "No entity blueprints in content pack.".into();
                        return;
                    };
                    self.level.spawns.push(EntitySpawn {
                        kind: bp.kind.to_string(),
                        x: self.cursor_x,
                        y: self.cursor_y,
                        glyph: bp.default_glyph,
                        name: bp.default_label.to_string(),
                    });
                    self.status = format!(
                        "Spawn {} at ({}, {}).",
                        bp.kind, self.cursor_x, self.cursor_y
                    );
                }
            }
            KeyChord {
                key: Key::Char('[') | Key::Char('k'),
                ctrl: false,
                ..
            } => match self.mode {
                Mode::PaintTiles => self.cycle_tile_palette(-1),
                Mode::PlaceSpawns => self.cycle_spawn_blueprint(-1),
            },
            KeyChord {
                key: Key::Char(']') | Key::Char('j'),
                ctrl: false,
                ..
            } => match self.mode {
                Mode::PaintTiles => self.cycle_tile_palette(1),
                Mode::PlaceSpawns => self.cycle_spawn_blueprint(1),
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
                key: Key::Up | Key::Char('w'),
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y - 1).max(0);
            }
            KeyChord {
                key: Key::Down | Key::Char('s'),
                ctrl: false,
                ..
            } => {
                self.cursor_y = (self.cursor_y + 1).min(self.level.height as i32 - 1);
            }
            KeyChord {
                key: Key::Left | Key::Char('a'),
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x - 1).max(0);
            }
            KeyChord {
                key: Key::Right | Key::Char('d'),
                ctrl: false,
                ..
            } => {
                self.cursor_x = (self.cursor_x + 1).min(self.level.width as i32 - 1);
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

    fn compose(&self, fb: &mut FrameBuffer) {
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
        let ox = 0u16;
        let oy = 0u16;
        for ty in 0..self.level.height {
            for tx in 0..self.level.width {
                let tid = self.level.tiles[ty as usize * self.level.width as usize + tx as usize];
                let def = self.level.tile_defs.iter().find(|d| d.id == tid);
                let ch = def.map(|d| d.glyph).unwrap_or('?');
                let tile_fg = def.map(|d| d.fg).unwrap_or(Color::rgb(200, 190, 170));
                let mut c = Cell {
                    ch,
                    fg: tile_fg,
                    bg,
                    style: Style::default(),
                };
                if tx as i32 == self.cursor_x && ty as i32 == self.cursor_y {
                    c.style.bold = true;
                    c.fg = Color::rgb(255, 255, 120);
                }
                fb.set(ox + tx, oy + ty, c);
            }
        }
        for s in &self.level.spawns {
            if s.x >= 0
                && s.y >= 0
                && (s.x as u16) < self.level.width
                && (s.y as u16) < self.level.height
            {
                let c = Cell {
                    ch: s.glyph,
                    fg: Color::rgb(255, 160, 80),
                    bg,
                    style: Style {
                        bold: true,
                        dim: false,
                        underline: false,
                    },
                };
                fb.set(ox + s.x as u16, oy + s.y as u16, c);
            }
        }
        let help = Rect::new(
            self.level.width + 1,
            0,
            fb.width.saturating_sub(self.level.width + 2),
            fb.height,
        );
        self.compose_sidebar(fb, help);

        if let Some(ref d) = self.dialog {
            self.draw_dialog_layer(fb, d);
        }
    }

    fn compose_sidebar(&self, fb: &mut FrameBuffer, help: Rect) {
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
        Self::sidebar_plain(fb, inner, &mut y, "WASD move  [/]jk brush  m mode");
        Self::sidebar_plain(fb, inner, &mut y, "Space paint/spawn  F2-F5  C-S save");
        Self::sidebar_plain(fb, inner, &mut y, &row(&format!("Mode: {:?}", self.mode)));
        Self::sidebar_plain(fb, inner, &mut y, "");

        Self::sidebar_plain(fb, inner, &mut y, "-- Terrain --");
        for def in &self.level.tile_defs {
            let sel = def.id == self.current_tile && self.mode == Mode::PaintTiles;
            Self::sidebar_tile_row(fb, inner, &mut y, def, sel);
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
                Self::sidebar_entity_row(fb, inner, &mut y, bp, sel);
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
        fb: &mut FrameBuffer,
        inner: Rect,
        y: &mut u16,
        def: &TileDef,
        selected: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
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
        *y = y.saturating_add(1);
    }

    fn sidebar_entity_row(
        fb: &mut FrameBuffer,
        inner: Rect,
        y: &mut u16,
        bp: &EntityBlueprint,
        selected: bool,
    ) {
        if *y >= inner.bottom() {
            return;
        }
        let right = inner.right();
        let bg = Color::rgb(18, 16, 22);
        let meta_fg = Color::rgb(175, 170, 160);
        let gcol = Color::rgb(255, 160, 80);
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
                        "Enter: apply (3..256)".into(),
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
    execute!(stdout, EnterAlternateScreen, Hide)?;

    let (mut tw, mut th) = crossterm::terminal::size()?;
    let mut fb = FrameBuffer::new(tw, th);
    let mut full = true;

    while !ed.should_quit() {
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

        if event::poll(Duration::from_millis(50))? {
            let mut batch = InputBatch::default();
            loop {
                match event::read()? {
                    Event::Key(k) => {
                        if let Some(ev) = map_key(k) {
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

    execute!(stdout, Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
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
