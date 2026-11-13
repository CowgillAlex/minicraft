use super::{buffer, gpu_chunk_packed, GpuChunk, SceneMesh};
use crate::{
    light_volume::RegionLight,
    startup::Progress,
    terrain_budget::{self, Loaded, View},
    terrain_lod::Level,
    terrain_region::{self, Key, Region},
    terrain_stream::Stream,
    world::World,
    worldgen::{sky::Sky, Generator},
};
use glam::Vec3;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::TryRecvError,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct Resident {
    level: Level,
    light: Option<Arc<RegionLight>>,
    bytes: usize,
    valid: bool,
    light_revision: u64,
}
struct Pending {
    region: Region,
    meshes: std::vec::IntoIter<(Key, std::sync::Arc<crate::gpu_vertex::PackedMesh>)>,
    uploaded: HashMap<Key, Option<GpuChunk>>,
    replacing: bool,
    upload_ms: f32,
    first_ready: Option<Instant>,
    light: Option<Arc<RegionLight>>,
}
pub(super) struct Terrain {
    stream: Stream,
    local: crate::local_light::LocalLight,
    regions: HashMap<Key, Resident>,
    initial: Option<Region>,
    pending: HashMap<Key, Pending>,
    last_latency: Option<crate::stream_metrics::Latency>,
    position: Vec3,
    radius: f32,
    draw_order: Vec<Key>,
}
struct UploadBudget {
    deadline: Option<Instant>,
    bytes: usize,
    pages: usize,
}
impl UploadBudget {
    fn frame() -> Self {
        Self {
            deadline: Some(Instant::now() + Duration::from_micros(2000)),
            bytes: 8 * 1024 * 1024,
            pages: usize::MAX,
        }
    }
    fn unlimited() -> Self {
        Self {
            deadline: None,
            bytes: usize::MAX,
            pages: usize::MAX,
        }
    }
    fn available(&self) -> bool {
        self.bytes > 0 && self.pages > 0 && self.deadline.is_none_or(|d| Instant::now() < d)
    }
}
impl SceneMesh {
    pub(super) fn infinite(
        device: &wgpu::Device,
        world: World,
        progress: &Progress,
    ) -> Result<Self, String> {
        let generator = Generator::new(world.noise_preview.expect("infinite").seed)?;
        let position = crate::physics::Player::safe_spawn(&world).position;
        let key = terrain_region::key(position - Vec3::Y * 0.125);
        let radius = crate::view_settings::ViewSettings::default().cache_radius();
        let mut builder = terrain_region::Builder::new(world, &generator, Sky::default());
        let initial = builder
            .build(key, 0, position, || progress.check().is_err())
            .ok_or("Loading cancelled")?;
        let (world, sky) = builder.into_parts();
        let stream = Stream::new(
            world.clone(),
            sky,
            position + Vec3::Y * crate::physics::EYE_HEIGHT,
            radius,
            HashMap::from([(
                key,
                Loaded {
                    bytes: initial.bytes,
                    level: initial.level,
                },
            )]),
        );
        let local = crate::local_light::LocalLight::new(world.noise_preview.unwrap());
        Ok(Self {
            world,
            vertices: buffer(
                device,
                "stream placeholder vertices",
                &[],
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                device,
                "stream placeholder indices",
                &[],
                wgpu::BufferUsages::INDEX,
            ),
            transparent: None,
            count: 0,
            batches: Vec::new(),
            transparent_batches: Vec::new(),
            replacements: HashMap::new(),
            stream: None,
            terrain: Some(Terrain {
                stream,
                local,
                regions: HashMap::new(),
                initial: Some(initial),
                pending: HashMap::new(),
                last_latency: None,
                position,
                radius,
                draw_order: Vec::new(),
            }),
        })
    }
    pub fn setup_terrain(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) {
        if let Some(mut terrain) = self.terrain.take() {
            if let Some(region) = terrain.initial.take() {
                terrain.begin(self, device, queue, layout, region);
                while let Some(key) = terrain.pending.keys().next().copied() {
                    terrain.upload(self, device, key, &mut UploadBudget::unlimited());
                }
            }
            self.terrain = Some(terrain);
        }
    }
    pub fn stream_edits(&mut self, cells: &[[i32; 3]]) {
        if let Some(terrain) = &mut self.terrain {
            if !cells.is_empty() {
                let min = std::array::from_fn::<_, 3, _>(|i| {
                    cells.iter().map(|p| p[i]).min().unwrap() - 128
                });
                let max = std::array::from_fn::<_, 3, _>(|i| {
                    cells.iter().map(|p| p[i]).max().unwrap() + 128
                });
                for (key, resident) in &mut terrain.regions {
                    let origin = key.map(|v| v * 128);
                    if origin[0] <= max[0]
                        && origin[0] + 128 >= min[0]
                        && origin[2] <= max[2]
                        && origin[2] + 128 >= min[2]
                        && origin[1] <= max[1]
                    {
                        resident.valid = false;
                    }
                }
            }
            let cancelled: Vec<_> = terrain
                .pending
                .drain()
                .map(|(_, p)| (p.region.key, p.region.revision, p.region.ticket))
                .collect();
            for (key, rev, ticket) in cancelled {
                terrain.stream.completed(key, rev, ticket, None);
            }
            let loaded = terrain.loaded();
            terrain.stream.request(
                terrain.position,
                terrain.radius,
                loaded,
                Some((&self.world, cells)),
            );
            terrain
                .local
                .edit(&self.world, cells, terrain.stream.revision);
        }
    }
    pub fn light_at(&self, position: Vec3) -> Option<&wgpu::BindGroup> {
        self.terrain
            .as_ref()?
            .regions
            .get(&terrain_region::key(position))?
            .light
            .as_ref()
            .map(|light| &light.group)
    }
    /// Empty air regions have no mesh or light texture. Sample the nearest
    /// available brick at its boundary until the camera enters a lit region.
    pub fn sky_light_at(&self, position: Vec3) -> Option<&wgpu::BindGroup> {
        self.light_at(position).or_else(|| {
            self.terrain
                .as_ref()?
                .regions
                .iter()
                .filter_map(|(key, region)| region.light.as_ref().map(|light| (key, light)))
                .min_by(|(a, _), (b, _)| {
                    terrain_region::center(**a)
                        .distance_squared(position)
                        .total_cmp(&terrain_region::center(**b).distance_squared(position))
                })
                .map(|(_, light)| &light.group)
        })
    }
    pub fn streaming_stats(&self) -> Option<crate::stream_metrics::Snapshot> {
        let terrain = self.terrain.as_ref()?;
        let mut stats = terrain.stream.stats();
        stats.staging = terrain.pending.len();
        stats.last = terrain.last_latency;
        Some(stats)
    }
    pub fn terrain_full(&self) -> bool {
        self.terrain.as_ref().is_some_and(|t| t.stream.full())
    }
}
impl Terrain {
    pub(super) fn invalidate_order(&mut self) {
        self.draw_order.clear();
    }
    fn loaded(&self) -> HashMap<Key, Loaded> {
        self.regions
            .iter()
            .filter(|(_, v)| v.valid)
            .map(|(k, v)| {
                (
                    *k,
                    Loaded {
                        bytes: v.bytes,
                        level: v.level,
                    },
                )
            })
            .chain(self.pending.iter().map(|(k, p)| {
                (
                    *k,
                    Loaded {
                        bytes: p.region.bytes,
                        level: p.region.level,
                    },
                )
            }))
            .collect()
    }
    pub(super) fn pending(&self) -> bool {
        !self.pending.is_empty() || self.stream.pending() || self.local.pending()
    }
    fn retained(&self, key: Key) -> Option<Loaded> {
        self.regions.get(&key).filter(|r| r.valid).map(|r| Loaded {
            bytes: r.bytes,
            level: r.level,
        })
    }
    fn discard(&mut self, key: Key, revision: u64, ticket: u64) {
        if self
            .pending
            .get(&key)
            .is_some_and(|p| p.region.ticket == ticket && p.region.revision == revision)
        {
            self.pending.remove(&key);
        }
        self.stream
            .completed(key, revision, ticket, self.retained(key));
    }
    fn begin(
        &mut self,
        scene: &mut SceneMesh,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        mut region: Region,
    ) {
        if region.revision != self.stream.revision
            || !terrain_region::visible(region.key, self.position, self.radius)
            || (region.ticket != 0
                && !self
                    .stream
                    .current_ticket(region.key, region.revision, region.ticket))
        {
            self.discard(region.key, region.revision, region.ticket);
            return;
        }
        let required = if self.stream.lod {
            Level::select(
                region.key,
                self.position,
                self.regions.get(&region.key).map(|r| r.level),
            )
        } else {
            Level::Full
        };
        if region.level > required {
            self.discard(region.key, region.revision, region.ticket);
            return;
        }
        let view = View {
            position: self.position,
            direction: self.stream.direction,
            travel: Vec3::ZERO,
            radius: self.radius,
        };
        let Some(victims) = terrain_budget::victims(
            &self.loaded(),
            region.key,
            region.bytes,
            view,
            terrain_region::MESH_BUDGET,
        ) else {
            self.stream.mark_full();
            self.discard(region.key, region.revision, region.ticket);
            return;
        };
        self.evict(scene, &victims);
        let started = Instant::now();
        for key in region.evicted.drain(..) {
            scene.world.unload(key);
        }
        for (key, data) in region.pages.drain(..) {
            scene.world.insert_page(key, data);
        }
        if let Some(field) = region.field.take() {
            self.local.seed(crate::local_light::Seed {
                key: region.key,
                field: *field,
                revision: region.revision,
                focus: self.position,
            });
        }
        let light = region
            .light
            .take()
            .map(|pixels| {
                Arc::new(RegionLight::new(
                    device,
                    queue,
                    layout,
                    region.key.map(|v| v * 128),
                    &pixels,
                ))
            })
            .or_else(|| {
                self.regions
                    .get(&region.key)
                    .filter(|r| r.light_revision == region.revision)
                    .and_then(|r| r.light.clone())
            });
        region.meshes.sort_by(|(a, _), (b, _)| {
            let center =
                |key: Key| World::meters(key[0] * 32 + 16, key[1] * 32 + 16, key[2] * 32 + 16);
            center(*a)
                .distance_squared(self.position)
                .total_cmp(&center(*b).distance_squared(self.position))
        });
        let meshes = std::mem::take(&mut region.meshes).into_iter();
        let replacing = self.regions.contains_key(&region.key);
        let upload_ms = started.elapsed().as_secs_f32() * 1000.0;
        self.pending.insert(
            region.key,
            Pending {
                region,
                meshes,
                uploaded: HashMap::new(),
                light,
                replacing,
                upload_ms,
                first_ready: None,
            },
        );
    }
    fn light_ready(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        mut region: Region,
    ) {
        if !self
            .stream
            .current_ticket(region.key, region.revision, region.ticket)
        {
            return;
        }
        let Some(pending) = self
            .pending
            .get_mut(&region.key)
            .filter(|p| p.region.ticket == region.ticket && p.region.revision == region.revision)
        else {
            return;
        };
        let started = Instant::now();
        if let Some(field) = region.field.take() {
            self.local.seed(crate::local_light::Seed {
                key: region.key,
                field: *field,
                revision: region.revision,
                focus: self.position,
            });
        }
        if let Some(pixels) = region.light.take() {
            pending.light = Some(Arc::new(RegionLight::new(
                device,
                queue,
                layout,
                region.key.map(|v| v * 128),
                &pixels,
            )));
        }
        pending.region.timing = region.timing;
        pending.upload_ms += started.elapsed().as_secs_f32() * 1000.0;
    }
    fn upload(
        &mut self,
        scene: &mut SceneMesh,
        device: &wgpu::Device,
        key: Key,
        budget: &mut UploadBudget,
    ) -> bool {
        let Some(pending) = self.pending.get_mut(&key) else {
            return false;
        };
        let started = Instant::now();
        while budget.available() {
            let Some((page, mesh)) = pending.meshes.next() else {
                break;
            };
            budget.bytes = budget.bytes.saturating_sub(mesh.bytes());
            budget.pages -= 1;
            pending
                .uploaded
                .insert(page, gpu_chunk_packed(device, page, &mesh));
        }
        pending.upload_ms += started.elapsed().as_secs_f32() * 1000.0;
        let lit = !pending.region.needs_light || pending.light.is_some();
        let complete = pending.meshes.len() == 0;
        let mut changed = false;
        if lit && !pending.replacing && !pending.uploaded.is_empty() {
            scene.replacements.extend(pending.uploaded.drain());
            pending.first_ready.get_or_insert_with(Instant::now);
            self.regions.insert(
                key,
                Resident {
                    level: pending.region.level,
                    light: pending.light.clone(),
                    bytes: pending.region.bytes,
                    valid: false,
                    light_revision: pending.region.revision,
                },
            );
            changed = true;
        }
        if !lit || !complete {
            return changed;
        }
        let pending = self.pending.remove(&key).unwrap();
        if pending.replacing {
            scene
                .replacements
                .retain(|page, _| terrain_region::page_region(*page) != key);
            scene.replacements.extend(pending.uploaded);
        }
        let now = Instant::now();
        let latency = pending.region.timing.latency(
            pending.first_ready.unwrap_or(now),
            now,
            pending.upload_ms,
        );
        self.last_latency = Some(latency);
        scene
            .world
            .chunk_timings
            .lock()
            .unwrap()
            .region(key, latency);
        self.regions.insert(
            key,
            Resident {
                level: pending.region.level,
                light: pending.light,
                bytes: pending.region.bytes,
                valid: true,
                light_revision: pending.region.revision,
            },
        );
        self.stream.completed(
            key,
            pending.region.revision,
            pending.region.ticket,
            Some(Loaded {
                bytes: pending.region.bytes,
                level: pending.region.level,
            }),
        );
        true
    }
    fn evict(&mut self, scene: &mut SceneMesh, keys: &[Key]) {
        self.stream.forget(keys);
        let keys: HashSet<_> = keys.iter().copied().collect();
        for key in &keys {
            self.regions.remove(key);
            if let Some(p) = self.pending.remove(key) {
                self.stream
                    .completed(*key, p.region.revision, p.region.ticket, None);
            }
        }
        scene
            .replacements
            .retain(|page, _| !keys.contains(&terrain_region::page_region(*page)));
    }
    pub(super) fn update(
        &mut self,
        scene: &mut SceneMesh,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        camera: &crate::camera::Camera,
        settings: crate::view_settings::ViewSettings,
    ) -> bool {
        let radius = settings.cache_radius();
        let detail_changed = self.stream.lod != settings.lod;
        self.stream.lod = settings.lod;
        let position = camera.position;
        let direction = camera.direction();
        let turned = self.stream.turned(direction);
        self.stream.direction = direction;
        let mut changed = false;
        while let Ok(update) = self.local.results.try_recv() {
            self.local.received();
            if let Some(resident) = self.regions.get_mut(&update.key) {
                if update.revision >= resident.light_revision {
                    if let Some(light) = &resident.light {
                        light.write(queue, &update.pixels);
                        resident.light_revision = update.revision;
                    }
                }
            }
            if let Some(pending) = self.pending.get(&update.key) {
                if update.revision >= pending.region.revision {
                    if let Some(light) = &pending.light {
                        light.write(queue, &update.pixels);
                    }
                }
            }
        }
        let moved =
            crate::mesh_stream::center(self.position) != crate::mesh_stream::center(position);
        self.position = position;
        self.radius = radius;
        let obsolete: Vec<_> = self
            .pending
            .values()
            .filter(|p| {
                p.region.revision != self.stream.revision
                    || !terrain_region::visible(p.region.key, position, radius)
                    || !self
                        .stream
                        .current_ticket(p.region.key, p.region.revision, p.region.ticket)
            })
            .map(|p| (p.region.key, p.region.revision, p.region.ticket))
            .collect();
        for (key, rev, ticket) in obsolete {
            self.discard(key, rev, ticket);
        }
        if self.stream.moved(position)
            || crate::mesh_stream::center(position) != self.stream.center
            || radius != self.stream.radius
            || turned
            || detail_changed
        {
            let before = self.regions.len();
            let gone: Vec<_> = self
                .regions
                .keys()
                .copied()
                .filter(|key| !terrain_region::visible(*key, position, radius))
                .collect();
            self.evict(scene, &gone);
            changed = before != self.regions.len();
            self.stream.request(position, radius, self.loaded(), None);
        }
        let mut budget = UploadBudget::frame();
        // Consume control and light events even while other regions are still uploading.
        while budget.available() {
            let update = match self.stream.results.try_recv() {
                Ok(update) => {
                    self.stream.received();
                    update.expect("terrain worker failed")
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => panic!("Terrain worker stopped"),
            };
            match update {
                crate::terrain_stream::Update::Pages(pages) => {
                    for (key, data) in pages {
                        scene.world.insert_page(key, data);
                    }
                }
                crate::terrain_stream::Update::MeshesEvicted { keys, epoch } => {
                    if self.stream.is_current(epoch) {
                        self.evict(scene, &keys);
                        changed = true;
                    }
                }
                crate::terrain_stream::Update::Evicted(keys) => {
                    for key in keys {
                        scene.world.unload(key);
                    }
                }
                crate::terrain_stream::Update::Geometry(region) => {
                    self.begin(scene, device, queue, layout, region)
                }
                crate::terrain_stream::Update::Region(region) => {
                    if region.needs_light {
                        self.light_ready(device, queue, layout, region);
                    } else {
                        self.begin(scene, device, queue, layout, region);
                    }
                }
                crate::terrain_stream::Update::Cancelled {
                    key,
                    revision,
                    ticket,
                } => self.discard(key, revision, ticket),
            }
        }
        let view = View {
            position,
            direction,
            travel: Vec3::ZERO,
            radius,
        };
        let mut keys: Vec<_> = self.pending.keys().copied().collect();
        keys.sort_by(|a, b| view.priority(*a).total_cmp(&view.priority(*b)));
        for key in keys {
            if !budget.available() {
                break;
            }
            changed |= self.upload(scene, device, key, &mut budget);
        }
        if changed || moved || self.draw_order.is_empty() {
            self.draw_order = scene
                .replacements
                .iter()
                .filter_map(|(k, c)| c.as_ref().map(|_| *k))
                .collect();
            self.draw_order.sort_by(|a, b| {
                let center = |k: Key| World::meters(k[0] * 32 + 16, k[1] * 32 + 16, k[2] * 32 + 16);
                center(*a)
                    .distance_squared(position)
                    .total_cmp(&center(*b).distance_squared(position))
            });
        }
        changed
    }
    pub(super) fn chunks<'a>(&self, scene: &'a SceneMesh) -> Vec<&'a GpuChunk> {
        self.draw_order
            .iter()
            .filter_map(|k| scene.replacements.get(k).and_then(Option::as_ref))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a graphics adapter"]
    fn editing_a_page_border_keeps_unloaded_neighbour_meshes_visible() {
        let instance = wgpu::Instance::default();
        let gpu = pollster::block_on(crate::gpu::Gpu::new(&instance, None)).unwrap();
        let mut world = World::with_height(32);
        for key in [[0, 0, 0], [1, 0, 0]] {
            world.insert_page(key, crate::chunk::Chunk::Uniform(crate::world::STONE));
        }
        let mut scene = SceneMesh::new(&gpu.device, world);
        scene.world.infinite = true;
        // Simulate a mesh arriving while its CPU page is still missing. A border
        // edit must never replace that existing solid neighbour with an empty mesh.
        let neighbour = [1, 0, 0];
        let original = crate::mesh::Mesh::build_chunk(&scene.world, neighbour);
        let count = original.indices.len() as u32;
        assert!(count > 0);
        scene.replacements.insert(
            neighbour,
            super::super::gpu_chunk(&gpu.device, neighbour, &original),
        );
        scene.world.unload(neighbour);
        scene
            .edit(&gpu.device, [31, 1, 1], crate::world::AIR)
            .unwrap();
        assert_eq!(
            scene.replacements[&neighbour].as_ref().unwrap().count,
            count
        );
        assert_eq!(scene.world.get_static(31, 1, 1), crate::world::AIR);
    }
    fn region(world: &World, light: bool) -> Region {
        let meshes: Vec<_> = [[0, 0, 0], [1, 0, 0]]
            .into_iter()
            .map(|key| {
                (
                    key,
                    Arc::new(crate::gpu_vertex::PackedMesh::from(
                        crate::mesh::Mesh::build_chunk(world, key),
                    )),
                )
            })
            .collect();
        let bytes =
            meshes.iter().map(|(_, m)| m.bytes()).sum::<usize>() + 34_usize.pow(3) * 2 + 256;
        Region {
            ticket: 0,
            needs_light: true,
            timing: crate::stream_metrics::WorkTiming::new(Instant::now()),
            key: [0, 0, 0],
            level: Level::Full,
            revision: 0,
            pages: Vec::new(),
            evicted: Vec::new(),
            meshes,
            light: light.then(|| vec![0; 34_usize.pow(3) * 2].into()),
            field: None,
            bytes,
        }
    }
    #[test]
    #[ignore = "requires a graphics adapter"]
    fn new_regions_publish_lit_pages_progressively_and_replacements_stay_atomic() {
        let instance = wgpu::Instance::default();
        let gpu = pollster::block_on(crate::gpu::Gpu::new(&instance, None)).unwrap();
        let mut world = World::empty();
        world.insert_page([0, 0, 0], crate::chunk::Chunk::Uniform(crate::world::STONE));
        world.insert_page([1, 0, 0], crate::chunk::Chunk::Uniform(crate::world::STONE));
        let mut scene = SceneMesh::new(&gpu.device, World::empty());
        let light_volume =
            crate::light_volume::LightVolume::new(&gpu.device, &gpu.queue, &scene.world);
        let preview = crate::worldgen::preview::Preview {
            origin: [120, 318, -20],
            height_m: 2,
            ..Default::default()
        };
        let mut stream_world = World::with_height(32);
        stream_world.infinite = true;
        stream_world.noise_preview = Some(preview);
        let position = terrain_region::center([0, 0, 0]);
        let mut terrain = Terrain {
            stream: Stream::new(stream_world, Sky::default(), position, 0.0, HashMap::new()),
            local: crate::local_light::LocalLight::new(preview),
            regions: HashMap::new(),
            initial: None,
            pending: HashMap::new(),
            last_latency: None,
            position,
            radius: 8.0,
            draw_order: Vec::new(),
        };
        let key = [0, 0, 0];
        terrain.begin(
            &mut scene,
            &gpu.device,
            &gpu.queue,
            &light_volume.layout,
            region(&world, true),
        );
        let mut one = UploadBudget::unlimited();
        one.pages = 1;
        assert!(terrain.upload(&mut scene, &gpu.device, key, &mut one));
        assert_eq!(scene.replacements.len(), 1);
        assert!(!terrain.regions[&key].valid);
        assert_eq!(terrain.pending[&key].meshes.len(), 1);
        terrain.upload(&mut scene, &gpu.device, key, &mut UploadBudget::unlimited());
        assert_eq!(scene.replacements.len(), 2);
        assert!(terrain.regions[&key].valid);
        let old: Vec<_> = [[0, 0, 0], [1, 0, 0]]
            .map(|k| scene.replacements[&k].as_ref().unwrap().count)
            .into();
        let mut edited = World::empty();
        edited.set(0, 0, 0, crate::world::STONE);
        edited.set(32, 0, 0, crate::world::STONE);
        terrain.begin(
            &mut scene,
            &gpu.device,
            &gpu.queue,
            &light_volume.layout,
            region(&edited, true),
        );
        let mut one = UploadBudget::unlimited();
        one.pages = 1;
        terrain.upload(&mut scene, &gpu.device, key, &mut one);
        for (k, count) in [[0, 0, 0], [1, 0, 0]].into_iter().zip(&old) {
            assert_eq!(
                scene.replacements[&k].as_ref().unwrap().count,
                *count,
                "replacement must not mix LODs across a region"
            );
        }
        terrain.upload(&mut scene, &gpu.device, key, &mut UploadBudget::unlimited());
        assert_ne!(
            scene.replacements[&[0, 0, 0]].as_ref().unwrap().count,
            old[0]
        );
        scene.replacements.clear();
        terrain.regions.clear();
        terrain.begin(
            &mut scene,
            &gpu.device,
            &gpu.queue,
            &light_volume.layout,
            region(&world, false),
        );
        assert!(!terrain.upload(&mut scene, &gpu.device, key, &mut UploadBudget::unlimited()));
        assert!(
            scene.replacements.is_empty(),
            "unlit geometry must remain staged"
        );
        terrain.discard(key, 0, 999);
        assert!(
            terrain.pending.contains_key(&key),
            "stale cancellation cannot discard a newer upload"
        );
        terrain.pending.get_mut(&key).unwrap().light = Some(Arc::new(RegionLight::new(
            &gpu.device,
            &gpu.queue,
            &light_volume.layout,
            [0; 3],
            &vec![0; 34_usize.pow(3) * 2],
        )));
        assert!(terrain.upload(&mut scene, &gpu.device, key, &mut UploadBudget::unlimited()));
        assert_eq!(scene.replacements.len(), 2);
    }
}
