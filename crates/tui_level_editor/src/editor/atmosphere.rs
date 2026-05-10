//! Atmosphere zones (placement on map).

use tui_game_core::level::{AtmosphereShape, AtmosphereZone};

use super::Editor;

impl Editor {
    pub fn add_atmosphere_zone_at(&mut self, ax: i32, ay: i32) {
        self.level.atmosphere_zones.push(AtmosphereZone {
            anchor_x: ax,
            anchor_y: ay,
            shape: AtmosphereShape::Rectangle {
                width_tiles: 5,
                height_tiles: 5,
            },
            edge_falloff_tiles: 2,
            recipe: self.level.default_atmosphere,
        });
        self.mark_dirty();
        self.rebuild_tile_display_full();
        self.status = format!(
            "Atmosphere zone {} at ({ax},{ay})",
            self.level.atmosphere_zones.len()
        );
    }

    pub fn remove_last_atmosphere_zone(&mut self) {
        if self.level.atmosphere_zones.pop().is_some() {
            self.mark_dirty();
            self.rebuild_tile_display_full();
            self.status = "Removed last atmosphere zone.".into();
        } else {
            self.status = "No atmosphere zones.".into();
        }
    }
}
