//! Static narrative and entity definitions for this game.
//! Keep [`crate::content`] free of game-specific tables so validation and pack shape stay reusable.

use crate::content::{ContentPack, ContentRuntimeHooks, DialogueNode, DialogueTree};
use crate::narrative::NarrativeState;

mod blueprints;
mod items;
mod macros;
mod npcs;
pub mod quests;

pub(crate) use macros::{dialogue_tree, effects, quest_defs, requires};

/// Default serialized level body: `assets/levels/demo_level.ron` (references terrain pack).
const EMBEDDED_DEMO_LEVEL_RON: &str = include_str!("../../../../assets/levels/demo_level.ron");
/// Default terrain definitions: `assets/terrains/demo_terrain_pack.ron`.
const EMBEDDED_DEMO_TERRAIN_PACK_RON: &str = include_str!("../../../../assets/terrains/demo_terrain_pack.ron");

/// Parsed embedded demo level with inline `tile_defs` (pack merged at compile time; `terrain_pack` cleared).
#[must_use]
pub fn embedded_demo_level() -> crate::level::LevelFile {
    use crate::level::{terrain_pack_from_ron, LevelFile};
    let mut level: LevelFile =
        crate::level::level_from_ron(EMBEDDED_DEMO_LEVEL_RON).expect("embedded demo_level.ron");
    let pack = terrain_pack_from_ron(EMBEDDED_DEMO_TERRAIN_PACK_RON)
        .expect("embedded demo_terrain_pack.ron must parse");
    level.tile_defs = pack.tile_defs;
    level.terrain_pack.clear();
    level
}

#[derive(Debug)]
struct GameContentRuntimeHooks;

impl ContentRuntimeHooks for GameContentRuntimeHooks {
    fn resolve_dialogue_text(&self, node: &DialogueNode, narrative: &NarrativeState) -> String {
        node.text_fn.map_or_else(|| node.text.to_string(), |f| f(narrative))
    }

    fn dialogue_start_node(
        &self,
        dialogue_id: &str,
        tree: &'static DialogueTree,
        narrative: &NarrativeState,
    ) -> usize {
        npcs::dialogue_start_node(dialogue_id, tree, narrative)
    }

    fn hud_quest_status_lines(&self, narrative: &NarrativeState) -> Vec<String> {
        npcs::hud_quest_status_lines(narrative)
    }

    fn training_spar_epilogue_node(&self, player_hp: u16, trainer_hp: u16) -> &'static str {
        npcs::training_spar_epilogue_node(player_hp, trainer_hp)
    }

    fn on_item_picked(
        &self,
        item_id: &str,
        narrative: &mut NarrativeState,
        log: &mut Vec<String>,
    ) -> Result<(), crate::narrative::NarrativeApplyError> {
        npcs::on_item_picked(item_id, narrative, log)
    }

    fn on_region_enter(
        &self,
        region_id: &str,
        narrative: &mut NarrativeState,
        log: &mut Vec<String>,
    ) -> Result<(), crate::narrative::NarrativeApplyError> {
        npcs::on_region_enter(region_id, narrative, log)
    }
}

static GAME_CONTENT_RUNTIME_HOOKS: GameContentRuntimeHooks = GameContentRuntimeHooks;

/// The default [`ContentPack`] for this workspace (game + level editor).
#[must_use]
pub fn content_pack() -> ContentPack {
    fn entity_blueprints() -> &'static [crate::content::EntityBlueprint] {
        use std::sync::OnceLock;
        static ALL: OnceLock<Vec<crate::content::EntityBlueprint>> = OnceLock::new();
        ALL.get_or_init(|| {
            let mut out = Vec::new();
            out.extend_from_slice(npcs::npc_blueprints());
            out.extend_from_slice(blueprints::ENTITY_BLUEPRINTS);
            out
        })
    }

    let dialogues = npcs::dialogue_map();
    ContentPack {
        dialogues,
        default_dialogue: npcs::default_dialogue(),
        runtime_hooks: &GAME_CONTENT_RUNTIME_HOOKS,
        quest_defs: quests::QUEST_DEFS,
        entity_blueprints: entity_blueprints(),
        item_defs: items::ITEM_DEFS,
    }
}

#[cfg(test)]
mod tests {
    use crate::level::{EntitySpawn, LevelFile};
    use crate::world::{MapGrid, TileTable};

    use super::content_pack;

    #[test]
    fn content_pack_validates() {
        let p = content_pack();
        p.validate().unwrap();
    }

    #[test]
    fn embedded_demo_level_validates() {
        let p = content_pack();
        let level = super::embedded_demo_level();
        p.validate_level(&level).unwrap();
    }

    #[test]
    fn validate_level_rejects_unknown_spawn_kind() {
        let p = content_pack();
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let map = MapGrid::filled(4, 4, 0, table);
        let level = LevelFile::from_map(
            &map,
            "x",
            vec![EntitySpawn {
                kind: "no_such_npc".into(),
                x: 1,
                y: 1,
                glyph_override: None,
                name_override: None,
                fg_override: None,
            }],
        );
        let err = p.validate_level(&level).unwrap_err();
        assert!(err.contains("no_such_npc"), "{err}");
    }

    #[test]
    fn validate_level_rejects_unknown_tile_id() {
        let p = content_pack();
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let mut level = LevelFile::from_map(&MapGrid::filled(2, 2, 0, table), "x", vec![]);
        level.tiles[0] = 99;
        let err = p.validate_level(&level).unwrap_err();
        assert!(err.contains("unknown tile id"), "{err}");
    }

    #[test]
    fn validate_level_rejects_atmosphere_zone_zero_rectangle() {
        use crate::level::{AtmosphereRecipe, AtmosphereShape, AtmosphereZone};
        let p = content_pack();
        let table = TileTable::default_pack().expect("default terrain pack must load");
        let mut level = LevelFile::from_map(&MapGrid::filled(2, 2, 0, table), "x", vec![]);
        level.atmosphere_zones.push(AtmosphereZone {
            anchor_x: 0,
            anchor_y: 0,
            shape: AtmosphereShape::Rectangle {
                width_tiles: 0,
                height_tiles: 3,
            },
            edge_falloff_tiles: 1,
            recipe: AtmosphereRecipe::default(),
        });
        let err = p.validate_level(&level).unwrap_err();
        assert!(err.contains("zero width"), "{err}");
    }
}
