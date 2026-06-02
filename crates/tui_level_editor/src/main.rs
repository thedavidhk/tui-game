//! Level editor binary: terminal bridge and main loop.
//!
//! Editor logic lives in [`editor`].

mod editor;

use std::env;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

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
use editor::Editor;
use tui_game_core::input::InputBatch;
use tui_game_core::render::{encode_frame_delta, encode_frame_full, FrameBuffer};
use tui_terminal::{map_key, map_mouse};

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
