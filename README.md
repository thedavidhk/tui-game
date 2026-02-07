# TUI fantasy RPG (first iteration)

Cross-platform terminal game and level editor in Rust: custom frame loop with **crossterm** in the binaries, simulation and rendering buffers in **`tui_game_core`** (no direct terminal I/O in core except `std::fs` for save/load helpers). Levels and saves use **RON** for human-readable debugging.

## Requirements

- **Rust** (2021 edition; stable recommended)
- A terminal with **truecolor** and **mouse** support for the full experience (the game degrades reasonably on smaller or simpler terminals)

## Quick start

From the repository root:

```bash
# Run the game (embedded `assets/levels/demo_level.ron`; start with the main menu)
cargo run -p tui_game

# Run the game with a level file produced by the editor (any path)
cargo run -p tui_game -- assets/levels/demo_level.ron

# Run the level editor (optional initial path; default opens `assets/levels/demo_level.ron` from repo root)
cargo run -p tui_level_editor
cargo run -p tui_level_editor -- assets/levels/other_level.ron
```

Saves written from the game use **`save.ron`** in the current working directory (see in-game keys below).

## Tests

```bash
# All workspace crates
cargo test --workspace

# Core library only (unit tests for FoW, saves, levels, content validation)
cargo test -p tui_game_core
```

Tests are **headless** (no TTY required): FoW, RON round-trips for `LevelFile` and `SaveGameV1`, content pack validation (including item ids on dialogue and blueprints), and inventory helpers.

## Workspace layout

| Crate | Role |
|--------|------|
| **`tui_game_core`** | Library: map, entities, FoW, game rules, `FrameBuffer` + ANSI encoding, input model, UI composition into buffers, save types, level file types, static content |
| **`tui_game`** | Binary: raw mode, alternate screen, input polling, resize, flush stdout |
| **`tui_level_editor`** | Binary: edit `LevelFile` (tiles + spawns), same buffer + ANSI approach as the game |

## `tui_game_core` module map

- **`render/`** — `FrameBuffer`, cells (glyph + truecolor + style), delta/full ANSI output, frame timing samples
- **`world/`** — `MapGrid`, `TileDef` / `TileId`, field-of-view (`compute_visible`, explored mask)
- **`entity.rs`** — `EntityId`, `EntityArena` (SoA-style stores; not a full ECS framework), optional ground `item` stacks and `is_container`
- **`item.rs`** — `ItemDef`, `ItemCategory`, `ItemCatalog`, `Inventory`, equipment slot stub
- **`game/`** — `Game`, mode stack, `modes` dispatch, composition into the buffer
- **`narrative.rs`** — `NarrativeState`, dialogue `Condition` / `Effect` application, `quest_stages`
- **`input.rs`** — `InputEvent` / `InputBatch` (normalized; binaries map crossterm events here)
- **`rect.rs`** — cell-space rectangles and hit testing
- **`ui/`** — panels, log, menu, dialogue, text fields, color presets (write only into a `FrameBuffer`; no crossterm)
- **`combat.rs`** — turn-order stub (move / pass / flee)
- **`save.rs`** — `SaveGameV1`, `WorldSnapshot`, schema version
- **`level.rs`** — `LevelFile`, `EntitySpawn`, RON (de)serialization
- **`content.rs`** — `ContentPack`, dialogue/blueprint types, `validate()` / `validate_level()` (no game-specific tables)
- **`game_content.rs`** — this game’s static dialogues, entity blueprints, and `game_content::content_pack()` (used by the game and level editor)

## Game controls (reference)

These are implemented in **`Game`** (`game/mod.rs`); the binary forwards keyboard and mouse into `InputBatch`.

- **Main menu**: arrow keys or `j` / `k`, **Enter** to choose; **mouse** click on a row (hit rects from last frame)
- **Exploration**: **WASD** or arrows to move; **E** or **Enter** to talk to an adjacent **NPC** or open **transfer** next to a **container** (chest); **I** opens **player inventory** (list + detail; **u** consume stub, **e** equip stub, **Esc** close); **C** combat stub (stand **south** of an entity); **F1** debug; **F5** / **F9** save / load `save.ron`
- **Player inventory** (pushed over exploration): **j** / **k** or arrows move selection; **u** use consumable (logs “no effect yet”); **e** equip ring-class items into a persisted slot (previous ring returns to inventory); **Esc** closes
- **Item transfer** (player ↔ adjacent container): **Tab** or **h** / **l** switch side; **j** / **k** move row on focused side; **Enter** moves the **entire** focused stack to the other side; **Esc** closes
- **Dialogue**: **j** / **k** or arrows for choices, **Enter** / **Space** to confirm, **1–9** to jump to a choice, **Esc** to close; **mouse** click on a choice row where supported. Choices may **require** an item (blocked with a log line if missing), **give** / **take** inventory, and set **quest phase** from static data (`game_content.rs`)
- **Combat (stub)**: **WASD** / arrows to move current actor, **Tab** or **Space** to end turn, **F** to flee, **Esc** to exit
- **Quit from binary**: **Ctrl+Q** (handled in `tui_game`); from menu choose **Quit**

## Level editor controls

- **WASD** or arrows: move cursor
- **Tab** or **m**: cycle **paint tiles** → **place spawns** → **erase spawns** → …
- **`[`** / **`]`** or **`k`** / **`j`**: cycle brush — **terrain** in paint mode, **entity blueprint** in spawn mode (sidebar lists names, ids, solid/open, and glyph colors)
- **Space**: paint with brush (paint mode), place one spawn (spawn mode), or **erase spawns in brush** (erase mode)
- **`+`** / **`-`** (or **`=`** / **`_`**): brush **radius** (square brush, paint mode); **mouse wheel** over the map changes radius too
- **Left drag** on the map: paint, stamp spawns, or **erase spawns in brush** by mode; **Shift + left drag** then release: fill a **tile** rectangle (paint) or remove **all spawns** in a rectangle (erase)
- **Hover** on the map: the brush footprint (or spawn cell, or shift-rectangle preview) is shown with a **slightly lighter background** before you click
- **Left click** a **terrain** or **entity** row in the sidebar: select that brush and switch to the matching mode
- **Esc** (no dialog): cancel a shift-rectangle in progress
- **F2**: save as — type a path (`.ron` is added if you omit an extension), **Enter** to write and close
- **F3**: edit the level’s display name (`LevelFile.name`)
- **F4**: resize map (width/height, **Tab** between fields, **Enter** to apply; valid range 3–256)
- **F5**: define a new terrain — name, single glyph, **solid** (blocks movement and line-of-sight), and a **preset RGB** swatch (stored as truecolor `fg` on `TileDef`)
- **Ctrl+S**: save to the current file path
- **Ctrl+Q**: quit

Spawns store `kind` matching an **`EntityBlueprint`** from **`game_content`** (see `game_content.rs`). Add blueprints there, then pick them in the editor; **`Ctrl+S`** refuses to save if the level references unknown tile ids or unknown spawn kinds. Blueprint fields include optional **`world_item`** (spawns a pickable ground entity for that `ItemDef.id`) and **`is_container`** (adjacent **E** opens transfer instead of dialogue). Demo level **[`assets/levels/demo_level.ron`](assets/levels/demo_level.ron)** includes villagers, quest pickups, and a **chest** (open in the editor from the repo root so the default path resolves).

Longer-term **quest / inventory / UI** refactors (phased roadmap, acceptance criteria, and completion log) live in **[`docs/REFACTOR_QUEST_INVENTORY_UI.md`](docs/REFACTOR_QUEST_INVENTORY_UI.md)**.

### Terrain color: ANSI vs RGB

The game and editor already target **24-bit truecolor** (`38;2` / `48;2` in `render/ansi.rs`). Tile appearance uses **`TileDef.fg` as RGB** in the level file. The editor’s F5 picker is a fixed **preset palette** for convenience; there is no separate “ANSI color id” in the format. If you need arbitrary colors later, extend the editor with explicit R/G/B fields or a hex field while keeping the on-disk representation as three `u8`s.

## Design notes for contributors

**Principles, linting, interface style, and compatibility expectations** (including guidance for AI agents) live in **`docs/DESIGN.md`**. Read that file before substantial changes.

1. **Core vs binaries** — Keep terminal setup, event polling, and `stdout` writes in **`tui_game`** / **`tui_level_editor`**. Add gameplay and rendering *data* in **`tui_game_core`**.
2. **Rendering** — Prefer updating the `FrameBuffer` then **delta-encoding** against the previous committed frame; force a full redraw on resize or when wiring a “full redraw” flag.
3. **Saves** — Bump **`SAVE_SCHEMA_VERSION`** / **`schema_version`** when serialized shapes change (v2: inventory + containers + equipment + entity item columns; v3: `quest_stages` on narrative; journal fields on narrative are additive with serde defaults, still v3). Migration logic is optional until the project needs to preserve real user data (see **`docs/DESIGN.md`**).
4. **Content** — Dialogue trees and blueprints live in **`game_content.rs`** as `&'static` data; the **`content`** module holds shared types and validation. Persist quest/world state via **`SaveGameV1`**, not by serializing the whole content pack.
5. **Dependencies** — Prefer small, well-scoped crates (e.g. `serde` + `ron`, `unicode-width`). Avoid pulling in a full TUI framework so the main loop and layout stay explicit.

## License

See `Cargo.toml` (`MIT OR Apache-2.0`).
