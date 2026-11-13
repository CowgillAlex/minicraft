use crate::{
    chunk::Chunk,
    light_field::LightField,
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
pub fn candidates(
    world: &World,
    position: Vec3,
    radius: f32,
    direction: Vec3,
    travel: Vec3,
) -> Vec<Key> {
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
                    let score = crate::terrain_budget::View {
                        position,
                        direction,
                        travel,
                        radius,
                    }
                    .priority(key);
                    keys.push((key, score));
                }
            }
        }
    }
    keys.sort_by(|a, b| a.1.total_cmp(&b.1));
    keys.into_iter().map(|(key, _)| key).collect()
}
pub struct Region {
    pub level: crate::terrain_lod::Level,
    pub key: Key,
    pub revision: u64,
    pub pages: Vec<(Key, Chunk)>,
    pub evicted: Vec<Key>,
    pub meshes: Vec<(Key, Arc<crate::gpu_vertex::PackedMesh>)>,
    pub light: Option<Arc<[u8]>>,
    pub bytes: usize,
    pub field: Option<Box<LightField>>,
}
struct CachedMesh {
    meshes: Vec<(Key, Arc<crate::gpu_vertex::PackedMesh>)>,
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
    meshes: HashMap<(Key, crate::terrain_lod::Level), CachedMesh>,
    mesh_bytes: usize,
    tiles: crate::light_tiles::Tiles,
    pages: Option<crate::terrain_pages::Pages>,
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
            tiles: Default::default(),
            pages: None,
        }
    }
    pub fn into_parts(self) -> (World, Sky) {
        (self.world, self.sky)
    }
    pub fn edit(&mut self, snapshot: &World, cells: &[[i32; 3]]) {
        self.world.copy_edits_from(snapshot);
        if let Some(pool) = &self.pages {
            pool.invalidate_meshes(cells);
        }
        self.sky.invalidate(cells);
        self.tiles.invalidate(cells);
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
            let min =
                std::array::from_fn::<_, 3, _>(|i| cells.iter().map(|p| p[i]).min().unwrap() - 1);
            let max =
                std::array::from_fn::<_, 3, _>(|i| cells.iter().map(|p| p[i]).max().unwrap() + 1);
            self.meshes.retain(|(key, _), _| {
                !(0..3).all(|i| key[i] * 128 <= max[i] && key[i] * 128 + 128 > min[i])
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
        self.build_lod(
            key,
            revision,
            focus,
            crate::terrain_lod::Level::desired(key, focus),
            cancel,
        )
    }
    pub fn build_lod(
        &mut self,
        key: Key,
        revision: u64,
        focus: Vec3,
        level: crate::terrain_lod::Level,
        cancel: impl Fn() -> bool,
    ) -> Option<Region> {
        let started = std::time::Instant::now();
        let mut phases = [0.0; 4];
        let mut local_field = None;
        let mut added = HashSet::new();
        self.ensure(key, 0, &cancel, &mut added)?;
        phases[0] = started.elapsed().as_secs_f64();
        self.clock = self.clock.wrapping_add(1);
        let (meshes, mut bytes) = if let Some(cache) = self.meshes.get_mut(&(key, level)) {
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
            let mut keys = Vec::new();
            for y in 0..4 {
                for z in 0..4 {
                    for x in 0..4 {
                        let page = [key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z];
                        if page[1] * 32 < self.world.height() && self.world.has_geometry(page) {
                            keys.push(page);
                        }
                    }
                }
            }
            let pool = self.pages.get_or_insert_with(|| {
                crate::terrain_pages::Pages::new(
                    self.world.noise_preview.unwrap(),
                    self.sampler.feature_plans(),
                )
            });
            let mut bytes = 0;
            let meshes: Vec<_> = pool
                .meshes(&self.world, keys, level, &cancel)
                .into_iter()
                .filter_map(|(page, mesh)| {
                    bytes += mesh.bytes();
                    (!mesh.vertices.is_empty()).then_some((page, mesh))
                })
                .collect();
            if cancel() {
                return None;
            }

            let cpu = meshes
                .iter()
                .map(|(_, m)| {
                    m.vertices.capacity() * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
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
                    (key, level),
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
        phases[1] = started.elapsed().as_secs_f64() - phases[0];
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
            phases[2] = started.elapsed().as_secs_f64() - phases[0] - phases[1];
            self.pages.get_or_insert_with(|| {
                crate::terrain_pages::Pages::new(
                    self.world.noise_preview.unwrap(),
                    self.sampler.feature_plans(),
                )
            });
            let mut field = LightField::build_streamed(
                &self.world,
                origin,
                &mut crate::light_tiles::Source::new(
                    &self.world,
                    &mut self.sampler,
                    &mut self.sky,
                    &mut self.tiles,
                    &cancel,
                )
                .parallel(self.pages.as_ref().expect("terrain pool")),
            )?;
            let light: Arc<[u8]> = field.region_pixels(origin).into();
            if center(key).distance(focus) < 20.0 {
                field.prepare_edits(
                    &mut crate::light_tiles::Source::new(
                        &self.world,
                        &mut self.sampler,
                        &mut self.sky,
                        &mut self.tiles,
                        &cancel,
                    )
                    .parallel(self.pages.as_ref().unwrap()),
                );
                local_field = Some(Box::new(field));
            }
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
        phases[3] = started.elapsed().as_secs_f64() - phases[..3].iter().sum::<f64>();
        if std::env::var_os("MINICRAFT_PROFILE_STREAM").is_some() {
            eprintln!(
                "PROFILE {key:?} data={:.4} mesh={:.4} halo={:.4} light={:.4} total={:.4}",
                phases[0],
                phases[1],
                phases[2],
                phases[3],
                started.elapsed().as_secs_f64()
            );
        }
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
            level,
            key,
            revision,
            pages,
            evicted,
            meshes,
            light,
            field: local_field,
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
        let mut missing = Vec::new();
        for y in (key[1] * 4 - padding).max(0)
            ..(key[1] * 4 + 4 + padding).min(self.world.chunks_y() as i32)
        {
            for z in key[2] * 4 - padding..key[2] * 4 + 4 + padding {
                for x in key[0] * 4 - padding..key[0] * 4 + 4 + padding {
                    let page = [x, y, z];
                    if self.world.page(page).is_none() {
                        missing.push(page);
                    }
                    if !self.published.contains(&page) {
                        added.insert(page);
                    }
                }
            }
        }
        if !missing.is_empty() {
            let pool = self.pages.get_or_insert_with(|| {
                crate::terrain_pages::Pages::new(preview, self.sampler.feature_plans())
            });
            for (key, data) in pool.generate(missing, cancel) {
                self.world.insert_page(key, data);
            }
        }
        if cancel() {
            return None;
        }
        Some(())
    }
    /// Publish the swept player corridor before waiting for meshes or light.
    pub fn collision(
        &mut self,
        eye: Vec3,
        travel: Vec3,
        cancel: &dyn Fn() -> bool,
    ) -> Vec<(Key, Chunk)> {
        let feet = eye - Vec3::Y * crate::physics::EYE_HEIGHT;
        let ahead = feet + travel * 8.0;
        let lo = crate::ray::grid_position(feet.min(ahead) - Vec3::new(0.6, 0.3, 0.6))
            .floor()
            .as_ivec3()
            .to_array()
            .map(|v| v.div_euclid(32));
        let hi = crate::ray::grid_position(feet.max(ahead) + Vec3::new(0.6, 2.1, 0.6))
            .floor()
            .as_ivec3()
            .to_array()
            .map(|v| v.div_euclid(32));
        let mut missing = Vec::new();
        let mut needed = Vec::new();
        for y in lo[1].max(0)..=hi[1].min(self.world.chunks_y() as i32 - 1) {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let key = [x, y, z];
                    if !self.world.contains(key.map(|v| v * 32)) {
                        continue;
                    }
                    if self.world.page(key).is_none() {
                        missing.push(key);
                    }
                    if !self.published.contains(&key) {
                        needed.push(key);
                    }
                }
            }
        }
        let preview = self.world.noise_preview.unwrap();
        if !missing.is_empty() {
            let pool = self.pages.get_or_insert_with(|| {
                crate::terrain_pages::Pages::new(preview, self.sampler.feature_plans())
            });
            for (key, data) in pool.generate(missing, cancel) {
                self.world.insert_page(key, data);
            }
        }
        needed
            .into_iter()
            .filter_map(|key| {
                self.world.page(key).cloned().map(|page| {
                    self.published.insert(key);
                    (key, page)
                })
            })
            .collect()
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
    progress.begin(crate::startup::Stage::Terrain, 216);
    let mut keys = Vec::new();
    for cy in (region[1] * 4 - 1).max(0)..(region[1] * 4 + 5).min(world.chunks_y() as i32) {
        for cz in region[2] * 4 - 1..region[2] * 4 + 5 {
            for cx in region[0] * 4 - 1..region[0] * 4 + 5 {
                progress.check()?;
                keys.push([cx, cy, cz]);
            }
        }
    }
    let pool = crate::terrain_pages::Pages::new(preview, generator.feature_plans.clone());
    for (key, data) in pool.generate_with_progress(keys, &|| progress.check().is_err(), |count| {
        for _ in 0..count {
            progress.advance();
        }
    }) {
        world.insert_page(key, data);
    }
    progress.check()?;
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
