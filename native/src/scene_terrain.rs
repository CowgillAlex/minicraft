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
    time::Instant,
};

pub(super) struct Resident {
    level: Level,
    light: Option<RegionLight>,
    bytes: usize,
    valid: bool,
    light_revision: u64,
}
struct Pending {
    region: Region,
    meshes: std::vec::IntoIter<(Key, std::sync::Arc<crate::gpu_vertex::PackedMesh>)>,
    uploaded: HashMap<Key, Option<GpuChunk>>,
    light: Option<RegionLight>,
}
pub(super) struct Terrain {
    stream: Stream,
    local: crate::local_light::LocalLight,
    regions: HashMap<Key, Resident>,
    initial: Option<Region>,
    pending: Option<Pending>,
    position: Vec3,
    radius: f32,
    draw_order: Vec<Key>,
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
                pending: None,
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
                while terrain.pending.is_some() {
                    terrain.upload(self, device, usize::MAX);
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
            let loaded = terrain.loaded();
            terrain.pending = None;
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
            .collect()
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some() || self.stream.pending() || self.local.pending()
    }
    fn refresh(&mut self) {
        let loaded = self.loaded();
        self.stream
            .request(self.position, self.radius, loaded, None);
    }
    fn begin(
        &mut self,
        scene: &mut SceneMesh,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        mut region: Region,
    ) {
        for key in region.evicted.drain(..) {
            scene.world.unload(key);
        }
        for (key, data) in region.pages.drain(..) {
            scene.world.insert_page(key, data);
        }
        if region.revision != self.stream.revision
            || !terrain_region::visible(region.key, self.position, self.radius)
        {
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
            self.refresh();
            return;
        }
        if let Some(field) = region.field.take() {
            self.local.seed(crate::local_light::Seed {
                key: region.key,
                field: *field,
                revision: region.revision,
                focus: self.position,
            });
        }
        let light = region.light.take().map(|pixels| {
            RegionLight::new(device, queue, layout, region.key.map(|v| v * 128), &pixels)
        });
        let meshes = std::mem::take(&mut region.meshes).into_iter();
        self.pending = Some(Pending {
            region,
            meshes,
            uploaded: HashMap::new(),
            light,
        });
    }
    fn upload(&mut self, scene: &mut SceneMesh, device: &wgpu::Device, budget: usize) -> bool {
        let Some(pending) = &mut self.pending else {
            return false;
        };
        let start = Instant::now();
        let mut bytes = 0;
        while start.elapsed().as_micros() < budget as u128 && bytes < 8 * 1024 * 1024 {
            let Some((key, mesh)) = pending.meshes.next() else {
                let pending = self.pending.take().unwrap();
                let loaded = self
                    .regions
                    .iter()
                    .map(|(k, r)| {
                        (
                            *k,
                            Loaded {
                                bytes: r.bytes,
                                level: r.level,
                            },
                        )
                    })
                    .collect();
                let view = View {
                    position: self.position,
                    direction: self.stream.direction,
                    travel: Vec3::ZERO,
                    radius: self.radius,
                };
                let Some(victims) = terrain_budget::victims(
                    &loaded,
                    pending.region.key,
                    pending.region.bytes,
                    view,
                    terrain_region::MESH_BUDGET,
                ) else {
                    self.stream.mark_full();
                    self.refresh();
                    return false;
                };
                let refresh = !victims.is_empty()
                    || (self.stream.lod
                        && Level::select(
                            pending.region.key,
                            self.position,
                            Some(pending.region.level),
                        ) != pending.region.level);
                self.evict(scene, &victims);
                scene
                    .replacements
                    .retain(|key, _| terrain_region::page_region(*key) != pending.region.key);
                scene.replacements.extend(pending.uploaded);
                self.regions.insert(
                    pending.region.key,
                    Resident {
                        level: pending.region.level,
                        light: pending.light,
                        bytes: pending.region.bytes,
                        valid: true,
                        light_revision: pending.region.revision,
                    },
                );
                if refresh {
                    self.refresh();
                }
                return true;
            };
            bytes += mesh.vertices.len() * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
                + (mesh.indices.len() + mesh.transparent_indices.len()) * 4;
            pending
                .uploaded
                .insert(key, gpu_chunk_packed(device, key, &mesh));
        }
        false
    }
    fn evict(&mut self, scene: &mut SceneMesh, keys: &[Key]) {
        let keys: HashSet<_> = keys.iter().copied().collect();
        for key in &keys {
            self.regions.remove(key);
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
        let lod = settings.lod;
        let detail_changed = self.stream.lod != lod;
        self.stream.lod = lod;
        let position = camera.position;
        let direction = camera.direction();
        let turned = self.stream.direction.dot(direction) < 0.9;
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
            if let Some(pending) = &self.pending {
                if pending.region.key == update.key && update.revision >= pending.region.revision {
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
        if self.pending.as_ref().is_some_and(|pending| {
            pending.region.revision != self.stream.revision
                || !terrain_region::visible(pending.region.key, position, radius)
        }) {
            self.pending = None;
        }
        if crate::mesh_stream::center(position) != self.stream.center
            || radius != self.stream.radius
            || turned
            || detail_changed
        {
            let before = self.regions.len();
            self.regions
                .retain(|key, _| terrain_region::visible(*key, position, radius));
            scene.replacements.retain(|key, _| {
                self.regions
                    .contains_key(&terrain_region::page_region(*key))
            });
            changed = before != self.regions.len();
            self.stream.request(position, radius, self.loaded(), None);
        }
        let start = Instant::now();
        while start.elapsed().as_micros() < 2000 {
            if self.pending.is_none() {
                match self.stream.results.try_recv() {
                    Ok(Ok(crate::terrain_stream::Update::Pages(pages))) => {
                        self.stream.received();
                        for (key, data) in pages {
                            scene.world.insert_page(key, data);
                        }
                    }
                    Ok(Ok(crate::terrain_stream::Update::MeshesEvicted { keys, epoch })) => {
                        self.stream.received();
                        if self.stream.is_current(epoch) {
                            self.evict(scene, &keys);
                            changed = true;
                        }
                    }
                    Ok(Ok(crate::terrain_stream::Update::Evicted(keys))) => {
                        self.stream.received();
                        for key in keys {
                            scene.world.unload(key);
                        }
                    }
                    Ok(Ok(crate::terrain_stream::Update::Region(region))) => {
                        self.stream.received();
                        self.begin(scene, device, queue, layout, region)
                    }
                    Ok(Err(error)) => panic!("{error}"),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => panic!("Terrain worker stopped"),
                }
            }
            changed |= self.upload(scene, device, 2000);
            if self.pending.is_some() {
                break;
            }
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
