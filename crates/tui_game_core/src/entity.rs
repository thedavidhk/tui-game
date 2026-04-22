use serde::{Deserialize, Serialize};

use crate::item::ItemStack;
use crate::render::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcBrainState {
    pub home: GridPos,
    pub roam_goal: Option<GridPos>,
    pub patrol_next_stop: u16,
    pub patrol_wait_ticks: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorStats {
    pub hp: u16,
    pub max_hp: u16,
    pub strength: u16,
    pub agility: u16,
    pub speed: u16,
}

impl ActorStats {
    pub const fn from_full(hp: u16, max_hp: u16, strength: u16, agility: u16, speed: u16) -> Self {
        Self {
            hp,
            max_hp,
            strength,
            agility,
            speed,
        }
    }
}

impl Default for ActorStats {
    fn default() -> Self {
        // Conservative baseline for non-combat props.
        Self::from_full(1, 1, 1, 1, 1)
    }
}

/// SoA-style stores indexed by `EntityId.0` (dense for small games).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EntityArena {
    pub next_id: u32,
    pub alive: Vec<bool>,
    pub position: Vec<Option<GridPos>>,
    pub blocks_movement: Vec<bool>,
    pub glyph: Vec<char>,
    #[serde(default)]
    pub fg: Vec<Color>,
    pub name: Vec<String>,
    /// If `Some`, entity is interactable as NPC with this content id string.
    pub npc_kind: Vec<Option<String>>,
    /// Ground pickup when `npc_kind` is `None` (world item entity).
    pub item: Vec<Option<ItemStack>>,
    /// Opens `ItemTransfer` when adjacent interact in exploration.
    pub is_container: Vec<bool>,
    #[serde(default)]
    pub combat_stats: Vec<ActorStats>,
    #[serde(default)]
    pub npc_brain: Vec<NpcBrainState>,
    /// Player entity always id 0 after bootstrap if used.
    pub player: Option<EntityId>,
}

impl EntityArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(
        &mut self,
        pos: GridPos,
        glyph: char,
        fg: Color,
        name: String,
        blocks_movement: bool,
        npc_kind: Option<String>,
        item: Option<ItemStack>,
        is_container: bool,
    ) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let i = id.0 as usize;
        self.extend(i);
        self.alive[i] = true;
        self.position[i] = Some(pos);
        self.blocks_movement[i] = blocks_movement;
        self.glyph[i] = glyph;
        self.fg[i] = fg;
        self.name[i] = name;
        self.npc_kind[i] = npc_kind;
        self.item[i] = item;
        self.is_container[i] = is_container;
        self.npc_brain[i] = NpcBrainState {
            home: pos,
            roam_goal: None,
            patrol_next_stop: 0,
            patrol_wait_ticks: 0,
        };
        id
    }

    fn extend(&mut self, i: usize) {
        while self.alive.len() <= i {
            self.alive.push(false);
            self.position.push(None);
            self.blocks_movement.push(false);
            self.glyph.push('?');
            self.fg.push(Color::default());
            self.name.push(String::new());
            self.npc_kind.push(None);
            self.item.push(None);
            self.is_container.push(false);
            self.combat_stats.push(ActorStats::default());
            self.npc_brain.push(NpcBrainState::default());
        }
    }

    /// All live entities at `(x, y)` in arbitrary order.
    pub fn occupants_at(&self, x: i32, y: i32) -> Vec<EntityId> {
        let mut out = Vec::new();
        for (i, alive) in self.alive.iter().enumerate() {
            if !alive {
                continue;
            }
            if let Some(p) = self.position[i] {
                if p.x == x && p.y == y {
                    out.push(EntityId(i as u32));
                }
            }
        }
        out
    }

    pub fn despawn(&mut self, id: EntityId) {
        let i = id.0 as usize;
        if i >= self.alive.len() {
            return;
        }
        self.alive[i] = false;
        self.position[i] = None;
        self.blocks_movement[i] = false;
        self.npc_kind[i] = None;
        self.item[i] = None;
        self.is_container[i] = false;
        self.glyph[i] = '?';
        self.fg[i] = Color::default();
        self.name[i].clear();
        self.combat_stats[i] = ActorStats::default();
        self.npc_brain[i] = NpcBrainState::default();
    }

    pub fn set_player(&mut self, id: EntityId) {
        self.player = Some(id);
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        self.alive.get(id.0 as usize).copied().unwrap_or(false)
    }

    pub fn pos(&self, id: EntityId) -> Option<GridPos> {
        self.position.get(id.0 as usize).and_then(|p| *p)
    }

    pub fn set_pos(&mut self, id: EntityId, pos: GridPos) {
        if let Some(p) = self.position.get_mut(id.0 as usize) {
            *p = Some(pos);
        }
    }

    pub fn occupant_at(&self, x: i32, y: i32) -> Option<EntityId> {
        self.occupants_at(x, y).into_iter().next()
    }

    pub fn first_npc_at(&self, x: i32, y: i32) -> Option<EntityId> {
        self.occupants_at(x, y)
            .into_iter()
            .find(|&e| self.npc_kind[e.0 as usize].is_some())
    }

    pub fn first_container_at(&self, x: i32, y: i32) -> Option<EntityId> {
        self.occupants_at(x, y)
            .into_iter()
            .find(|&e| self.is_container[e.0 as usize])
    }

    pub fn can_move_to(&self, map_blocked: bool, dest: GridPos, ignore: Option<EntityId>) -> bool {
        if map_blocked {
            return false;
        }
        for oid in self.occupants_at(dest.x, dest.y) {
            if Some(oid) == ignore {
                continue;
            }
            if self.blocks_movement[oid.0 as usize] {
                return false;
            }
        }
        true
    }

    pub fn set_stats(&mut self, id: EntityId, stats: ActorStats) {
        if let Some(slot) = self.combat_stats.get_mut(id.0 as usize) {
            *slot = stats;
        }
    }

    pub fn stats(&self, id: EntityId) -> Option<ActorStats> {
        self.combat_stats.get(id.0 as usize).copied()
    }

    pub fn stats_mut(&mut self, id: EntityId) -> Option<&mut ActorStats> {
        self.combat_stats.get_mut(id.0 as usize)
    }
}
