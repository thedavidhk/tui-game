//! Map glyphs for actors. Stick to BMP symbols that render reliably in monospace terminals.

/// Player (classic roguelike).
pub const PLAYER: char = '@';

/// Friendly humanoid NPC — white chess pawn reads as a standing figure.
pub const HUMANOID_FRIENDLY: char = '♟';

/// Hostile humanoid NPC — black chess pawn pairs with [`HUMANOID_FRIENDLY`].
pub const HUMANOID_HOSTILE: char = '♙';
