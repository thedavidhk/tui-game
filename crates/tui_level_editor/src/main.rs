//! Minimal level editor: shared `LevelFile` format, tile paint, entity spawns.

use std::env;
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    QueueableCommand,
};
use tui_game_core::input::{InputBatch, InputEvent, Key, KeyChord};
use tui_game_core::level::{level_from_ron, level_to_ron, EntitySpawn, LevelFile};
use tui_game_core::rect::Rect;
use tui_game_core::render::{encode_frame_delta, encode_frame_full, Cell, Color, FrameBuffer, Style};
use tui_game_core::world::{TileId, TileTable};

struct Editor {
    path: PathBuf,
    level: LevelFile,
    cursor_x: i32,
    cursor_y: i32,
    current_tile: TileId,
    mode: Mode,
    status: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    PaintTiles,
    PlaceSpawns,
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
                    Err(e) => (Self::default_level(), format!("Parse error: {e}; new level")),
                },
                Err(e) => (Self::default_level(), format!("Read error: {e}; new level")),
            }
        } else {
            (Self::default_level(), format!("New level ({} missing)", path.display()))
        };
        Self {
            path: path.clone(),
            level,
            cursor_x: 4,
            cursor_y: 4,
            current_tile: 0,
            mode: Mode::PaintTiles,
            status,
        }
    }

    fn save(&mut self) -> Result<(), String> {
        let s = level_to_ron(&self.level).map_err(|e| e.to_string())?;
        fs::write(&self.path, s).map_err(|e| e.to_string())?;
        self.status = format!("Saved {}", self.path.display());
        Ok(())
    }

    fn step(&mut self, input: &InputBatch) {
        for ev in &input.events {
            match ev {
                InputEvent::Key(KeyChord {
                    key: Key::Char('s'),
                    ctrl: true,
                    ..
                }) => {
                    if let Err(e) = self.save() {
                        self.status = e;
                    }
                }
                InputEvent::Key(KeyChord {
                    key: Key::Char('m'),
                    ..
                }) => {
                    self.mode = if self.mode == Mode::PaintTiles {
                        Mode::PlaceSpawns
                    } else {
                        Mode::PaintTiles
                    };
                    self.status = format!("Mode: {:?}", self.mode);
                }
                InputEvent::Key(KeyChord {
                    key: Key::Char(' '),
                    ..
                }) => {
                    if self.mode == Mode::PaintTiles {
                        let i = self.cursor_y as usize * self.level.width as usize + self.cursor_x as usize;
                        if i < self.level.tiles.len() {
                            self.level.tiles[i] = self.current_tile;
                        }
                    } else {
                        self.level.spawns.push(EntitySpawn {
                            kind: "guide".into(),
                            x: self.cursor_x,
                            y: self.cursor_y,
                            glyph: 'g',
                            name: "Guide".into(),
                        });
                        self.status = format!("Spawn at ({}, {}).", self.cursor_x, self.cursor_y);
                    }
                }
                InputEvent::Key(KeyChord {
                    key: Key::Char('0'), ..
                }) => {
                    self.current_tile = 0;
                }
                InputEvent::Key(KeyChord {
                    key: Key::Char('1'), ..
                }) => {
                    self.current_tile = 1;
                }
                InputEvent::Key(KeyChord {
                    key: Key::Up | Key::Char('w'),
                    ..
                }) => {
                    self.cursor_y = (self.cursor_y - 1).max(0);
                }
                InputEvent::Key(KeyChord {
                    key: Key::Down | Key::Char('s'),
                    ..
                }) => {
                    self.cursor_y = (self.cursor_y + 1).min(self.level.height as i32 - 1);
                }
                InputEvent::Key(KeyChord {
                    key: Key::Left | Key::Char('a'),
                    ..
                }) => {
                    self.cursor_x = (self.cursor_x - 1).max(0);
                }
                InputEvent::Key(KeyChord {
                    key: Key::Right | Key::Char('d'),
                    ..
                }) => {
                    self.cursor_x = (self.cursor_x + 1).min(self.level.width as i32 - 1);
                }
                InputEvent::Key(KeyChord {
                    key: Key::Char('q'),
                    ctrl: true,
                    ..
                }) => {
                    self.status = "QUIT".into();
                }
                _ => {}
            }
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
                let def = self
                    .level
                    .tile_defs
                    .iter()
                    .find(|d| d.id == tid);
                let ch = def.map(|d| d.glyph).unwrap_or('?');
                let mut c = Cell {
                    ch,
                    fg: Color::rgb(200, 190, 170),
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
        let lines = [
            format!("{}", self.status),
            "WASD: move cursor".into(),
            "0/1: tile id".into(),
            "Space: paint / add spawn".into(),
            "m: toggle mode".into(),
            "Ctrl+S: save".into(),
            "Ctrl+Q: quit".into(),
            format!("Mode: {:?}", self.mode),
            format!("Tile: {}", self.current_tile),
        ];
        let mut y = help.y;
        for line in lines {
            let mut x = help.x;
            for ch in line.chars() {
                if x >= help.right() {
                    break;
                }
                fb.set(
                    x,
                    y,
                    Cell {
                        ch,
                        fg,
                        bg: Color::rgb(20, 20, 28),
                        style: Style::default(),
                    },
                );
                x = x.saturating_add(1);
            }
            y = y.saturating_add(1);
        }
    }

    fn should_quit(&self) -> bool {
        self.status == "QUIT"
    }
}

fn main() -> std::io::Result<()> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("demo_level.ron"));
    let mut ed = Editor::load_or_new(&path);

    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;

    let (mut tw, mut th) = crossterm::terminal::size()?;
    let mut fb = FrameBuffer::new(tw, th);
    let mut full = true;

    while !ed.should_quit() {
        if tw < ed.level.width + 22 || th < ed.level.height + 2 {
            // terminal too small — still run
        }
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
                        fb.resize(w, h);
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
