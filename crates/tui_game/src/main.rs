//! Game binary: crossterm terminal setup and main loop.
//!
//! Optional: `tui_game path/to/level.ron` loads a level from disk (default layout: `assets/levels/demo_level.ron`).

use std::env;
use std::io::{stdout, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    QueueableCommand,
};
mod keymap;

use tui_game_core::game::GameMode;
use tui_game_core::input::{InputBatch, InputEvent, Key, KeyChord};
use tui_game_core::rect::Rect;
use tui_game_core::render::{
    encode_frame_delta, encode_frame_full, FrameBuffer, FrameSample, FrameStatsRing,
};
use tui_game_core::ui::layout::GameShellLayout;
use tui_game_core::Game;
use tui_terminal::{map_key, map_mouse};

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
        let pack_base = Path::new(&path).parent();
        let mut g = Game::from_level_file(&level, tw, th, keymap::game_key_map(), pack_base)
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
