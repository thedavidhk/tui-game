# TUI fantasy RPG (first iteration)

Cross-platform terminal game and level editor in Rust: custom frame loop with **crossterm** in the binaries, simulation and rendering buffers in **`tui_game_core`** (no direct terminal I/O in core except `std::fs` for save/load helpers). Levels and saves use **RON** for human-readable debugging.

## Requirements

- **Rust** (2021 edition; stable recommended)
- A terminal with **truecolor** and **mouse** support for the full experience (the game degrades reasonably on smaller or simpler terminals)

## Quick start

From the repository root:

```bash
# Run the game (built-in demo room; start with the main menu)
cargo run -p tui_game

# Run the game with a level file produced by the editor
cargo run -p tui_game -- path/to/level.ron

# Run the level editor (optional initial path; use F2 in-app to change save path)
cargo run -p tui_level_editor
cargo run -p tui_level_editor -- my_level.ron
```

Saves written from the game use **`save.ron`** in the current working directory (see in-game keys below).

## Tests

```bash
# All workspace crates
cargo test --workspace

# Core library only (unit tests for FoW, saves, levels, content validation)
cargo test -p tui_game_core
```

Tests are **headless** (no TTY required): FoW, RON round-trips for `LevelFile` and `SaveGameV1`, and content pack validation.

## Workspace layout

| Crate | Role |
|--------|------|
| **`tui_game_core`** | Library: map, entities, FoW, game rules, `FrameBuffer` + ANSI encoding, input model, UI composition into buffers, save types, level file types, static content |
| **`tui_game`** | Binary: raw mode, alternate screen, input polling, resize, flush stdout |
| **`tui_level_editor`** | Binary: edit `LevelFile` (tiles + spawns), same buffer + ANSI approach as the game |

## `tui_game_core` module map

- **`render/`** — `FrameBuffer`, cells (glyph + truecolor + style), delta/full ANSI output, frame timing samples
- **`world/`** — `MapGrid`, `TileDef` / `TileId`, field-of-view (`compute_visible`, explored mask)
- **`entity.rs`** — `EntityId`, `EntityArena` (SoA-style stores; not a full ECS framework)
- **`game.rs`** — `Game`, mode stack (`MainMenu`, `Exploration`, `Dialogue`, `Combat`), composition into the buffer
- **`input.rs`** — `InputEvent` / `InputBatch` (normalized; binaries map crossterm events here)
- **`rect.rs`** — cell-space rectangles and hit testing
- **`ui/`** — panels, log, menu, dialogue, text fields, color presets (write only into a `FrameBuffer`; no crossterm)
- **`combat.rs`** — turn-order stub (move / pass / flee)
- **`save.rs`** — `SaveGameV1`, `WorldSnapshot`, schema version
- **`level.rs`** — `LevelFile`, `EntitySpawn`, RON (de)serialization
- **`content.rs`** — demo `ContentPack`, static `DialogueTree`, `EntityBlueprint` registry, `validate()` / `validate_level()`

## Game controls (reference)

These are implemented in **`Game`** (`game.rs`); the binary forwards keyboard and mouse into `InputBatch`.

- **Main menu**: arrow keys or `j` / `k`, **Enter** to choose; **mouse** click on a row (hit rects from last frame)
- **Exploration**: **WASD** or arrows to move; **E** or **Enter** to try to talk to an adjacent NPC; **C** to try combat stub (stand **south** of an entity); **F1** debug overlay; **F5** / **F9** save / load `save.ron`
- **Dialogue**: **j** / **k** or arrows for choices, **Enter** / **Space** to confirm, **1–9** to jump to a choice, **Esc** to close; **mouse** click on a choice row where supported
- **Combat (stub)**: **WASD** / arrows to move current actor, **Tab** or **Space** to end turn, **F** to flee, **Esc** to exit
- **Quit from binary**: **Ctrl+Q** (handled in `tui_game`); from menu choose **Quit**

## Level editor controls

- **WASD** or arrows: move cursor
- **`[`** / **`]`** or **`k`** / **`j`**: cycle brush — **terrain** in paint mode, **entity blueprint** in spawn mode (sidebar lists names, ids, solid/open, and glyph colors)
- **Space**: paint tile (paint mode) or add a spawn (spawn mode)
- **m**: toggle paint vs place spawns
- **F2**: save as — type a path (`.ron` is added if you omit an extension), **Enter** to write and close
- **F3**: edit the level’s display name (`LevelFile.name`)
- **F4**: resize map (width/height, **Tab** between fields, **Enter** to apply; valid range 3–256)
- **F5**: define a new terrain — name, single glyph, **solid** (blocks movement and line-of-sight), and a **preset RGB** swatch (stored as truecolor `fg` on `TileDef`)
- **Ctrl+S**: save to the current file path
- **Ctrl+Q**: quit

Spawns store `kind` matching an **`EntityBlueprint`** in `ContentPack` (see `content.rs` / `DEMO_ENTITY_BLUEPRINTS`). Add blueprints in Rust, then pick them in the editor; **`Ctrl+S`** refuses to save if the level references unknown tile ids or unknown spawn kinds.

### Terrain color: ANSI vs RGB

The game and editor already target **24-bit truecolor** (`38;2` / `48;2` in `render/ansi.rs`). Tile appearance uses **`TileDef.fg` as RGB** in the level file. The editor’s F5 picker is a fixed **preset palette** for convenience; there is no separate “ANSI color id” in the format. If you need arbitrary colors later, extend the editor with explicit R/G/B fields or a hex field while keeping the on-disk representation as three `u8`s.

## Design notes for contributors

**Principles, linting, interface style, and compatibility expectations** (including guidance for AI agents) live in **`docs/DESIGN.md`**. Read that file before substantial changes.

1. **Core vs binaries** — Keep terminal setup, event polling, and `stdout` writes in **`tui_game`** / **`tui_level_editor`**. Add gameplay and rendering *data* in **`tui_game_core`**.
2. **Rendering** — Prefer updating the `FrameBuffer` then **delta-encoding** against the previous committed frame; force a full redraw on resize or when wiring a “full redraw” flag.
3. **Saves** — Bump **`SAVE_SCHEMA_VERSION`** / **`schema_version`** when serialized shapes change. Migration logic is optional until the project needs to preserve real user data (see **`docs/DESIGN.md`**).
4. **Content** — Dialogue trees are static Rust data (`ContentPack::demo()`); they are intentionally **not** serde-friendly as `&'static` graphs. Persist quest/world state via **`SaveGameV1`**, not by serializing the whole content pack.
5. **Dependencies** — Prefer small, well-scoped crates (e.g. `serde` + `ron`, `unicode-width`). Avoid pulling in a full TUI framework so the main loop and layout stay explicit.

## License

See `Cargo.toml` (`MIT OR Apache-2.0`).
