use crate::chunk::Chunk;
use glam::Vec3;
#[path = "world_read.rs"]
mod read;
pub use read::Reader;
#[path = "door_world.rs"]
mod door_world;
pub use door_world::DoorAction;
#[path = "world_village.rs"]
mod village;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub const CHUNK: usize = 32;
pub const NX: usize = 20;
pub const NY: usize = 6;
pub const NZ: usize = 20;
pub const WIDTH: i32 = (NX * CHUNK) as i32;
pub const DEPTH: i32 = (NZ * CHUNK) as i32;
pub const HEIGHT: i32 = (NY * CHUNK) as i32;
pub const VOXEL: f32 = 1.0 / 16.0;
pub use crate::materials::*;

pub fn opaque(material: u8) -> bool {
    material != AIR && material != GLASS && material != WATER
}

#[derive(Clone)]
pub struct World {
    chunks: HashMap<[i32; 3], crate::chunk::Chunk>,
    pub infinite: bool,
    pub spawn: Option<Vec3>,
    edits: HashMap<[i32; 3], HashMap<usize, u8>>,
    height: i32,
    pub max_occupied_y: i32,
    door_chunks: HashMap<[i32; 3], Arc<[u8]>>,
    pub doors: Vec<crate::doors::Door>,
    loose_handles: HashSet<[i32; 3]>,
    pub emitters: HashSet<[i32; 3]>,
    pub voxels: usize,
    pub noise_preview: Option<crate::worldgen::preview::Preview>,
}
impl World {
    pub fn empty() -> Self {
        Self::with_height(HEIGHT)
    }
    pub fn with_height(height: i32) -> Self {
        assert!(height > 0 && height % CHUNK as i32 == 0);
        Self {
            chunks: HashMap::new(),
            infinite: false,
            spawn: None,
            edits: HashMap::new(),
            height,
            max_occupied_y: 0,
            door_chunks: HashMap::new(),
            doors: Vec::new(),
            loose_handles: HashSet::new(),
            voxels: 0,
            emitters: HashSet::new(),
            noise_preview: None,
        }
    }
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(self)
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn chunks_y(&self) -> usize {
        self.height as usize / CHUNK
    }
    pub fn voxel_storage_bytes(&self) -> usize {
        self.chunks
            .values()
            .map(|chunk| match chunk {
                crate::chunk::Chunk::Uniform(_) => 1,
                crate::chunk::Chunk::Dense { cells, .. } => cells.len(),
            })
            .sum()
    }
    pub(crate) fn insert_chunk(&mut self, chunk: [usize; 3], data: crate::chunk::Chunk) {
        self.insert_page(chunk.map(|v| v as i32), data);
    }
    pub(crate) fn insert_page(&mut self, key: [i32; 3], mut data: crate::chunk::Chunk) {
        if let Some(edits) = self.edits.get(&key) {
            for (&i, &material) in edits {
                data.set(i, material);
            }
        }
        self.voxels -= self.chunks.get(&key).map_or(0, |c| c.occupied());
        let count = data.occupied();
        self.voxels += count;
        if count > 0 {
            self.max_occupied_y = self.max_occupied_y.max((key[1] + 1) * CHUNK as i32);
        }
        if self.infinite || count > 0 {
            self.chunks.insert(key, data);
        } else {
            self.chunks.remove(&key);
        }
    }
    pub fn contains(&self, cell: [i32; 3]) -> bool {
        (0..self.height).contains(&cell[1])
            && if self.infinite {
                let origin = self.noise_preview.map_or([0; 3], |p| p.origin);
                [0, 2]
                    .into_iter()
                    .all(|i| (i64::from(cell[i]) + i64::from(origin[i]) * 16).abs() <= 480_000_000)
            } else {
                (0..WIDTH).contains(&cell[0]) && (0..DEPTH).contains(&cell[2])
            }
    }
    fn address(&self, x: i32, y: i32, z: i32) -> Option<([i32; 3], usize)> {
        self.contains([x, y, z]).then(|| {
            (
                [x, y, z].map(|v| v.div_euclid(CHUNK as i32)),
                x.rem_euclid(CHUNK as i32) as usize
                    + CHUNK
                        * (z.rem_euclid(CHUNK as i32) as usize
                            + CHUNK * y.rem_euclid(CHUNK as i32) as usize),
            )
        })
    }
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        let Some((key, index)) = self.address(x, y, z) else {
            return AIR;
        };
        let material = self.chunks.get(&key).map_or(AIR, |c| c.get(index));
        if material != AIR {
            material
        } else {
            self.door_chunks.get(&key).map_or(AIR, |c| c[index])
        }
    }
    pub fn get_static(&self, x: i32, y: i32, z: i32) -> u8 {
        self.address(x, y, z)
            .and_then(|(key, index)| self.chunks.get(&key).map(|c| c.get(index)))
            .unwrap_or(AIR)
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, material: u8) {
        if let Some((key, index)) = self.address(x, y, z) {
            if material == AIR && !self.infinite && !self.chunks.contains_key(&key) {
                return;
            }
            let data = self
                .chunks
                .entry(key)
                .or_insert(crate::chunk::Chunk::Uniform(AIR));
            let previous = data.get(index);
            self.voxels += usize::from(previous == AIR && material != AIR);
            self.voxels -= usize::from(previous != AIR && material == AIR);
            data.set(index, material);
            if self.infinite {
                self.edits.entry(key).or_default().insert(index, material);
            }
            if material != AIR {
                self.max_occupied_y = self.max_occupied_y.max(y + 1);
            }
            if previous == LIGHT && material != LIGHT {
                self.emitters.remove(&[x, y, z]);
            }
            if material == LIGHT {
                self.emitters.insert([x, y, z]);
            }
            if previous == HANDLE && material != HANDLE {
                self.loose_handles.remove(&[x, y, z]);
            }
            if material == HANDLE {
                self.loose_handles.insert([x, y, z]);
            }
        }
    }
    pub fn chunk_exists(&self, x: usize, y: usize, z: usize) -> bool {
        self.has_geometry([x as i32, y as i32, z as i32])
    }
    pub fn has_geometry(&self, key: [i32; 3]) -> bool {
        self.chunks.get(&key).is_some_and(|c| c.occupied() > 0)
    }
    pub fn is_loaded(&self, cell: [i32; 3]) -> bool {
        !self.infinite
            || self
                .chunks
                .contains_key(&cell.map(|v| v.div_euclid(CHUNK as i32)))
    }
    pub fn has_overlay(&self, key: [i32; 3]) -> bool {
        self.door_chunks.contains_key(&key)
    }
    pub fn page(&self, key: [i32; 3]) -> Option<&crate::chunk::Chunk> {
        self.chunks.get(&key)
    }
    pub fn pages(&self) -> impl Iterator<Item = ([i32; 3], &crate::chunk::Chunk)> {
        self.chunks.iter().map(|(k, v)| (*k, v))
    }
    pub fn unload(&mut self, key: [i32; 3]) {
        if let Some(c) = self.chunks.remove(&key) {
            self.voxels -= c.occupied();
        }
    }
    pub fn copy_edits_from(&mut self, source: &World) {
        self.edits = source.edits.clone();
        self.doors = source.doors.clone();
        self.door_chunks = source.door_chunks.clone();
        self.emitters = source.emitters.clone();
        self.loose_handles = source.loose_handles.clone();
        for (key, edits) in &self.edits {
            if let Some(page) = self.chunks.get_mut(key) {
                self.voxels -= page.occupied();
                for (&index, &material) in edits {
                    page.set(index, material);
                }
                self.voxels += page.occupied();
            }
        }
    }
    pub fn has_changes(&self, min: [i32; 3], max: [i32; 3]) -> bool {
        let lo = min.map(|v| v.div_euclid(32));
        let hi = max.map(|v| v.div_euclid(32));
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let key = [x, y, z];
                    if self.edits.contains_key(&key) || self.door_chunks.contains_key(&key) {
                        return true;
                    }
                }
            }
        }
        false
    }
    pub fn override_at(&self, p: [i32; 3]) -> Option<u8> {
        let (key, index) = self.address(p[0], p[1], p[2])?;
        let edit = self.edits.get(&key).and_then(|e| e.get(&index)).copied();
        if edit.is_some_and(|m| m != AIR) {
            return edit;
        }
        self.door_chunks
            .get(&key)
            .and_then(|c| (c[index] != AIR).then_some(c[index]))
            .or(edit)
    }
    pub fn column_overrides(&self, x: i32, z: i32) -> std::collections::BTreeMap<i32, u8> {
        let mut values = std::collections::BTreeMap::new();
        let cx = x.div_euclid(CHUNK as i32);
        let cz = z.div_euclid(CHUNK as i32);
        let horizontal =
            x.rem_euclid(CHUNK as i32) as usize + CHUNK * z.rem_euclid(CHUNK as i32) as usize;
        for (key, edits) in &self.edits {
            if key[0] == cx && key[2] == cz {
                for (&index, &material) in edits {
                    if index % (CHUNK * CHUNK) == horizontal {
                        values.insert(
                            key[1] * CHUNK as i32 + (index / (CHUNK * CHUNK)) as i32,
                            material,
                        );
                    }
                }
            }
        }
        for (key, cells) in &self.door_chunks {
            if key[0] == cx && key[2] == cz {
                for y in 0..CHUNK {
                    let m = cells[horizontal + y * CHUNK * CHUNK];
                    if m != AIR {
                        values.insert(key[1] * CHUNK as i32 + y as i32, m);
                    }
                }
            }
        }
        values
    }
    pub fn buried_chunk(&self, chunk: [i32; 3]) -> bool {
        let Some(Chunk::Uniform(material)) = self.chunks.get(&chunk) else {
            return false;
        };
        if *material == AIR {
            return true;
        }
        let blocked = |key| matches!(self.chunks.get(&key),Some(Chunk::Uniform(other)) if opaque(*other) || (*material==WATER && *other==WATER) || (*material==GLASS && *other==GLASS));
        [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ]
        .into_iter()
        .all(|d| blocked(std::array::from_fn(|i| chunk[i] + d[i])))
    }
    pub fn meters(x: i32, y: i32, z: i32) -> Vec3 {
        Vec3::new((x - WIDTH / 2) as f32, y as f32, (z - DEPTH / 2) as f32) * VOXEL
    }
}

pub fn hash(x: i32, y: i32, z: i32) -> u32 {
    let mut v = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ (z as u32).wrapping_mul(2246822519);
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    v ^ (v >> 16)
}
pub fn terrain_height(x: i32, z: i32) -> i32 {
    22 + ((x as f32 * 0.017).sin() * 4.0
        + (z as f32 * 0.022).cos() * 3.0
        + ((x + z) as f32 * 0.009).sin() * 5.0) as i32
}
fn road_center(x: i32) -> i32 {
    326 + (x as f32 * 0.013).sin().mul_add(22.0, 0.0) as i32
}
fn lake_distance(x: i32, z: i32) -> f32 {
    ((x - 460) as f32 / 105.0).powi(2) + ((z - 155) as f32 / 69.0).powi(2)
}
pub fn color(material: u8) -> [f32; 3] {
    crate::materials::get(material).color
}

#[cfg(test)]
mod lighting_world_tests {
    use super::*;
    #[test]
    fn generated_houses_have_glass_and_interior_lamps() {
        let mut world = World::empty();
        world.cabin(348, 445);
        let base = terrain_height(348, 445) + 2;
        assert_eq!(world.get(348 - 18, base + 24, 445 + 28), GLASS);
        assert_eq!(world.get(348, base + 39, 445), LIGHT);
        assert!(world.emitters.contains(&[348, base + 39, 445]));
        assert_eq!(world.get(348, base + 2, 445), AIR);
    }
}
