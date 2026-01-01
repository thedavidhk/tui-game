//! Small RGB presets for editor pickers (truecolor; matches `FrameBuffer` / ANSI encoder).

use crate::render::Color;

/// Sixteen saturated / pastel picks: easy in the TUI, stored on disk as full RGB.
pub const PRESET_COLORS: [Color; 16] = [
    Color::rgb(220, 220, 210),
    Color::rgb(180, 60, 55),
    Color::rgb(90, 170, 95),
    Color::rgb(70, 130, 220),
    Color::rgb(220, 180, 60),
    Color::rgb(170, 90, 200),
    Color::rgb(60, 200, 200),
    Color::rgb(200, 120, 70),
    Color::rgb(120, 120, 130),
    Color::rgb(40, 40, 48),
    Color::rgb(255, 140, 180),
    Color::rgb(100, 200, 140),
    Color::rgb(255, 255, 120),
    Color::rgb(160, 160, 255),
    Color::rgb(200, 200, 255),
    Color::rgb(255, 200, 120),
];
