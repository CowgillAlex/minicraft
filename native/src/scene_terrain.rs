use super::{buffer, gpu_chunk, GpuChunk, SceneMesh};
use crate::{
    light_volume::RegionLight,
    startup::Progress,
    terrain_region::{self, Key, Region},
    terrain_stream::Stream,
    world::World,
    worldgen::{sky::Sky, Generator},
};
use glam::Vec3;
use std::{collections::HashMap, sync::mpsc::TryRecvError, time::Instant};

pub(super) struct Resident {
    light: Option<RegionLight>,
    bytes: usize,
    valid: bool,
}
struct Pending {
    region: Region,
    meshes: std::vec::IntoIter<(Key, std::sync::Arc<crate::mesh::Mesh>)>,
    uploaded: HashMap<Key, Option<GpuChunk>>,
    light: Option<RegionLight>,
}
pub(super) struct Terrain {
    stream: Stream,
    regions: HashMap<Key, Resident>,
    initial: Option<Region>,
    pending: Option<Pending>,
    position: Vec3,
    radius: f32,
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
            HashMap::from([(key, initial.bytes)]),
        );
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
                regions: HashMap::new(),
                initial: Some(initial),
                pending: None,
                position,
                radius,
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
    pub fn terrain_full(&self) -> bool {
        self.terrain.as_ref().is_some_and(|t| t.stream.full())
    }
}
impl Terrain {
    fn loaded(&self) -> HashMap<Key, usize> {
        self.regions
            .iter()
            .filter(|(_, v)| v.valid)
            .map(|(k, v)| (*k, v.bytes))
            .collect()
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some() || self.stream.pending()
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
                let existing = self.regions.get(&pending.region.key).map_or(0, |r| r.bytes);
                let mut total = self.regions.values().map(|r| r.bytes).sum::<usize>() - existing
                    + pending.region.bytes;
                if total > terrain_region::MESH_BUDGET {
                    self.stream.mark_full();
                    if !self.regions.contains_key(&pending.region.key)
                        && terrain_region::center(pending.region.key).distance(self.position) > 16.0
                    {
                        return false;
                    }
                    let mut far: Vec<_> = self
                        .regions
                        .keys()
                        .copied()
                        .filter(|key| {
                            *key != pending.region.key
                                && terrain_region::center(*key).distance(self.position) > 16.0
                        })
                        .collect();
                    far.sort_by(|a, b| {
                        terrain_region::center(*b)
                            .distance_squared(self.position)
                            .total_cmp(&terrain_region::center(*a).distance_squared(self.position))
                    });
                    for key in far {
                        if total <= terrain_region::MESH_BUDGET {
                            break;
                        }
                        total -= self.regions.remove(&key).unwrap().bytes;
                        scene
                            .replacements
                            .retain(|page, _| terrain_region::page_region(*page) != key);
                    }
                }
                scene
                    .replacements
                    .retain(|key, _| terrain_region::page_region(*key) != pending.region.key);
                scene.replacements.extend(pending.uploaded);
                self.regions.insert(
                    pending.region.key,
                    Resident {
                        light: pending.light,
                        bytes: pending.region.bytes,
                        valid: true,
                    },
                );
                return true;
            };
            bytes += mesh.vertices.len() * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
                + (mesh.indices.len() + mesh.transparent_indices.len()) * 4;
            pending.uploaded.insert(key, gpu_chunk(device, key, &mesh));
        }
        false
    }
    pub(super) fn update(
        &mut self,
        scene: &mut SceneMesh,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        position: Vec3,
        radius: f32,
    ) -> bool {
        let mut changed = false;
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
        changed
    }
    pub(super) fn chunks<'a>(&self, scene: &'a SceneMesh) -> Vec<&'a GpuChunk> {
        let mut chunks: Vec<_> = scene.replacements.values().flatten().collect();
        chunks.sort_by(|a, b| {
            a.center
                .distance_squared(self.position)
                .total_cmp(&b.center.distance_squared(self.position))
        });
        chunks
    }
}
