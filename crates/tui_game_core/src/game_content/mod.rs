//! Static narrative and entity definitions for this game.
//! Keep [`crate::content`] free of game-specific tables so validation and pack shape stay reusable.

use std::collections::HashMap;

use crate::content::{ContentPack, DialogueNode};
use crate::narrative::NarrativeState;

mod blueprints;
pub mod dialogues;
mod items;
mod macros;
pub mod quests;
mod triggers;

pub(crate) use macros::{dialogue_tree, effects, quest_defs, requires};

#[must_use]
pub fn resolve_dialogue_text(node: &DialogueNode, narrative: &NarrativeState) -> String {
    node.text_fn.map_or_else(|| node.text.to_string(), |f| f(narrative))
}

/// The default [`ContentPack`] for this workspace (game + level editor).
#[must_use]
pub fn content_pack() -> ContentPack {
    let mut dialogues = HashMap::new();
    dialogues.insert("guide", &dialogues::TREE_GUIDE);
    dialogues.insert("healer", &dialogues::TREE_HEALER);
    dialogues.insert("scholar", &dialogues::TREE_SCHOLAR);
    dialogues.insert("merchant", &dialogues::TREE_MERCHANT);
    ContentPack {
        dialogues,
        guide_dialogue: &dialogues::TREE_GUIDE,
        quest_defs: quests::QUEST_DEFS,
        trigger_rules: triggers::TRIGGER_RULES,
        entity_blueprints: blueprints::ENTITY_BLUEPRINTS,
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
    fn validate_level_rejects_unknown_spawn_kind() {
        let p = content_pack();
        let table = TileTable::default_pack();
        let map = MapGrid::filled(4, 4, 0, table);
        let level = LevelFile::from_map(
            &map,
            "x",
            vec![EntitySpawn {
                kind: "no_such_npc".into(),
                x: 1,
                y: 1,
                glyph: 'x',
                name: "X".into(),
            }],
        );
        let err = p.validate_level(&level).unwrap_err();
        assert!(err.contains("no_such_npc"), "{err}");
    }

    #[test]
    fn validate_level_rejects_unknown_tile_id() {
        let p = content_pack();
        let table = TileTable::default_pack();
        let mut level = LevelFile::from_map(&MapGrid::filled(2, 2, 0, table), "x", vec![]);
        level.tiles[0] = 99;
        let err = p.validate_level(&level).unwrap_err();
        assert!(err.contains("unknown tile id"), "{err}");
    }
}
