//! Static narrative and entity definitions for this game. Uses types from [`crate::content`];
//! keep [`crate::content`] free of game-specific tables so validation and pack shape stay reusable.

use std::collections::HashMap;

use crate::content::{
    Condition, ContentPack, DemoQuestPhase, DialogueChoice, DialogueNode, DialogueTree, Effect,
    EntityBlueprint,
};
use crate::item::{EquipSlot, ItemCategory, ItemDef};

static REQ_NONE: &[Condition] = &[];
static REQ_CELLAR_KEY: &[Condition] = &[Condition::HasItem("cellar_key")];

static EFF_TALKED: &[Effect] = &[Effect::SetDemoQuest(DemoQuestPhase::TalkedToGuide)];
static EFF_NONE: &[Effect] = &[];
static EFF_RETURN_KEY: &[Effect] = &[
    Effect::TakeItem("cellar_key"),
    Effect::SetDemoQuest(DemoQuestPhase::ReturnedKey),
];
static EFF_GIVE_KEY: &[Effect] = &[
    Effect::GiveItem("cellar_key"),
    Effect::SetDemoQuest(DemoQuestPhase::HasCellarKey),
];

static DC_GUIDE_0: [DialogueChoice; 3] = [
    DialogueChoice {
        label: "I'll listen.",
        next: 1,
        requires: REQ_NONE,
        effects: EFF_TALKED,
    },
    DialogueChoice {
        label: "Not interested.",
        next: 2,
        requires: REQ_NONE,
        effects: EFF_NONE,
    },
    DialogueChoice {
        label: "I brought the cellar key back.",
        next: 3,
        requires: REQ_CELLAR_KEY,
        effects: EFF_RETURN_KEY,
    },
];
/// `next == tree.nodes.len()` means close dialogue (sentinel).
static DC_GUIDE_1: [DialogueChoice; 1] = [DialogueChoice {
    label: "I'll take it.",
    next: 4,
    requires: REQ_NONE,
    effects: EFF_GIVE_KEY,
}];
static DC_GUIDE_2: [DialogueChoice; 1] = [DialogueChoice {
    label: "Farewell.",
    next: 4,
    requires: REQ_NONE,
    effects: EFF_NONE,
}];
static DC_GUIDE_3: [DialogueChoice; 1] = [DialogueChoice {
    label: "Farewell.",
    next: 4,
    requires: REQ_NONE,
    effects: EFF_NONE,
}];

static NODES_GUIDE: [DialogueNode; 4] = [
    DialogueNode {
        text: "Welcome, traveler. Listen, and the cellar key may be yours.",
        choices: &DC_GUIDE_0,
    },
    DialogueNode {
        text: "Good. Here is the cellar key.",
        choices: &DC_GUIDE_1,
    },
    DialogueNode {
        text: "As you wish.",
        choices: &DC_GUIDE_2,
    },
    DialogueNode {
        text: "Thank you. Safe travels.",
        choices: &DC_GUIDE_3,
    },
];

static TREE_GUIDE: DialogueTree = DialogueTree {
    id: "guide",
    nodes: &NODES_GUIDE,
};

static ITEM_DEFS: &[ItemDef] = &[
    ItemDef {
        id: "cellar_key",
        name: "Cellar key",
        description: "Heavy iron; faint cellar mold. Opens the old cellar (not implemented).",
        glyph: ',',
        category: ItemCategory::Mundane,
    },
    ItemDef {
        id: "health_tonic",
        name: "Health tonic",
        description: "Bitter red syrup. Labels promise vigor (effect not implemented).",
        glyph: '!',
        category: ItemCategory::Consumable,
    },
    ItemDef {
        id: "brass_ring",
        name: "Brass ring",
        description: "Worn smooth. Fits a finger; no enchantment detected yet.",
        glyph: '=',
        category: ItemCategory::Equippable(EquipSlot::Ring),
    },
];

static ENTITY_BLUEPRINTS: &[EntityBlueprint] = &[
    EntityBlueprint {
        kind: "guide",
        display_name: "Guide",
        description: "Demo NPC; dialogue id \"guide\".",
        default_glyph: 'g',
        default_label: "Guide",
        dialogue_id: Some("guide"),
        world_item: None,
        is_container: false,
    },
    EntityBlueprint {
        kind: "prop",
        display_name: "Prop",
        description: "Set dressing; no dialogue hook.",
        default_glyph: '*',
        default_label: "Crate",
        dialogue_id: None,
        world_item: None,
        is_container: false,
    },
    EntityBlueprint {
        kind: "cellar_key_drop",
        display_name: "Cellar key",
        description: "World pickup for fetch demo.",
        default_glyph: ',',
        default_label: "Key",
        dialogue_id: None,
        world_item: Some("cellar_key"),
        is_container: false,
    },
    EntityBlueprint {
        kind: "wooden_chest",
        display_name: "Wooden chest",
        description: "Storage; press E nearby to open transfer.",
        default_glyph: '□',
        default_label: "Chest",
        dialogue_id: None,
        world_item: None,
        is_container: true,
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
        item_defs: ITEM_DEFS,
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
