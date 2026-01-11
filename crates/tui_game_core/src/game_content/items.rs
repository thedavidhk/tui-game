use crate::item::{EquipSlot, ItemCategory, ItemDef};

pub static ITEM_DEFS: &[ItemDef] = &[
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
