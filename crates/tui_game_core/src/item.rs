//! Item definitions, stacks, and player-style inventories.

use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// Equipment slots for UI, persistence, and combat resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    /// Primary weapon (sword, bow, …).
    MainHand,
    OffHand,
    Armor,
    Amulet,
    Ring,
}

impl EquipSlot {
    pub const VARIANTS: [EquipSlot; 5] = [
        EquipSlot::MainHand,
        EquipSlot::OffHand,
        EquipSlot::Armor,
        EquipSlot::Amulet,
        EquipSlot::Ring,
    ];
}

impl Display for EquipSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EquipSlot::MainHand => "Main hand",
                EquipSlot::OffHand => "Off hand",
                EquipSlot::Armor => "Armor",
                EquipSlot::Amulet => "Amulet",
                EquipSlot::Ring => "Ring",
            }
        )
    }
}

/// Where a stack “lives” when tied to equipment or the ranged ammo bandolier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StackEquipped {
    Wear(EquipSlot),
    Quiver,
}

/// Weapon behavior when equipped in [`EquipSlot::MainHand`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    Melee {
        to_hit: i8,
        damage_bonus: i8,
    },
    RangedBow {
        to_hit: i8,
        damage_bonus: i8,
        /// Maximum Chebyshev distance for a shot.
        range: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    Mundane,
    Consumable,
    Equippable(EquipSlot),
    /// Ammunition (arrows); press **e** in inventory to load/unload the quiver ([`StackEquipped::Quiver`]).
    Ammo,
}

/// Read-only view of static item definitions for UI and validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemCatalog {
    defs: &'static [ItemDef],
}

impl ItemCatalog {
    #[must_use]
    pub const fn new(defs: &'static [ItemDef]) -> Self {
        Self { defs }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&'static ItemDef> {
        self.defs.iter().find(|d| d.id == id)
    }

    /// Display name from defs, or the raw `id` if unknown.
    #[must_use]
    pub fn display_name<'a>(&self, id: &'a str) -> &'a str {
        self.get(id).map(|d| d.name).unwrap_or(id)
    }

    /// Short line for detail panes (category / slot hint).
    #[must_use]
    pub fn category_line(&self, id: &str) -> String {
        let Some(d) = self.get(id) else {
            return "Unknown item".into();
        };
        match d.category {
            ItemCategory::Mundane => "Mundane".into(),
            ItemCategory::Consumable => "Consumable".into(),
            ItemCategory::Equippable(slot) => {
                format!("Equippable ({slot:?})")
            }
            ItemCategory::Ammo => "Ammo (e: load quiver)".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub glyph: char,
    pub category: ItemCategory,
    /// When `Some`, this item is a weapon for [`EquipSlot::MainHand`].
    pub weapon: Option<WeaponKind>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: String,
    pub count: u32,
    #[serde(default)]
    pub equipped: Option<StackEquipped>,
}

impl ItemStack {
    pub fn new(id: impl Into<String>, count: u32) -> Self {
        Self {
            id: id.into(),
            count: count.max(1),
            equipped: None,
        }
    }

    #[must_use]
    pub fn loose(id: impl Into<String>, count: u32) -> Self {
        Self::new(id, count)
    }

    #[must_use]
    pub fn worn(id: impl Into<String>, slot: EquipSlot) -> Self {
        Self {
            id: id.into(),
            count: 1,
            equipped: Some(StackEquipped::Wear(slot)),
        }
    }

    #[must_use]
    pub fn quiver(id: impl Into<String>, count: u32) -> Self {
        Self {
            id: id.into(),
            count: count.max(1),
            equipped: Some(StackEquipped::Quiver),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    /// Ordered stacks (merge same id on add).
    pub stacks: Vec<ItemStack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryError {
    NotEnough,
    UnknownItem,
}

impl Inventory {
    pub fn count_of(&self, id: &str) -> u32 {
        self.stacks
            .iter()
            .filter(|s| s.id == id)
            .map(|s| s.count)
            .sum()
    }

    pub fn has(&self, id: &str, n: u32) -> bool {
        self.count_of(id) >= n
    }

    /// Adds loose items, merging into an existing **loose** stack, then the **quiver** stack for
    /// that id (pickups funnel into the quiver when loaded).
    pub fn add(&mut self, id: impl Into<String>, n: u32) {
        if n == 0 {
            return;
        }
        let id = id.into();
        if let Some(s) = self.stacks.iter_mut().find(|s| {
            s.id == id && matches!(s.equipped, Some(StackEquipped::Quiver))
        }) {
            s.count = s.count.saturating_add(n);
            return;
        }
        if let Some(s) = self
            .stacks
            .iter_mut()
            .find(|s| s.id == id && s.equipped.is_none())
        {
            s.count = s.count.saturating_add(n);
            return;
        }
        self.stacks.push(ItemStack {
            id,
            count: n,
            equipped: None,
        });
    }

    /// Removes `n` copies of `id`, consuming **loose** stacks first, then worn/quiver stacks.
    pub fn try_remove(&mut self, id: &str, n: u32) -> Result<(), InventoryError> {
        if n == 0 {
            return Ok(());
        }
        let mut rem = n;
        let mut i = 0;
        while rem > 0 && i < self.stacks.len() {
            if self.stacks[i].id != id {
                i += 1;
                continue;
            }
            if self.stacks[i].equipped.is_some() {
                i += 1;
                continue;
            }
            let c = self.stacks[i].count;
            let take = rem.min(c);
            let left = c - take;
            if left == 0 {
                self.stacks.remove(i);
            } else {
                self.stacks[i].count = left;
                i += 1;
            }
            rem -= take;
        }
        i = 0;
        while rem > 0 && i < self.stacks.len() {
            if self.stacks[i].id != id {
                i += 1;
                continue;
            }
            if self.stacks[i].equipped.is_none() {
                i += 1;
                continue;
            }
            let c = self.stacks[i].count;
            let take = rem.min(c);
            let left = c - take;
            if left == 0 {
                self.stacks.remove(i);
            } else {
                self.stacks[i].count = left;
                i += 1;
            }
            rem -= take;
        }
        if rem > 0 {
            Err(InventoryError::NotEnough)
        } else {
            Ok(())
        }
    }

    /// Collapses all **loose** stacks for `id` into one row (stable order).
    pub fn consolidate_loose(&mut self, id: &str) {
        let mut total = 0u32;
        self.stacks.retain(|s| {
            if s.id == id && s.equipped.is_none() {
                total = total.saturating_add(s.count);
                false
            } else {
                true
            }
        });
        if total > 0 {
            self.stacks.push(ItemStack::loose(id.to_string(), total));
        }
    }

    /// Move up to `n` items from `other` into `self`.
    pub fn transfer_from(
        &mut self,
        other: &mut Inventory,
        id: &str,
        n: u32,
    ) -> Result<(), InventoryError> {
        other.try_remove(id, n)?;
        self.add(id.to_string(), n);
        Ok(())
    }

    /// Move the entire stack at `idx` from `from` into `to` (preserves equipped / quiver flags).
    pub fn try_move_stack_index(
        from: &mut Inventory,
        to: &mut Inventory,
        idx: usize,
    ) -> Result<(), InventoryError> {
        let stack = from
            .stacks
            .get(idx)
            .cloned()
            .ok_or(InventoryError::UnknownItem)?;
        from.stacks.remove(idx);
        to.absorb_stack(stack);
        Ok(())
    }

    /// Inserts `stack`, merging into quiver or loose rows as appropriate.
    pub fn absorb_stack(&mut self, stack: ItemStack) {
        match stack.equipped {
            Some(StackEquipped::Quiver) => {
                if let Some(s) = self.stacks.iter_mut().find(|t| {
                    t.id == stack.id && matches!(t.equipped, Some(StackEquipped::Quiver))
                }) {
                    s.count = s.count.saturating_add(stack.count);
                    return;
                }
                self.stacks.push(stack);
            }
            Some(StackEquipped::Wear(_)) => {
                self.stacks.push(stack);
            }
            None => {
                self.add(stack.id.clone(), stack.count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Inventory, InventoryError, ItemCatalog, ItemCategory, ItemDef};

    #[test]
    fn item_catalog_display_name_falls_back_to_id() {
        static DEFS: &[ItemDef] = &[ItemDef {
            id: "a",
            name: "Apple",
            description: "d",
            glyph: 'a',
            category: ItemCategory::Mundane,
            weapon: None,
        }];
        let c = ItemCatalog::new(DEFS);
        assert_eq!(c.display_name("a"), "Apple");
        assert_eq!(c.display_name("missing"), "missing");
    }

    #[test]
    fn try_remove_not_enough() {
        let mut inv = Inventory::default();
        inv.add("a", 1);
        assert_eq!(inv.try_remove("a", 2), Err(InventoryError::NotEnough));
    }

    #[test]
    fn try_move_stack_index_round_trip() {
        let mut a = Inventory::default();
        let mut b = Inventory::default();
        a.add("k", 3);
        a.add("x", 1);
        Inventory::try_move_stack_index(&mut a, &mut b, 0).unwrap();
        assert_eq!(a.count_of("k"), 0);
        assert_eq!(b.count_of("k"), 3);
        assert_eq!(a.count_of("x"), 1);
        Inventory::try_move_stack_index(&mut b, &mut a, 0).unwrap();
        assert_eq!(a.count_of("k"), 3);
    }
}
