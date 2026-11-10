use crate::{
    chunk::Chunk,
    light_field::LightField,
    mesh::Mesh,
    world::World,
    worldgen::{preview::Preview, sky::Sky, Generator, TerrainSampler},
};
use glam::Vec3;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub type Key = [i32; 3];
pub const METRES: f32 = 8.0;
pub const CELLS: i32 = 128;
pub const DATA_BUDGET: usize = 512 * 1024 * 1024;
pub const MESH_BUDGET: usize = 512 * 1024 * 1024;
pub fn key(position: Vec3) -> Key {
    crate::ray::grid_position(position)
        .floor()
        .as_ivec3()
        .to_array()
        .map(|v| v.div_euclid(CELLS))
}
pub fn center(key: Key) -> Vec3 {
    World::meters(
        key[0] * CELLS + CELLS / 2,
        key[1] * CELLS + CELLS / 2,
        key[2] * CELLS + CELLS / 2,
    )
}
pub fn page_region(key: Key) -> Key {
    key.map(|v| v.div_euclid(4))
}
pub fn visible(key: Key, position: Vec3, radius: f32) -> bool {
    center(key).distance(position) <= radius + METRES * 3.0_f32.sqrt() / 2.0
}
pub fn candidates(world: &World, position: Vec3, radius: f32) -> Vec<Key> {
    let c = key(position);
    let r = (radius / METRES).ceil() as i32 + 1;
    let mut keys = Vec::new();
    for y in (c[1] - r).max(0)..=(c[1] + r).min((world.height() - 1) / CELLS) {
        for z in c[2] - r..=c[2] + r {
            for x in c[0] - r..=c[0] + r {
                let key = [x, y, z];
                if visible(key, position, radius)
                    && world.contains([x * CELLS, y * CELLS, z * CELLS])
                {
                    keys.push(key);
                }
            }
        }
    }
    keys.sort_by(|a, b| {
        center(*a)
            .distance_squared(position)
            .total_cmp(&center(*b).distance_squared(position))
    });
    keys
}
pub struct Region {
    pub key: Key,
    pub revision: u64,
    pub pages: Vec<(Key, Chunk)>,
    pub evicted: Vec<Key>,
    pub meshes: Vec<(Key, Arc<Mesh>)>,
    pub light: Option<Arc<[u8]>>,
    pub bytes: usize,
}
struct CachedMesh {
    meshes: Vec<(Key, Arc<Mesh>)>,
    gpu: usize,
    cpu: usize,
    used: u64,
}
pub struct Builder<'a> {
    pub world: World,
    sampler: TerrainSampler<'a>,
    sky: Sky,
    published: HashSet<Key>,
    lights: HashMap<Key, (Arc<[u8]>, u64)>,
    clock: u64,
    meshes: HashMap<Key, CachedMesh>,
    mesh_bytes: usize,
}
impl<'a> Builder<'a> {
    pub fn new(world: World, generator: &'a Generator, sky: Sky) -> Self {
        let published = world.pages().map(|(key, _)| key).collect();
        Self {
            world,
            sampler: generator.sampler(),
            sky,
            published,
            lights: HashMap::new(),
            clock: 0,
            meshes: HashMap::new(),
            mesh_bytes: 0,
        }
    }
    pub fn into_parts(self) -> (World, Sky) {
        (self.world, self.sky)
    }
    pub fn edit(&mut self, snapshot: &World, cells: &[[i32; 3]]) {
        self.world.copy_edits_from(snapshot);
        self.sky.invalidate(cells);
        if !cells.is_empty() {
            let min =
                std::array::from_fn::<_, 3, _>(|i| cells.iter().map(|p| p[i]).min().unwrap() - 128);
            let max =
                std::array::from_fn::<_, 3, _>(|i| cells.iter().map(|p| p[i]).max().unwrap() + 128);
            self.lights.retain(|key, _| {
                let p = key.map(|v| v * 128);
                !(p[0] <= max[0]
                    && p[0] + 128 >= min[0]
                    && p[2] <= max[2]
                    && p[2] + 128 >= min[2]
                    && p[1] <= max[1])
            });
            self.meshes.retain(|key, _| {
                let p = key.map(|v| v * 128);
                !(p[0] <= max[0]
                    && p[0] + 128 >= min[0]
                    && p[2] <= max[2]
                    && p[2] + 128 >= min[2]
                    && p[1] <= max[1])
            });
            self.mesh_bytes = self.meshes.values().map(|m| m.cpu).sum();
        }
    }
    pub fn has_near_data(&self, key: Key) -> bool {
        (0..4).all(|y| {
            (0..4).all(|z| {
                (0..4).all(|x| {
                    self.published
                        .contains(&[key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z])
                        && self
                            .world
                            .page([key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z])
                            .is_some()
                })
            })
        })
    }
    pub fn build(
        &mut self,
        key: Key,
        revision: u64,
        focus: Vec3,
        cancel: impl Fn() -> bool,
    ) -> Option<Region> {
        let mut added = HashSet::new();
        self.ensure(key, 0, &cancel, &mut added)?;
        self.clock = self.clock.wrapping_add(1);
        let (meshes, mut bytes) = if let Some(cache) = self.meshes.get_mut(&key) {
            cache.used = self.clock;
            (cache.meshes.clone(), cache.gpu)
        } else {
            let nonempty = (0..4).any(|y| {
                (0..4).any(|z| {
                    (0..4).any(|x| {
                        self.world
                            .has_geometry([key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z])
                    })
                })
            });
            if nonempty {
                self.ensure(key, 1, &cancel, &mut added)?;
            }
            let mut meshes = Vec::new();
            let mut bytes = 0;
            for y in 0..4 {
                for z in 0..4 {
                    for x in 0..4 {
                        if cancel() {
                            return None;
                        }
                        let page = [key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z];
                        if page[1] * 32 >= self.world.height() {
                            continue;
                        }
                        let mesh = Mesh::build_chunk(&self.world, page);
                        bytes += mesh.vertices.len()
                            * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
                            + (mesh.indices.len() + mesh.transparent_indices.len()) * 4;
                        if !mesh.vertices.is_empty() {
                            meshes.push((page, Arc::new(mesh)));
                        }
                    }
                }
            }

            let cpu = meshes
                .iter()
                .map(|(_, m)| {
                    m.vertices.capacity() * std::mem::size_of::<crate::mesh::Vertex>()
                        + (m.indices.capacity() + m.transparent_indices.capacity()) * 4
                })
                .sum::<usize>();
            let budget = 256 * 1024 * 1024;
            if !meshes.is_empty() && cpu <= budget {
                while self.mesh_bytes + cpu > budget {
                    let Some(old) = self
                        .meshes
                        .iter()
                        .min_by_key(|(_, m)| m.used)
                        .map(|(k, _)| *k)
                    else {
                        break;
                    };
                    self.mesh_bytes -= self.meshes.remove(&old).unwrap().cpu;
                }
                self.meshes.insert(
                    key,
                    CachedMesh {
                        meshes: meshes.clone(),
                        gpu: bytes,
                        cpu,
                        used: self.clock,
                    },
                );
                self.mesh_bytes += cpu;
            }
            (meshes, bytes)
        };
        let origin = key.map(|v| v * CELLS);
        let objects = self.world.doors.iter().any(|d| {
            d.occupied()
                .any(|(p, _)| (0..3).all(|i| p[i] >= origin[i] && p[i] < origin[i] + CELLS))
        });
        let light = if meshes.is_empty() && !objects {
            None
        } else if let Some((light, used)) = self.lights.get_mut(&key) {
            *used = self.clock;
            Some(light.clone())
        } else {
            self.ensure(key, 4, &cancel, &mut added)?;
            let low = origin.map(|v| v - 128);
            let field = LightField::build_region(&self.world, low, [96; 3], |point| {
                self.sky
                    .values(&self.world, &mut self.sampler, point, low[1], 96)
            });
            let light: Arc<[u8]> = field.crop([31; 3], [34; 3]).into();
            if self.lights.len() >= 768 {
                if let Some(old) = self
                    .lights
                    .iter()
                    .min_by_key(|(_, (_, used))| *used)
                    .map(|(key, _)| *key)
                {
                    self.lights.remove(&old);
                }
            }
            self.lights.insert(key, (light.clone(), self.clock));
            Some(light)
        };
        if light.is_some() {
            bytes += 34_usize.pow(3) * 2;
        }
        bytes += 256;
        if cancel() {
            return None;
        }
        let evicted = self.prune(focus, key, DATA_BUDGET);
        let pages = added
            .into_iter()
            .filter_map(|key| {
                self.world.page(key).cloned().map(|data| {
                    self.published.insert(key);
                    (key, data)
                })
            })
            .collect();
        Some(Region {
            key,
            revision,
            pages,
            evicted,
            meshes,
            light,
            bytes,
        })
    }
    fn ensure(
        &mut self,
        key: Key,
        padding: i32,
        cancel: &impl Fn() -> bool,
        added: &mut HashSet<Key>,
    ) -> Option<()> {
        let preview = self.world.noise_preview.expect("infinite source");
        for y in (key[1] * 4 - padding).max(0)
            ..(key[1] * 4 + 4 + padding).min(self.world.chunks_y() as i32)
        {
            for z in key[2] * 4 - padding..key[2] * 4 + 4 + padding {
                for x in key[0] * 4 - padding..key[0] * 4 + 4 + padding {
                    if cancel() {
                        return None;
                    }
                    let page = [x, y, z];
                    if self.world.page(page).is_none() {
                        let chunk = crate::worldgen::page::generate(
                            preview,
                            &mut self.sampler,
                            page.map(|v| v * 32),
                        )
                        .data;
                        self.world.insert_page(page, chunk);
                    }
                    if !self.published.contains(&page) {
                        added.insert(page);
                    }
                }
            }
        }
        Some(())
    }
    pub fn trim(&mut self, focus: Vec3) -> Vec<Key> {
        self.prune(focus, key(focus), DATA_BUDGET)
    }
    fn prune(&mut self, focus: Vec3, current: Key, budget: usize) -> Vec<Key> {
        let mut bytes = self.world.voxel_storage_bytes() + self.world.pages().count() * 128;
        let mut evicted = Vec::new();
        if bytes <= budget {
            return evicted;
        }
        let mut keys: Vec<_> = self
            .world
            .pages()
            .map(|(key, c)| {
                (
                    key,
                    c.occupied(),
                    match c {
                        Chunk::Uniform(_) => 129,
                        Chunk::Dense { cells, .. } => cells.len() + 128,
                    },
                )
            })
            .collect();
        keys.sort_by(|a, b| {
            let distance = |p: Key| {
                World::meters(p[0] * 32 + 16, p[1] * 32 + 16, p[2] * 32 + 16)
                    .distance_squared(focus)
            };
            distance(b.0).total_cmp(&distance(a.0))
        });
        for (key, _, size) in keys {
            let p = World::meters(key[0] * 32 + 16, key[1] * 32 + 16, key[2] * 32 + 16);
            if p.distance(focus) < 16.0
                || (0..3).all(|i| key[i] >= current[i] * 4 - 4 && key[i] < current[i] * 4 + 8)
            {
                continue;
            }
            self.world.unload(key);
            bytes -= size;
            if self.published.remove(&key) {
                evicted.push(key);
            }
            if bytes <= budget {
                break;
            }
        }
        evicted
    }
}

pub fn initial(preview: Preview, progress: &crate::startup::Progress) -> Result<World, String> {
    let generator = Generator::new(preview.seed)?;
    let mut world = World::with_height(preview.height_m * 16);
    world.infinite = true;
    world.noise_preview = Some(preview);
    let y = crate::worldgen::sky::surface(preview, &mut generator.sampler(), 100, 347);
    let position = World::meters(100, y, 347) + Vec3::Y * 0.00002;
    world.spawn = Some(position);
    let region = key(position - Vec3::Y * 0.125);
    progress.begin(crate::startup::Stage::Terrain, 1728);
    let mut sampler = generator.sampler();
    for cy in (region[1] * 4 - 4).max(0)..(region[1] * 4 + 8).min(world.chunks_y() as i32) {
        for cz in region[2] * 4 - 4..region[2] * 4 + 8 {
            for cx in region[0] * 4 - 4..region[0] * 4 + 8 {
                progress.check()?;
                let key = [cx, cy, cz];
                let data =
                    crate::worldgen::page::generate(preview, &mut sampler, key.map(|v| v * 32))
                        .data;
                world.insert_page(key, data);
                progress.advance();
            }
        }
    }
    println!(
        "Infinite spawn prepared at {:?}, {:.1} MiB resident voxel data",
        position,
        world.voxel_storage_bytes() as f64 / 1048576.0
    );
    Ok(world)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_eviction_discards_resident_data_but_preserves_edit_overrides() {
        let generator = Generator::new(1).unwrap();
        let mut world = World::with_height(384 * 16);
        world.infinite = true;
        world.noise_preview = Some(Default::default());
        let far = [100, 10, -100];
        let cell = [far[0] * 32 + 2, far[1] * 32 + 2, far[2] * 32 + 2];
        world.insert_page(far, Chunk::Uniform(crate::world::STONE));
        world.set(cell[0], cell[1], cell[2], crate::world::LIGHT);
        let mut builder = Builder::new(world, &generator, Sky::default());
        let evicted = builder.prune(Vec3::ZERO, [2, 2, 2], 128);
        assert!(evicted.contains(&far));
        assert!(!builder.world.is_loaded(cell));
        let preview = builder.world.noise_preview.unwrap();
        let data =
            crate::worldgen::page::generate(preview, &mut builder.sampler, far.map(|v| v * 32))
                .data;
        builder.world.insert_page(far, data);
        assert_eq!(
            builder.world.get(cell[0], cell[1], cell[2]),
            crate::world::LIGHT
        );
    }
}
