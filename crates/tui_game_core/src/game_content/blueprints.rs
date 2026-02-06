use crate::content::EntityBlueprint;

pub const BLUEPRINT_PROP: EntityBlueprint = EntityBlueprint {
    kind: "prop",
    display_name: "Prop",
    description: "Set dressing; no dialogue hook.",
    default_glyph: '*',
    default_label: "Crate",
    dialogue_id: None,
    world_item: None,
    is_container: false,
    base_max_hp: 1,
    base_strength: 1,
    base_agility: 1,
    base_speed: 1,
};

pub const BLUEPRINT_HEALTH_TONIC_DROP: EntityBlueprint = EntityBlueprint {
    kind: "health_tonic_drop",
    display_name: "Health tonic",
    description: "World pickup for healer quest.",
    default_glyph: '!',
    default_label: "Tonic",
    dialogue_id: None,
    world_item: Some("health_tonic"),
    is_container: false,
    base_max_hp: 1,
    base_strength: 1,
    base_agility: 1,
    base_speed: 1,
};

pub const BLUEPRINT_BRASS_RING_DROP: EntityBlueprint = EntityBlueprint {
    kind: "brass_ring_drop",
    display_name: "Brass ring",
    description: "World pickup for scholar quest.",
    default_glyph: '=',
    default_label: "Ring",
    dialogue_id: None,
    world_item: Some("brass_ring"),
    is_container: false,
    base_max_hp: 1,
    base_strength: 1,
    base_agility: 1,
    base_speed: 1,
};

pub const BLUEPRINT_WOODEN_CHEST: EntityBlueprint = EntityBlueprint {
    kind: "wooden_chest",
    display_name: "Wooden chest",
    description: "Storage; press E nearby to open transfer.",
    default_glyph: '□',
    default_label: "Chest",
    dialogue_id: None,
    world_item: None,
    is_container: true,
    base_max_hp: 20,
    base_strength: 1,
    base_agility: 1,
    base_speed: 1,
};

pub static ENTITY_BLUEPRINTS: &[EntityBlueprint] = &[
    BLUEPRINT_PROP,
    BLUEPRINT_HEALTH_TONIC_DROP,
    BLUEPRINT_BRASS_RING_DROP,
    BLUEPRINT_WOODEN_CHEST,
];
