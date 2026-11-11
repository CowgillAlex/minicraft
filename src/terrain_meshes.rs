//! A byte-bounded source mesh cache avoids repeating exact meshing for each LOD.
use crate::mesh::Mesh;
use std::{collections::HashMap, sync::Arc};
const BUDGET: usize = 64 * 1024 * 1024;
#[derive(Default)]
pub struct Cache {
    entries: HashMap<[i32; 3], (Arc<Mesh>, u64, usize)>,
    clock: u64,
    bytes: usize,
}
impl Cache {
    pub fn get(&mut self, key: [i32; 3]) -> Option<Arc<Mesh>> {
        self.clock = self.clock.wrapping_add(1);
        let (mesh, used, _) = self.entries.get_mut(&key)?;
        *used = self.clock;
        Some(mesh.clone())
    }
    pub fn insert(&mut self, key: [i32; 3], mesh: Arc<Mesh>) {
        let size = mesh.vertices.capacity() * std::mem::size_of::<crate::mesh::Vertex>()
            + (mesh.indices.capacity() + mesh.transparent_indices.capacity()) * 4;
        if size > BUDGET {
            return;
        }
        if let Some((_, _, old)) = self.entries.remove(&key) {
            self.bytes -= old;
        }
        while self.bytes + size > BUDGET || self.entries.len() >= 4096 {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used, _))| *used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.bytes -= self.entries.remove(&key).unwrap().2;
        }
        self.clock = self.clock.wrapping_add(1);
        self.bytes += size;
        self.entries.insert(key, (mesh, self.clock, size));
    }
    pub fn invalidate(&mut self, cells: &[[i32; 3]]) {
        for cell in cells {
            let lo = cell.map(|v| (v - 1).div_euclid(32));
            let hi = cell.map(|v| (v + 1).div_euclid(32));
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        if let Some((_, _, size)) = self.entries.remove(&[x, y, z]) {
                            self.bytes -= size;
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
#[test]
fn edits_invalidate_both_sides_of_a_page_border() {
    let mut cache = Cache::default();
    let mut world = crate::world::World::empty();
    world.set(31, 1, 1, crate::world::STONE);
    for key in [[0, 0, 0], [1, 0, 0], [4, 0, 0]] {
        cache.insert(key, Arc::new(Mesh::build_chunk(&world, key)));
    }
    cache.invalidate(&[[31, 1, 1]]);
    assert!(cache.get([0, 0, 0]).is_none());
    assert!(cache.get([1, 0, 0]).is_none());
    assert!(cache.get([4, 0, 0]).is_some());
}
