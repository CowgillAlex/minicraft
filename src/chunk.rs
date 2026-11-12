use crate::world::{AIR, CHUNK};
use std::sync::Arc;

/// Uniform chunks need one material byte; only mixed chunks allocate a voxel array.
#[derive(Clone)]
pub enum Chunk {
    Uniform(u8),
    Dense { cells: Arc<[u8]>, occupied: usize },
}

impl Chunk {
    pub fn from_cells(cells: Vec<u8>) -> Self {
        let material = cells[0];
        if cells.iter().all(|v| *v == material) {
            Self::Uniform(material)
        } else {
            Self::Dense {
                occupied: cells.iter().filter(|v| **v != AIR).count(),
                cells: Arc::from(cells),
            }
        }
    }
    #[inline]
    pub fn get(&self, index: usize) -> u8 {
        match self {
            Self::Uniform(material) => *material,
            Self::Dense { cells, .. } => cells[index],
        }
    }
    pub fn set(&mut self, index: usize, material: u8) {
        if let Self::Uniform(previous) = self {
            if *previous == material {
                return;
            }
            *self = Self::Dense {
                cells: Arc::from(vec![*previous; CHUNK.pow(3)]),
                occupied: if *previous == AIR { 0 } else { CHUNK.pow(3) },
            };
        }
        if let Self::Dense { cells, occupied } = self {
            *occupied += usize::from(cells[index] == AIR && material != AIR);
            *occupied -= usize::from(cells[index] != AIR && material == AIR);
            Arc::make_mut(cells)[index] = material;
        }
    }
    /// Detach a shared page once for a material pass, then restore occupancy and
    /// compact storage. Per-voxel edits keep using `set`.
    pub fn with_cells_mut(&mut self, edit: impl FnOnce(&mut [u8])) {
        if let Self::Uniform(material) = self {
            *self = Self::Dense {
                cells: Arc::from(vec![*material; CHUNK.pow(3)]),
                occupied: 0,
            };
        }
        if let Self::Dense { cells, occupied } = self {
            let cells = Arc::make_mut(cells);
            edit(cells);
            *occupied = cells.iter().filter(|&&m| m != AIR).count();
        }
        self.compact();
    }

    /// Restore compact storage after a bulk material pass.
    pub fn compact(&mut self) {
        if let Self::Dense { cells, .. } = self {
            let material = cells[0];
            if cells.iter().all(|v| *v == material) {
                *self = Self::Uniform(material);
            }
        }
    }
    pub fn occupied(&self) -> usize {
        match self {
            Self::Uniform(AIR) => 0,
            Self::Uniform(_) => CHUNK.pow(3),
            Self::Dense { occupied, .. } => *occupied,
        }
    }
    #[cfg(test)]
    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            Self::Uniform(material) => vec![*material; CHUNK.pow(3)],
            Self::Dense { cells, .. } => cells.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bulk_material_pass_preserves_snapshots_occupancy_and_compaction() {
        let mut page = Chunk::Uniform(crate::world::STONE);
        page.with_cells_mut(|cells| cells[7] = AIR);
        let snapshot = page.clone();
        page.with_cells_mut(|cells| {
            cells[7] = crate::world::DIRT;
            cells[8..12].fill(AIR);
        });
        assert_eq!(page.occupied(), CHUNK.pow(3) - 4);
        assert_eq!(snapshot.occupied(), CHUNK.pow(3) - 1);
        assert_eq!(snapshot.get(7), AIR);
        assert_eq!(snapshot.get(8), crate::world::STONE);
        page.with_cells_mut(|cells| cells.fill(AIR));
        assert!(matches!(page, Chunk::Uniform(AIR)));
        assert_eq!(page.occupied(), 0);
    }

    #[test]
    fn uniform_chunks_expand_on_edit_without_changing_snapshots() {
        let mut world = crate::world::World::with_height(384 * 16);
        world.insert_chunk([0, 100, 0], Chunk::Uniform(crate::world::STONE));
        let snapshot = world.clone();
        let y = 100 * CHUNK as i32 + 4;
        assert_eq!(world.voxels, CHUNK.pow(3));
        world.set(4, y, 4, AIR);
        assert_eq!(world.get(4, y, 4), AIR);
        assert_eq!(snapshot.get(4, y, 4), crate::world::STONE);
        assert_eq!(world.get(5, y, 4), crate::world::STONE);
        assert_eq!(world.voxels, CHUNK.pow(3) - 1);
        assert!(world.edit([4, 6000, 4], crate::world::STONE).is_some());
        assert_eq!(world.get(4, 6000, 4), crate::world::STONE);
    }
}
