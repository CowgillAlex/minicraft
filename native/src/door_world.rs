use super::*;
use crate::{
    doors::{Door, Status, SIZE},
    physics::Aabb,
};

pub struct Edit {
    pub terrain: Vec<[i32; 3]>,
    pub light: Vec<[i32; 3]>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoorAction {
    Changed,
    FrameBroken,
    HandleMissing,
    Blocked,
}
impl World {
    pub fn door_at(&self, cell: [i32; 3]) -> Option<(usize, [i32; 3])> {
        self.doors
            .iter()
            .enumerate()
            .find_map(|(id, door)| door.local_cell(cell).map(|p| (id, p)))
    }
    fn overlay_set(&mut self, cell: [i32; 3], material: u8) {
        let Some((chunk, index)) = self.address(cell[0], cell[1], cell[2]) else {
            return;
        };
        if material == AIR && !self.door_chunks.contains_key(&chunk) {
            return;
        }
        let data = self
            .door_chunks
            .entry(chunk)
            .or_insert_with(|| Arc::from(vec![0; CHUNK.pow(3)]));
        let data = Arc::make_mut(data);
        let old = data[index];
        self.voxels += usize::from(old == AIR && material != AIR);
        self.voxels -= usize::from(old != AIR && material == AIR);
        data[index] = material;
        if old == LIGHT && material != LIGHT {
            self.emitters.remove(&cell);
        }
        if material == LIGHT {
            self.emitters.insert(cell);
        }
    }
    pub fn install_door(&mut self, door: Door) -> Vec<[i32; 3]> {
        let mut cells = Vec::new();
        for z in 0..SIZE[2] {
            for y in 0..SIZE[1] {
                for x in 0..SIZE[0] {
                    let cell = door.closed_cell([x, y, z]);
                    if self.get_static(cell[0], cell[1], cell[2]) != AIR {
                        self.set(cell[0], cell[1], cell[2], AIR);
                        cells.push(cell);
                    }
                }
            }
        }
        for (cell, material) in door.occupied() {
            self.overlay_set(cell, material);
        }
        self.doors.push(door);
        cells
    }
    pub fn edit(&mut self, cell: [i32; 3], material: u8) -> Option<Edit> {
        if !self.contains(cell) || self.get(cell[0], cell[1], cell[2]) == material {
            return None;
        }
        if let Some((id, local)) = self
            .door_at(cell)
            .filter(|_| self.get_static(cell[0], cell[1], cell[2]) == AIR)
        {
            self.doors[id].set(local, material);
            self.overlay_set(cell, material);
            return Some(Edit {
                terrain: Vec::new(),
                light: vec![cell],
            });
        }
        self.set(cell[0], cell[1], cell[2], material);
        let mut terrain = vec![cell];
        let handles: Vec<_> = self
            .loose_handles
            .iter()
            .copied()
            .filter(|p| (0..3).all(|axis| (p[axis] - cell[axis]).abs() <= 48))
            .collect();
        for handle in handles {
            if !self.loose_handles.contains(&handle) {
                continue;
            }
            if let Some(door) = crate::doors::recognition::recognize(self, handle) {
                terrain.extend(self.install_door(door));
            }
        }
        Some(Edit {
            terrain,
            light: vec![cell],
        })
    }
    pub fn toggle_door(
        &mut self,
        id: usize,
        player: Aabb,
        rendered_player: Aabb,
    ) -> (DoorAction, Vec<[i32; 3]>) {
        let door = &self.doors[id];
        match door.status() {
            Status::FrameBroken => return (DoorAction::FrameBroken, Vec::new()),
            Status::HandleMissing => return (DoorAction::HandleMissing, Vec::new()),
            Status::Ready => {}
        }
        let mut next = door.clone();
        next.open = !next.open;
        if next.blocked_by(player)
            || next.blocked_by(rendered_player)
            || next.occupied().any(|(p, _)| {
                !self.contains(p)
                    || self.get_static(p[0], p[1], p[2]) != AIR
                    || self.door_at(p).is_some_and(|(other, _)| other != id)
            })
        {
            return (DoorAction::Blocked, Vec::new());
        }
        let old: Vec<_> = door.occupied().map(|(p, _)| p).collect();
        let mut changed = old.clone();
        for cell in old {
            self.overlay_set(cell, AIR);
        }
        let occupied: Vec<_> = next.occupied().collect();
        for (cell, material) in occupied {
            self.overlay_set(cell, material);
            changed.push(cell);
        }
        self.doors[id] = next;
        changed.sort_unstable();
        changed.dedup();
        (DoorAction::Changed, changed)
    }
}
