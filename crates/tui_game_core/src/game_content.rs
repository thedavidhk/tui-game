//! Static narrative and entity definitions for this game. Uses types from [`crate::content`];
//! keep [`crate::content`] free of game-specific tables so validation and pack shape stay reusable.

use std::collections::HashMap;

use crate::content::{
    ContentPack, DialogueChoice, DialogueNode, DialogueTree, EntityBlueprint,
};

static DC_GUIDE_0: [DialogueChoice; 2] = [
    DialogueChoice {
        label: "I'll listen.",
        next: 1,
    },
    DialogueChoice {
        label: "Not interested.",
        next: 2,
    },
];
/// `next == tree.nodes.len()` means close dialogue (sentinel).
static DC_GUIDE_1: [DialogueChoice; 1] = [DialogueChoice {
    label: "Farewell.",
    next: 3,
}];
static DC_GUIDE_2: [DialogueChoice; 1] = [DialogueChoice {
    label: "Leave.",
    next: 3,
}];

static NODES_GUIDE: [DialogueNode; 3] = [
    DialogueNode {
        text: "Welcome, traveler. The old cellar key is yours if you listen.",
        choices: &DC_GUIDE_0,
    },
    DialogueNode {
        text: "Good. Remember: the plan is only beginning.",
        choices: &DC_GUIDE_1,
    },
    DialogueNode {
        text: "As you wish.",
        choices: &DC_GUIDE_2,
    },
];

static TREE_GUIDE: DialogueTree = DialogueTree {
    id: "guide",
    nodes: &NODES_GUIDE,
};

static ENTITY_BLUEPRINTS: &[EntityBlueprint] = &[
    EntityBlueprint {
        kind: "guide",
        display_name: "Guide",
        description: "Demo NPC; dialogue id \"guide\".",
        default_glyph: 'g',
        default_label: "Guide",
        dialogue_id: Some("guide"),
    },
    EntityBlueprint {
        kind: "prop",
        display_name: "Prop",
        description: "Set dressing; no dialogue hook.",
        default_glyph: '*',
        default_label: "Crate",
        dialogue_id: None,
    },
];

/// The default [`ContentPack`] for this workspace (game + level editor).
#[must_use]
pub fn content_pack() -> ContentPack {
    let mut dialogues = HashMap::new();
    dialogues.insert("guide", &TREE_GUIDE);
    ContentPack {
        dialogues,
        guide_dialogue: &TREE_GUIDE,
        entity_blueprints: ENTITY_BLUEPRINTS,
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
