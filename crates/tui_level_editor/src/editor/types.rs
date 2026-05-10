//! Editor mode, paint layer, sidebar hit targets, and modal dialog payloads.

use tui_game_core::ui::{SearchListPicker, TextField};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    PaintTiles,
    PlaceSpawns,
    EraseSpawns,
    /// Single-cell marker: where the runtime spawns the player (`LevelFile.player_spawn`).
    SetPlayerSpawn,
    /// Place atmosphere zone volumes (click map).
    AtmosphereZones,
}

/// Which grid [`tui_game_core::level::LevelFile::tiles`] (ground) vs [`LevelFile::props`] receives paint / sparse brush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintLayer {
    Ground,
    Prop,
}

#[derive(Clone, Copy, Debug)]
pub enum SidebarHit {
    /// Clear prop overlay (only listed when painting props).
    ClearPropOverlay,
    OpenTerrainPicker,
    OpenEntityPicker,
    LayerGround,
    LayerProp,
    ModePaint,
    ModePlace,
    ModeErase,
    ModePlayer,
    ModeAtmos,
    PlayerSpawnRow,
}

pub enum Dialog {
    SavePath {
        field: TextField,
    },
    PickTerrain {
        picker: SearchListPicker,
    },
    PickEntity {
        picker: SearchListPicker,
    },
    /// External file changed while the in-memory level has unsaved edits.
    HotReloadUnsaved,
}
