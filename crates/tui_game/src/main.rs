//! Game binary: crossterm terminal setup and main loop.
//!
//! Optional: `tui_game path/to/level.ron` loads a level from disk (default layout: `assets/levels/demo_level.ron`).

use std::env;
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

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
mod keymap;

use tui_game_core::game::GameMode;
use tui_game_core::input::{
    InputBatch, InputEvent, Key, KeyChord, MouseButton, MouseCell, MouseEventKind,
};
use tui_game_core::rect::Rect;
use tui_game_core::ui::layout::GameShellLayout;
use tui_game_core::render::{
    encode_frame_delta, encode_frame_full, FrameBuffer, FrameSample, FrameStatsRing,
};
use tui_game_core::Game;

fn main() -> std::io::Result<()> {
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        crossterm::event::EnableMouseCapture,
    )?;

    let (mut tw, mut th) = crossterm::terminal::size()?;
    let mut game = if let Some(path) = env::args().nth(1) {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| std::io::Error::other(format!("read {path}: {e}")))?;
        let level = tui_game_core::level::level_from_ron(&raw).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("ron: {e}"))
        })?;
        let mut g = Game::from_level_file(&level, tw, th, keymap::game_key_map())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        g.set_restart_level_ron_path(Some(path));
        g
    } else {
        Game::new_bootstrapped_with_keymap(tw, th, keymap::game_key_map())
    };
    let mut fb = FrameBuffer::new(tw, th);
    let mut stats = FrameStatsRing::new(120);
    let mut full_redraw_next = true;
    let mut first_frame = true;
    while !game.quit_requested {
        game.viewport_w = tw;
        game.viewport_h = th;

        // Exploration and combat run the game `step` at ~60 Hz so input, auto-walk, and
        // viewport edge-scroll stay responsive. Menus and modal UIs use a slower poll.
        let poll_ms = match game.modes.current() {
            Some(GameMode::Exploration) | Some(GameMode::Combat(_)) => 16,
            _ => 250,
        };
        let mut batch = InputBatch::default();
        let poll_wait = if first_frame {
            first_frame = false;
            Duration::ZERO
        } else {
            Duration::from_millis(poll_ms)
        };
        if event::poll(poll_wait)? {
            loop {
                match event::read()? {
                    Event::Key(k) => {
                        if let Some(ev) = map_key(k) {
                            if matches!(
                                ev,
                                InputEvent::Key(KeyChord {
                                    key: Key::Char('q'),
                                    ctrl: true,
                                    ..
                                })
                            ) {
                                game.quit_requested = true;
                                break;
                            }
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
                        fb.resize(w, h);
                        full_redraw_next = true;
                        batch.push(InputEvent::Resize {
                            width: w,
                            height: h,
                        });
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        game.step(&batch);

        let (world_r, hud_r, log_r) = layout(tw, th);
        game.compose(&mut fb, world_r, hud_r, log_r);

        let use_full = full_redraw_next;
        full_redraw_next = false;

        let encode_start = Instant::now();
        let (buf, dirty) = if use_full {
            (encode_frame_full(&fb), fb.width as u32 * fb.height as u32)
        } else {
            encode_frame_delta(&fb)
        };
        let encode_nanos = encode_start.elapsed().as_nanos() as u64;
        fb.commit_frame();

        let sample = FrameSample {
            encode_nanos,
            cells_dirty: dirty,
            bytes_written: buf.len() as u32,
            terminal_w: tw,
            terminal_h: th,
        };
        stats.push(sample);
        game.last_perf = stats.last();

        stdout.queue(crossterm::cursor::MoveTo(0, 0))?;
        if use_full {
            stdout.queue(Clear(ClearType::All))?;
        }
        stdout.write_all(&buf)?;
        stdout.flush()?;
    }

    execute!(
        stdout,
        Show,
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    disable_raw_mode()?;
    Ok(())
}

fn layout(w: u16, h: u16) -> (Rect, Rect, Rect) {
    GameShellLayout::root_panels(w, h)
}

fn map_key(k: KeyEvent) -> Option<InputEvent> {
    let mut chord = KeyChord {
        key: Key::Char(' '),
        ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
        alt: k.modifiers.contains(KeyModifiers::ALT),
        shift: k.modifiers.contains(KeyModifiers::SHIFT),
    };
    chord.key = match k.code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Tab => Key::Tab,
        KeyCode::Esc => Key::Esc,
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::F(n) => Key::F(n.min(12)),
        _ => return None,
    };
    Some(InputEvent::Key(chord))
}

fn map_mouse(m: crossterm::event::MouseEvent) -> Option<InputEvent> {
    use crossterm::event::KeyModifiers;
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
