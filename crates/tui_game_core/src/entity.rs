use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

/// SoA-style stores indexed by `EntityId.0` (dense for small games).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EntityArena {
    pub next_id: u32,
    pub alive: Vec<bool>,
    pub position: Vec<Option<GridPos>>,
    pub blocks_movement: Vec<bool>,
    pub glyph: Vec<char>,
    pub name: Vec<String>,
    /// If `Some`, entity is interactable as NPC with this content id string.
    pub npc_kind: Vec<Option<String>>,
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
        name: String,
        blocks_movement: bool,
        npc_kind: Option<String>,
    ) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let i = id.0 as usize;
        self.extend(i);
        self.alive[i] = true;
        self.position[i] = Some(pos);
        self.blocks_movement[i] = blocks_movement;
        self.glyph[i] = glyph;
        self.name[i] = name;
        self.npc_kind[i] = npc_kind;
        id
    }

    fn extend(&mut self, i: usize) {
        while self.alive.len() <= i {
            self.alive.push(false);
            self.position.push(None);
            self.blocks_movement.push(false);
            self.glyph.push('?');
            self.name.push(String::new());
            self.npc_kind.push(None);
        }
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
        for (i, alive) in self.alive.iter().enumerate() {
            if !alive {
                continue;
            }
            if let Some(p) = self.position[i] {
                if p.x == x && p.y == y {
                    return Some(EntityId(i as u32));
                }
            }
        }
        None
    }

    pub fn can_move_to(&self, map_blocked: bool, dest: GridPos, ignore: Option<EntityId>) -> bool {
        if map_blocked {
            return false;
        }
        if let Some(oid) = self.occupant_at(dest.x, dest.y) {
            if Some(oid) == ignore {
                return true;
            }
            if self.blocks_movement[oid.0 as usize] {
                return false;
            }
        }
        true
    }
}
