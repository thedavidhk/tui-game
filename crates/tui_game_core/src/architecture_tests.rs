#[test]
fn ai_and_magic_do_not_depend_on_render_layer() {
    let ai_mod = include_str!("ai/mod.rs");
    let ai_combat = include_str!("ai/combat.rs");
    let ai_exploration = include_str!("ai/exploration.rs");
    let magic_mod = include_str!("magic/mod.rs");
    let magic_combat = include_str!("magic/combat.rs");
    let magic_exploration = include_str!("magic/exploration.rs");

    for src in [
        ai_mod,
        ai_combat,
        ai_exploration,
        magic_mod,
        magic_combat,
        magic_exploration,
    ] {
        assert!(
            !src.contains("crate::render"),
            "logic-layer seams must not depend on render APIs"
        );
    }
}

#[test]
fn screen_effect_renderer_stays_a_pure_post_pass() {
    // `render::effects` draws self-describing effects over a `FrameBuffer`; deciding *when*
    // an effect is active belongs to `game::effects`. Keep that split so the renderer never
    // reaches back into game state.
    let render_effects = include_str!("render/effects.rs");
    assert!(
        !render_effects.contains("crate::game"),
        "render::effects must not depend on game state (policy lives in game::effects)"
    );
}

#[test]
fn game_content_npc_defs_remain_declarative_tables() {
    let trainer = include_str!("game_content/npcs/trainer.rs");
    assert!(
        !trainer.contains("crate::render"),
        "game content must not depend on UI/render modules"
    );
}
