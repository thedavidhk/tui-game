//! Map and inventory glyphs for actors and items (not terrain tiles).
//! BMP symbols that render reliably in monospace terminals.

// --- Player ---

/// Player (classic roguelike).
pub const PLAYER: char = '@';

// --- Humanoids ---

/// Friendly humanoid NPC — white chess pawn reads as a standing figure.
pub const HUMANOID_FRIENDLY: char = '♙';

/// Hostile humanoid NPC — black chess pawn pairs with [`HUMANOID_FRIENDLY`].
pub const HUMANOID_HOSTILE: char = '♟';

// --- Wildlife ---

/// Wolf and other hostile quadruped predators.
pub const WOLF: char = '♞';

/// Deer and other skittish herbivores.
pub const DEER: char = '♘';

// --- World objects ---

/// Generic prop / set dressing.
pub const PROP: char = '*';

/// Storage container (chest).
pub const CHEST: char = '□';

// --- Items (by category; world pickups reuse the same glyph) ---

/// Keys and lockpicks.
pub const ITEM_KEY: char = '◎';

/// Potions, tonics, food, and other consumables.
pub const ITEM_CONSUMABLE: char = '!';

/// Rings and other finger jewelry.
pub const ITEM_RING: char = '○';

/// Swords, daggers, clubs, and other melee weapons.
pub const ITEM_MELEE: char = '/';

/// Bows and similar ranged weapons.
pub const ITEM_BOW: char = ')';

/// Arrows, bolts, and other ammunition.
pub const ITEM_AMMO: char = '^';

/// Mundane carryables that are not keys (notes, tokens, …).
pub const ITEM_MUNDANE: char = '‽';
