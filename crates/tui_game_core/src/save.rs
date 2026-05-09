use serde::{Deserialize, Serialize};

use crate::entity::EntityArena;
use crate::game::GameModeStack;
use crate::narrative::NarrativeState;
use crate::world::{normalize_tile_def_ids, MapGrid};

pub const SAVE_SCHEMA_VERSION: u32 = 7;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorldSnapshot {
    pub map: MapGrid,
    pub entities: EntityArena,
    pub narrative: NarrativeState,
    pub rng_seed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SaveGameV1 {
    pub schema_version: u32,
    pub build: Option<String>,
    pub world: WorldSnapshot,
    pub modes: GameModeStack,
}

impl SaveGameV1 {
    pub fn new(world: WorldSnapshot, modes: GameModeStack) -> Self {
        Self {
            schema_version: SAVE_SCHEMA_VERSION,
            build: option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            world,
            modes,
        }
    }
}

pub fn save_to_ron(s: &SaveGameV1) -> Result<String, ron::Error> {
    ron::ser::to_string_pretty(s, ron::ser::PrettyConfig::new())
}

pub fn save_from_ron(s: &str) -> Result<SaveGameV1, ron::de::SpannedError> {
    let mut sg: SaveGameV1 = ron::from_str(s)?;
    normalize_tile_def_ids(&mut sg.world.map.table.defs);
    Ok(sg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;
    use crate::game::GameMode;
    use crate::item::{EquipSlot, Inventory};
    use crate::world::TileTable;

    #[test]
    fn round_trip_save() {
        let map = MapGrid::filled(
            3,
            3,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        let mut ents = EntityArena::new();
        let p = ents.spawn(
            crate::entity::GridPos { x: 1, y: 1 },
            '@',
            crate::render::Color::rgb(255, 235, 180),
            "Hero".into(),
            false,
            None,
            None,
            false,
        );
        ents.set_player(p);
        let world = WorldSnapshot {
            map,
            entities: ents,
            narrative: NarrativeState::default(),
            rng_seed: 42,
        };
        let modes = GameModeStack {
            stack: vec![GameMode::Exploration],
        };
        let sg = SaveGameV1::new(world.clone(), modes.clone());
        let ron = save_to_ron(&sg).unwrap();
        let back: SaveGameV1 = save_from_ron(&ron).unwrap();
        assert_eq!(back.schema_version, SAVE_SCHEMA_VERSION);
        assert_eq!(back.world, world);
        assert_eq!(back.modes, modes);
    }

    #[test]
    fn round_trip_save_with_inventory_and_containers() {
        let map = MapGrid::filled(
            3,
            3,
            0,
            TileTable::default_pack().expect("default terrain pack must load"),
        );
        let mut ents = EntityArena::new();
        let p = ents.spawn(
            crate::entity::GridPos { x: 1, y: 1 },
            '@',
            crate::render::Color::rgb(255, 235, 180),
            "Hero".into(),
            false,
            None,
            None,
            false,
        );
        ents.set_player(p);
        let mut narrative = NarrativeState::default();
        narrative.inventory.add("cellar_key", 1);
        narrative.container_inventories.insert(7, {
            let mut c = Inventory::default();
            c.add("health_tonic", 2);
            c
        });
        narrative
            .equipment
            .insert(EquipSlot::Ring, "brass_ring".into());
        narrative.quest_stages.insert("side_quest".into(), 2);
        let world = WorldSnapshot {
            map,
            entities: ents,
            narrative,
            rng_seed: 3,
        };
        let modes = GameModeStack {
            stack: vec![GameMode::ItemTransfer {
                container: EntityId(7),
                focus: crate::game::TransferFocus::Player,
                cursor_player: 0,
                cursor_container: 0,
            }],
        };
        let sg = SaveGameV1::new(world.clone(), modes.clone());
        let ron = save_to_ron(&sg).unwrap();
        let back: SaveGameV1 = save_from_ron(&ron).unwrap();
        assert_eq!(back.world, world);
        assert_eq!(back.modes, modes);
    }
}
