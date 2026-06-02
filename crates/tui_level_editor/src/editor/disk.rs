//! Level file save and hot-reload (fingerprints for level + terrain pack).

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant, UNIX_EPOCH};

use tui_game_core::content::ContentPack;
use tui_game_core::level::{
    level_from_ron, level_to_ron, materialize_tile_defs_from_pack, LevelFile,
};
use tui_game_core::world::normalize_tile_def_ids;

use super::Editor;

/// `mtime` + `len` so two writes in the same second still register as different when size changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileFingerprint {
    pub len: u64,
    pub modified_ns: Option<u128>,
}

impl FileFingerprint {
    pub const MISSING: Self = Self {
        len: 0,
        modified_ns: None,
    };

    pub fn from_path(path: &Path) -> Option<Self> {
        let m = fs::metadata(path).ok()?;
        let len = m.len();
        let modified_ns = m
            .modified()
            .ok()
            .and_then(|st| st.duration_since(UNIX_EPOCH).ok().map(|d| d.as_nanos()));
        Some(Self { len, modified_ns })
    }
}

impl Editor {
    pub fn pack_fingerprint_for_path(level_path: &Path, level: &LevelFile) -> FileFingerprint {
        let parent = level_path.parent().unwrap_or_else(|| Path::new("."));
        let rel = level.terrain_pack.trim();
        if rel.is_empty() {
            return FileFingerprint::MISSING;
        }
        let pack_path = parent.join(rel);
        FileFingerprint::from_path(&pack_path).unwrap_or(FileFingerprint::MISSING)
    }

    pub fn refresh_disk_fingerprint(&mut self) {
        self.last_level_disk_fingerprint =
            FileFingerprint::from_path(&self.path).unwrap_or(FileFingerprint::MISSING);
        self.last_pack_disk_fingerprint = Self::pack_fingerprint_for_path(&self.path, &self.level);
    }

    /// Parse and validate `path` without mutating editor state.
    pub fn load_level_from_disk(path: &Path, content: &ContentPack) -> Result<LevelFile, String> {
        let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut level = level_from_ron(&raw).map_err(|e| format!("RON parse: {e}"))?;
        materialize_tile_defs_from_pack(&mut level, path.parent())
            .map_err(|e| format!("terrain pack: {e}"))?;
        content
            .validate_level(&level)
            .map_err(|e| format!("level check: {e}"))?;
        Ok(level)
    }

    pub fn try_hot_reload_replace(&mut self) -> Result<(), String> {
        let new_level = Self::load_level_from_disk(&self.path, &self.content)?;
        self.apply_reloaded_level(new_level);
        self.status = format!("Hot-reloaded {}", self.path.display());
        Ok(())
    }

    /// Called every frame from the main loop. Reloads when the file changes on disk if safe.
    pub fn poll_hot_reload(&mut self) {
        const INTERVAL: Duration = Duration::from_millis(250);
        if self.last_hot_reload_poll.elapsed() < INTERVAL {
            return;
        }
        self.last_hot_reload_poll = Instant::now();
        if self.dialog.is_some() {
            return;
        }
        let Some(level_fp) = FileFingerprint::from_path(&self.path) else {
            return;
        };
        let pack_fp = Self::pack_fingerprint_for_path(&self.path, &self.level);
        if level_fp == self.last_level_disk_fingerprint
            && pack_fp == self.last_pack_disk_fingerprint
        {
            return;
        }
        if !self.dirty {
            match self.try_hot_reload_replace() {
                Ok(()) => {}
                Err(e) => {
                    self.status = format!("Hot-reload skipped: {e}");
                    self.last_level_disk_fingerprint = level_fp;
                    self.last_pack_disk_fingerprint = pack_fp;
                }
            }
            return;
        }
        self.dialog = Some(super::Dialog::HotReloadUnsaved);
        self.status =
            "File changed on disk (unsaved edits). Y: reload & discard   N/Esc: keep editing."
                .into();
    }

    pub fn save(&mut self) -> Result<(), String> {
        normalize_tile_def_ids(&mut self.level.tile_defs);
        if !self.level.terrain_pack.trim().is_empty() {
            self.level.terrain_palette = self
                .level
                .tile_defs
                .iter()
                .map(|d| d.terrain_id.clone())
                .collect();
            self.level.tile_defs.clear();
        } else {
            self.level.terrain_palette.clear();
        }
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
}
