//! Pointer: brush, palette pickers, rectangle drag, edge scroll.

use tui_game_core::input::{InputEvent, MouseButton, MouseEventKind};
use tui_game_core::ui::{cell_local_in_rect, EditorHitTarget, UiHitTarget};
use tui_game_core::world::EMPTY_PROP_ID;

use super::{Editor, Mode, PaintLayer, MAX_BRUSH_SIZE};

impl Editor {
    pub fn update_map_hover_from_mouse(&mut self, ev: &InputEvent) {
        let InputEvent::Mouse { cell, .. } = ev else {
            return;
        };
        self.last_mouse_cell = Some(*cell);
        let map_rect = self.map_area_rect();
        self.hover_map_cell = cell_local_in_rect(*cell, map_rect)
            .map(|(lx, ly)| (self.view_origin_x + lx, self.view_origin_y + ly));
    }

    pub fn step_main_mouse(&mut self, ev: &InputEvent) {
        let InputEvent::Mouse {
            kind,
            cell,
            shift,
            ctrl,
            alt: _,
            ..
        } = ev
        else {
            return;
        };

        if matches!(kind, MouseEventKind::Moved) {
            return;
        }

        let map_rect = self.map_area_rect();

        if matches!(kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
            if cell_local_in_rect(*cell, map_rect).is_some() {
                if *ctrl && self.mode == Mode::PaintTiles && self.paint_layer == PaintLayer::Prop {
                    let delta: i8 = if matches!(kind, MouseEventKind::ScrollUp) {
                        1
                    } else {
                        -1
                    };
                    self.brush_sparse_pct =
                        (self.brush_sparse_pct as i16 + delta as i16).clamp(0, 100) as u8;
                    self.status = if self.brush_sparse_pct == 0 {
                        "Prop brush: dense. Ctrl+wheel: sparse % (else clears prop)".into()
                    } else {
                        format!("Prop sparse {}% (else clear)", self.brush_sparse_pct)
                    };
                } else if matches!(kind, MouseEventKind::ScrollUp) {
                    self.brush_radius = (self.brush_radius + 1).min(MAX_BRUSH_SIZE);
                    self.status = format!("Brush radius {}", self.brush_radius);
                } else {
                    self.brush_radius = self.brush_radius.saturating_sub(1);
                    self.status = format!("Brush radius {}", self.brush_radius);
                }
            }
            return;
        }

        if let Some(UiHitTarget::Editor(hit)) = self.ui_hit_at(*cell) {
            if let MouseEventKind::Down(MouseButton::Left) = kind {
                match hit {
                    EditorHitTarget::ClearPropOverlay => {
                        self.set_mode(Mode::PaintTiles);
                        self.paint_layer = PaintLayer::Prop;
                        self.current_tile = EMPTY_PROP_ID;
                        self.status = "Brush: clear prop overlay.".into();
                    }
                    EditorHitTarget::OpenTerrainPicker => {
                        self.open_terrain_picker();
                    }
                    EditorHitTarget::OpenEntityPicker => {
                        self.open_entity_picker();
                    }
                    EditorHitTarget::LayerGround => {
                        self.set_mode(Mode::PaintTiles);
                        self.paint_layer = PaintLayer::Ground;
                        self.ensure_valid_terrain_brush();
                        self.status = "Paint layer: ground.".into();
                    }
                    EditorHitTarget::LayerProp => {
                        self.set_mode(Mode::PaintTiles);
                        self.paint_layer = PaintLayer::Prop;
                        self.status = "Paint layer: props.".into();
                    }
                    EditorHitTarget::ModePaint => {
                        self.set_mode(Mode::PaintTiles);
                        self.status = "Mode: paint tiles.".into();
                    }
                    EditorHitTarget::ModePlace => {
                        self.set_mode(Mode::PlaceSpawns);
                        self.status = "Mode: place entities (LMB on map).".into();
                    }
                    EditorHitTarget::ModeErase => {
                        self.set_mode(Mode::EraseSpawns);
                        self.status = "Mode: erase spawns.".into();
                    }
                    EditorHitTarget::ModePlayer => {
                        self.set_mode(Mode::SetPlayerSpawn);
                        self.status = "Player spawn: click map. Backspace clears.".into();
                    }
                    EditorHitTarget::ModeAtmos => {
                        self.set_mode(Mode::AtmosphereZones);
                        self.status = "Atmosphere: LMB adds zone; Backspace removes last.".into();
                    }
                    EditorHitTarget::PlayerSpawnRow => {
                        self.set_mode(Mode::SetPlayerSpawn);
                        self.status = "Player spawn: click map. Backspace clears.".into();
                    }
                    // Picker rows are only registered while a modal is open (handled there).
                    EditorHitTarget::PickerRow(_) => {}
                }
            }
            return;
        }

        let Some((lx, ly)) = cell_local_in_rect(*cell, map_rect) else {
            return;
        };
        let tx = self.view_origin_x + lx;
        let ty = self.view_origin_y + ly;

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if *shift {
                    self.rect_drag_start = Some((tx, ty));
                    self.last_paint_cell = None;
                } else {
                    self.rect_drag_start = None;
                    match self.mode {
                        Mode::PaintTiles => {
                            if self.paint_layer == PaintLayer::Prop
                                && self.brush_sparse_pct > 0
                                && self.brush_sparse_pct < 100
                            {
                                self.sparse_paint_drag_seen.clear();
                            }
                            self.apply_paint_brush(tx, ty, true);
                            self.set_status_after_dense_paint(tx, ty, false);
                        }
                        Mode::PlaceSpawns => self.place_spawn_at(tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_brush(tx, ty);
                            self.status = format!(
                                "Removed {n} spawn(s) at ({tx},{ty}) r{}.",
                                self.brush_radius
                            );
                        }
                        Mode::SetPlayerSpawn => self.set_player_spawn_at(tx, ty),
                        Mode::AtmosphereZones => {
                            self.add_atmosphere_zone_at(tx, ty);
                        }
                    }
                    self.last_paint_cell = Some((tx, ty));
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.rect_drag_start.is_some() {
                    return;
                }
                if self.last_paint_cell != Some((tx, ty)) {
                    match self.mode {
                        Mode::PaintTiles => {
                            self.apply_paint_brush(tx, ty, true);
                            self.set_status_after_dense_paint(tx, ty, true);
                        }
                        Mode::PlaceSpawns => self.place_spawn_at(tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_brush(tx, ty);
                            if n > 0 {
                                self.status = format!("Removed {n} spawn(s) (drag).");
                            }
                        }
                        Mode::SetPlayerSpawn => self.set_player_spawn_at(tx, ty),
                        Mode::AtmosphereZones => {}
                    }
                    self.last_paint_cell = Some((tx, ty));
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {}
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((sx, sy)) = self.rect_drag_start.take() {
                    match self.mode {
                        Mode::PaintTiles => self.fill_rect_tiles(sx, sy, tx, ty),
                        Mode::EraseSpawns => {
                            let n = self.remove_spawns_in_rect(sx, sy, tx, ty);
                            self.status = format!(
                                "Removed {n} spawn(s) in rectangle ({sx},{sy})—({tx},{ty})."
                            );
                        }
                        Mode::AtmosphereZones | Mode::PlaceSpawns | Mode::SetPlayerSpawn => {}
                    }
                }
                self.last_paint_cell = None;
                self.sparse_paint_drag_seen.clear();
            }
            _ => {}
        }
    }
}
