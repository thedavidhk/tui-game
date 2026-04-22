use crate::content::{
    Disposition, EntityBlueprint, HostileTriggerDef, NpcBehaviorDef, NpcRoutineDef, PatrolStopDef,
    Rgb24,
};

pub const BLUEPRINT_PROP: EntityBlueprint = EntityBlueprint {
    kind: "prop",
    display_name: "Prop",
    description: "Set dressing; no dialogue hook.",
    default_glyph: '*',
    default_fg: Rgb24::new(185, 170, 140),
    default_label: "Crate",
    is_actor: false,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: None,
    world_item: None,
    is_container: false,
    faction_id: "world",
    disposition_to_player: Disposition::Neutral,
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
    default_fg: Rgb24::new(235, 120, 120),
    default_label: "Tonic",
    is_actor: false,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: None,
    world_item: Some("health_tonic"),
    is_container: false,
    faction_id: "world",
    disposition_to_player: Disposition::Neutral,
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
    default_fg: Rgb24::new(230, 210, 120),
    default_label: "Ring",
    is_actor: false,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: None,
    world_item: Some("brass_ring"),
    is_container: false,
    faction_id: "world",
    disposition_to_player: Disposition::Neutral,
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
    default_fg: Rgb24::new(210, 165, 115),
    default_label: "Chest",
    is_actor: false,
    behavior: NpcBehaviorDef::idle(),
    dialogue_id: None,
    world_item: None,
    is_container: true,
    faction_id: "world",
    disposition_to_player: Disposition::Neutral,
    base_max_hp: 20,
    base_strength: 1,
    base_agility: 1,
    base_speed: 1,
};

pub const BLUEPRINT_WOLF: EntityBlueprint = EntityBlueprint {
    kind: "wolf",
    display_name: "Wolf",
    description: "Wildlife enemy that roams near its den and attacks close targets.",
    default_glyph: 'w',
    default_fg: Rgb24::new(190, 190, 190),
    default_label: "Wolf",
    is_actor: true,
    behavior: NpcBehaviorDef {
        routine: NpcRoutineDef::Roam { radius: 5 },
        hostile_trigger: Some(HostileTriggerDef::PlayerWithinChebyshev { range: 4 }),
    },
    dialogue_id: None,
    world_item: None,
    is_container: false,
    faction_id: "wildlife",
    disposition_to_player: Disposition::Hostile,
    base_max_hp: 12,
    base_strength: 5,
    base_agility: 6,
    base_speed: 6,
};

pub static BLUEPRINT_BANDIT_PATROL_STOPS: &[PatrolStopDef] = &[
    PatrolStopDef {
        dx: 0,
        dy: 0,
        wait_ticks: 10,
    },
    PatrolStopDef {
        dx: 4,
        dy: 0,
        wait_ticks: 14,
    },
    PatrolStopDef {
        dx: 4,
        dy: 3,
        wait_ticks: 10,
    },
    PatrolStopDef {
        dx: 0,
        dy: 3,
        wait_ticks: 14,
    },
];

pub const BLUEPRINT_BANDIT: EntityBlueprint = EntityBlueprint {
    kind: "bandit",
    display_name: "Bandit",
    description: "Hostile patrol actor with short loiter times at each waypoint.",
    default_glyph: 'b',
    default_fg: Rgb24::new(205, 185, 165),
    default_label: "Bandit",
    is_actor: true,
    behavior: NpcBehaviorDef {
        routine: NpcRoutineDef::Patrol {
            stops: BLUEPRINT_BANDIT_PATROL_STOPS,
        },
        hostile_trigger: Some(HostileTriggerDef::PlayerWithinChebyshev { range: 5 }),
    },
    dialogue_id: None,
    world_item: None,
    is_container: false,
    faction_id: "bandit",
    disposition_to_player: Disposition::Hostile,
    base_max_hp: 18,
    base_strength: 6,
    base_agility: 5,
    base_speed: 5,
};

pub static ENTITY_BLUEPRINTS: &[EntityBlueprint] = &[
    BLUEPRINT_PROP,
    BLUEPRINT_HEALTH_TONIC_DROP,
    BLUEPRINT_BRASS_RING_DROP,
    BLUEPRINT_WOODEN_CHEST,
    BLUEPRINT_WOLF,
    BLUEPRINT_BANDIT,
];
