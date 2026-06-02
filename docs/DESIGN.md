# Design & contributor guide

## 1. Interfaces and complexity

**Goal:** keep the codebase understandable as features accumulate, by favoring **small, deep interfaces** over wide, shallow APIs.

- **Deep interface** — A few types and functions hide most of the branching and invariants (e.g. `Game::step` + `InputBatch`, `LevelFile::to_map`, `FrameBuffer` + ANSI encoding). Callers should not need to know implementation details to use them correctly.
- **Narrow surface** — Prefer `pub` only where other crates or modules need it. Internal modules can stay private with a single re-export layer (`lib.rs`, `mod.rs`).
- **Boundaries** — **`tui_game_core`** owns simulation, level/save **data** types, rendering buffers, and UI **composition into** `FrameBuffer`. **`tui_game`** and **`tui_level_editor`** own terminals: raw mode, resize, stdout, and the main loop. **`tui_terminal`** is the one place that maps crossterm events into core's backend-agnostic `InputEvent` (shared by both binaries). Do not pull crossterm into core except where already justified (e.g. nothing today beyond `std::fs` for saves in core—keep it that way).
- **When the project grows** — If a module file becomes a grab-bag, split by **responsibility** (e.g. world vs narrative vs UI widgets), not by layer duplication. If two binaries need the same terminal glue, extract a tiny internal crate or shared module—only when the duplication is real.

Agents: before adding `pub fn` helpers everywhere, ask whether behavior belongs behind one existing entry point.

**UI / quest refactors** — Ongoing roadmap for layout, hit testing, mode split, and narrative effects: [`docs/REFACTOR_QUEST_INVENTORY_UI.md`](REFACTOR_QUEST_INVENTORY_UI.md).

---

## 2. Code quality and Rust conventions

**Goal:** **high-quality, maintainable** Rust—clear types, honest error handling, and lints that catch foot-guns early.

- **`rustfmt`** — Use default formatting; do not fight the tool in new code.
- **Clippy** — Treat **`clippy::pedantic`** as the **aspirational bar** for new and touched code:

  ```bash
  cargo clippy --workspace --all-targets -- -W clippy::pedantic
  ```

  The workspace is not required to be pedantic-clean at every moment; fix warnings in code you modify, and reduce noise over time. If the team later enables workspace `[lints]`, document the chosen groups (e.g. pedantic + a subset of nursery) in a PR and fix fallout in one go.

- **Errors** — Prefer **explicit** error handling (`Result`, meaningful `String` or typed errors in core) over panics in library paths. Binaries may exit on fatal IO failures where appropriate.
- **Idiomatic modeling** — Prefer Rust’s type system to encode invariants instead of runtime flags:
  - **State machines** — Use `enum` variants with associated data for modes and phases (e.g. game/UI state), not one large struct full of `Option` fields where only some combinations are valid. Invalid states should be **unrepresentable**; `match` should stay exhaustive.
  - **Traits** — When several types share behavior (rendering a row, applying an effect, parsing input), extract a small trait or shared helper rather than duplicating branches or growing a god struct.
  - **Refactoring toward idioms** — When touching prototype code, nudge it toward these patterns if the change is local and clear; avoid drive-by rewrites of unrelated modules.
- **Tests** — Add or extend **headless** tests in `tui_game_core` for rules and serialization (`cargo test -p tui_game_core`). UI binaries stay thin; logic worth keeping should be testable in core.

Agents: run `cargo test --workspace` and clippy (as above) before considering work complete unless the task is explicitly local.

---

## 3. Evolving structure and technical debt

**Goal:** in this **early stage**, **re-evaluate layout and boundaries regularly** so debt does not accumulate “because we were in a hurry once.”

- **Refactors are normal** — Moving modules, renaming for clarity, or splitting a type is **encouraged** when it reduces coupling or clarifies ownership. Prefer one focused PR over a long-lived branch.
- **Triggers to refactor** — Cyclic `use` dependencies, files mixing unrelated concerns, duplicated protocol between editor and game, or “only the tests know how to construct X” are signals to simplify.
- **Dependencies** — Keep dependencies **small and scoped** (see README). Adding a full TUI framework or heavy ECS is a deliberate architectural decision—default is **no**.

Agents: if a change makes a module harder to describe in one sentence, propose a structural follow-up (same PR if small, otherwise note it in the PR description).

---

## 4. Backward compatibility and versioning

**Goal:** **do not organize the design around backward compatibility** at this phase. Ship the **simplest correct model**; avoid migration machinery until there is a real need (e.g. players with irreplaceable saves).

- **Level / save schemas** — `schema_version` (and similar) exist so formats can be **identified** and bumped cleanly later. Bumping and breaking RON layout is **acceptable**; full migration paths are **optional** until the project commits to stable releases.
- **Where cheap** — Serde defaults, additive fields, or a one-line version check are fine when they cost almost nothing. **Do not** preemptively build compatibility shims, feature flags, or multi-version loaders “just in case.”
- **APIs** — `tui_game_core` is not a semver-stable public crate for external consumers yet; internal refactors that break call sites in-repo are fine if tests pass.

Agents: default to **clean break + test update** over compatibility layers unless the user explicitly asks for migration support.

---

## 5. Product direction (editor, content, levels)

Short **north star** so code and tools stay aligned (details may evolve):

- **Content in Rust** — Dialogues, quests, entity **definitions**, and gameplay rules live in Rust under **`game_content`** (tables) plus **`content`** (types / `ContentPack` / validation), not in hand-authored RON as primary source.
- **Levels as data** — `LevelFile` (and similar) should stay **thin**: geometry, tile table, spawn **references** (stable string ids / blueprint ids), and instance overrides where needed—not full behavior trees. Serialized levels for this repo live under **`assets/levels/`** (e.g. `demo_level.ron`); the editor’s default open path and the game’s embedded menu level should stay aligned with that tree.
- **Terrain ids** — Each `TileDef` in RON has a stable string **`id`** (snake_case `terrain_id` in Rust) plus a human-readable **`name`**. Runtime assigns **`idx == index`** in the resolved `tile_defs` slice (`normalize_tile_def_ids`). Numeric **`tiles`** / **`props`** indices use that slice. When a level sets **`terrain_pack`** and **`terrain_palette`**, palette order defines those indices so **definitions may be listed in any order inside the pack file**; without a palette, indices follow the pack file order (legacy). The editor saves **`terrain_palette`** and omits inline **`tile_defs`** when a pack path is set.
- **Editor ergonomics** — The level editor should surface **names and previews** from core data (tile defs, spawn catalog), not raw numeric ids alone. The **`EntityBlueprint`** slice on `ContentPack` (filled by **`game_content::content_pack`**) is the registry (`kind` → defaults + optional `dialogue_id`); the editor lists blueprints and terrains from that data. **`ContentPack::validate_level`** keeps levels aligned with the pack.

Agents: when adding editor features, prefer reading from the same definitions the game will load, rather than duplicating strings in the editor.

---

## 6. Visual atmosphere (terrain backgrounds and fog)

- **Terrain `TileDef.bg`** — Optional per-terrain background; when omitted, the runtime derives a darkened background from `fg`. Static variants and animation frames still share one `bg` per def; it is baked into `TileDisplayCell` at rebuild time.
- **`AtmosphereRecipe` and zones** — `LevelFile` / `MapGrid` carry a **`default_atmosphere`** (`void_background`, `void_glyph_foreground`, `visible_background_pull` `0..=100`, and reserved **`sight_strength`**) plus a list of **`AtmosphereZone`** entries (anchor tile, `AtmosphereShape` rectangle or circle, `edge_falloff_tiles`, per-zone recipe). Overlap is resolved at **bake time** in `world::atmosphere` (weights from shape + smooth falloff, normalized blend of colors and pull). Serialized levels and saves store recipes and zones only, not per-cell colors.
- **Per-cell bake** — After `MapGrid::rebuild_display_cache`, **`rebuild_atmosphere_bake`** fills **`FogBakedTrio`** per cell: composed fg/bg for void/unseen, explored, and visible fog states using that cell’s resolved recipe and the baked terrain display. Per frame, **`compose_fog_from_luminance`** lerps between these three endpoints using the same smoothed fog luminance as before (`FOG_COLOR_SOFTEN_RADIUS_CHEBYSHEV` in `world::fog_visual`). **`compose_map_tile_discrete`** is used where a single fog state is enough (e.g. editor terrain preview as “visible”).
- **Line of sight** — v1 keeps a **global circular** LOS; **`sight_strength`** on **`default_atmosphere` only** scales effective radius via **`effective_fow_radius_cells`** (clamped). Zonal `sight_strength` is stored for forward compatibility but not applied to LOS yet.
- **Future (not implemented)** — Directional or ray-based **visibility budget** (per-cell cost along LOS, campfires that extend sight along a cone) is intentionally deferred; it would plug into the same resolved atmosphere data.
- **Terrain pack** — Tile definitions may live in a separate **`TerrainPack`** RON referenced by **`LevelFile::terrain_pack`**. **`materialize_tile_defs_from_pack`** loads the pack and **`apply_terrain_pack_to_level`** resolves defs using **`terrain_palette`** when present. The level editor hot-reload fingerprints **both** the level file and the resolved pack path.
