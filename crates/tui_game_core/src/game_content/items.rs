use crate::item::{EquipSlot, ItemCategory, ItemDef, WeaponKind};

use super::glyphs;

pub static ITEM_DEFS: &[ItemDef] = &[
    ItemDef {
        id: "cellar_key",
        name: "Cellar key",
        description: "Heavy iron; faint cellar mold. Opens the old cellar (not implemented).",
        glyph: glyphs::ITEM_KEY,
        category: ItemCategory::Mundane,
        weapon: None,
    },
    ItemDef {
        id: "health_tonic",
        name: "Health tonic",
        description: "Bitter red syrup. Labels promise vigor (effect not implemented).",
        glyph: glyphs::ITEM_CONSUMABLE,
        category: ItemCategory::Consumable,
        weapon: None,
    },
    ItemDef {
        id: "brass_ring",
        name: "Brass ring",
        description: "Worn smooth. Fits a finger; no enchantment detected yet.",
        glyph: glyphs::ITEM_RING,
        category: ItemCategory::Equippable(EquipSlot::Ring),
        weapon: None,
    },
    ItemDef {
        id: "iron_sword",
        name: "Iron sword",
        description: "A balanced blade. Better to-hit and damage than bare fists.",
        glyph: glyphs::ITEM_MELEE,
        category: ItemCategory::Equippable(EquipSlot::MainHand),
        weapon: Some(WeaponKind::Melee {
            to_hit: 2,
            damage_bonus: 2,
        }),
    },
    ItemDef {
        id: "hunting_bow",
        name: "Hunting bow",
        description: "Tensioned yew. Shoots at range if arrows are loaded in the quiver (e).",
        glyph: glyphs::ITEM_BOW,
        category: ItemCategory::Equippable(EquipSlot::MainHand),
        weapon: Some(WeaponKind::RangedBow {
            to_hit: 1,
            damage_bonus: 1,
            range: 15,
        }),
    },
    ItemDef {
        id: "arrow",
        name: "Arrow",
        description:
            "Simple bodkin points. Load with e while highlighted; bow spends one per shot.",
        glyph: glyphs::ITEM_AMMO,
        category: ItemCategory::Ammo,
        weapon: None,
    },
];
