use crate::{
    camera::Frustum,
    editing,
    mesh::{Batch, Mesh},
    world::World,
};
use glam::Vec3;
use std::collections::HashMap;
use wgpu::util::DeviceExt;
struct GpuChunk {
    vertices: wgpu::Buffer,
    indices: Option<wgpu::Buffer>,
    transparent: Option<wgpu::Buffer>,
    count: u32,
    transparent_count: u32,
    center: Vec3,
}
/// Nearby streamed buffers, or shared buffers for small scenes with replacements after edits.
pub struct SceneMesh {
    pub world: World,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    transparent: Option<wgpu::Buffer>,
    count: u32,
    batches: Vec<Batch>,
    transparent_batches: Vec<Batch>,
    replacements: HashMap<[i32; 3], Option<GpuChunk>>,
    stream: Option<crate::mesh_stream::Stream>,
}
impl SceneMesh {
    #[cfg(test)]
    pub fn new(device: &wgpu::Device, world: World) -> Self {
        Self::new_with_progress(device, world, &crate::startup::Progress::default())
            .expect("Uncancelled scene upload")
    }
    pub fn new_with_progress(
        device: &wgpu::Device,
        world: World,
        progress: &crate::startup::Progress,
    ) -> Result<Self, String> {
        if world.height() > crate::world::HEIGHT {
            let position = crate::physics::Player::safe_spawn(&world).position
                + Vec3::Y * crate::physics::EYE_HEIGHT;
            let keys = crate::mesh_stream::wanted(&world, position);
            progress.begin(crate::startup::Stage::Mesh, keys.len());
            let mut replacements = HashMap::new();
            let mut bytes = 0;
            let next = std::sync::atomic::AtomicUsize::new(0);
            let workers = std::thread::available_parallelism()
                .map_or(1, usize::from)
                .saturating_sub(2)
                .clamp(1, 4);
            std::thread::scope(|scope| {
                let (sender, receiver) = std::sync::mpsc::sync_channel(4);
                for _ in 0..workers {
                    let sender = sender.clone();
                    let next = &next;
                    let keys = &keys;
                    let world = &world;
                    scope.spawn(move || loop {
                        if progress.check().is_err() {
                            break;
                        }
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&key) = keys.get(index) else { break };
                        if sender.send((key, Mesh::build_chunk(world, key))).is_err() {
                            break;
                        }
                    });
                }
                drop(sender);
                for (key, mesh) in &receiver {
                    if progress.check().is_err() {
                        break;
                    }
                    bytes += mesh.vertices.len()
                        * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
                        + (mesh.indices.len() + mesh.transparent_indices.len()) * 4;
                    replacements.insert(key, gpu_chunk(device, key, &mesh));
                    progress.advance();
                }
                drop(receiver);
            });
            progress.check()?;
            println!(
                "Nearby mesh cache: {} chunks, {:.1} MiB",
                keys.len(),
                bytes as f64 / 1048576.0
            );
            return Ok(Self {
                world,
                vertices: buffer(
                    device,
                    "unused shared vertices",
                    &[],
                    wgpu::BufferUsages::VERTEX,
                ),
                indices: buffer(
                    device,
                    "unused shared indices",
                    &[],
                    wgpu::BufferUsages::INDEX,
                ),
                transparent: None,
                count: 0,
                batches: Vec::new(),
                transparent_batches: Vec::new(),
                replacements,
                stream: Some(crate::mesh_stream::Stream::new(position, keys)),
            });
        }
        let mut mesh = Mesh::build_with_progress(&world, progress)?;
        if world.noise_preview.is_none() {
            mesh.add_horizon();
        }
        println!(
            "{} opaque / {} transparent triangles in {} chunks",
            mesh.indices.len() / 3,
            mesh.transparent_indices.len() / 3,
            mesh.batches.len()
        );
        Ok(Self {
            world,
            vertices: buffer(
                device,
                "world vertices",
                bytemuck::cast_slice(&crate::gpu_vertex::pack(&mesh.vertices)),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                device,
                "opaque indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            transparent: optional_indices(device, &mesh.transparent_indices),
            count: mesh.indices.len() as u32,
            batches: mesh.batches,
            transparent_batches: mesh.transparent_batches,
            replacements: HashMap::new(),
            stream: None,
        })
    }
    pub fn edit(
        &mut self,
        device: &wgpu::Device,
        cell: [i32; 3],
        material: u8,
    ) -> Option<Vec<[i32; 3]>> {
        let edit = self.world.edit(cell, material)?;
        let chunks: std::collections::HashSet<_> = edit
            .terrain
            .iter()
            .flat_map(|&p| editing::affected_chunks(p, self.world.height()))
            .collect();
        for chunk in chunks {
            let mesh = Mesh::build_chunk(&self.world, chunk);
            let replacement = gpu_chunk(device, chunk, &mesh);
            self.replacements.insert(chunk, replacement);
        }
        if let Some(stream) = &mut self.stream {
            let position = World::meters(
                stream.center[0] * 32 + 16,
                stream.center[1] * 32 + 16,
                stream.center[2] * 32 + 16,
            );
            stream.request(&self.world, position, |key| {
                self.replacements.contains_key(key)
            });
        }
        Some(edit.light)
    }
    pub fn update_stream(&mut self, device: &wgpu::Device, position: Vec3) -> bool {
        let Some(stream) = &mut self.stream else {
            return false;
        };
        let mut changed = false;
        if crate::mesh_stream::center(position) != stream.center {
            stream.request(&self.world, position, |key| {
                self.replacements.contains_key(key)
            });
            let before = self.replacements.len();
            self.replacements
                .retain(|key, _| stream.desired.contains(key));
            changed = before != self.replacements.len();
        }
        stream.submit();
        let start = std::time::Instant::now();
        let mut bytes = 0;
        while start.elapsed().as_micros() < 2000 && bytes < 8 * 1024 * 1024 {
            let result = match stream.results.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("Mesh worker stopped"),
            };
            if !stream.accepts(&result) {
                continue;
            }
            bytes += result.mesh.vertices.len()
                * std::mem::size_of::<crate::gpu_vertex::GpuVertex>()
                + (result.mesh.indices.len() + result.mesh.transparent_indices.len()) * 4;
            self.replacements
                .insert(result.key, gpu_chunk(device, result.key, &result.mesh));
            changed = true;
        }
        changed
    }
    pub fn meshes_pending(&self) -> bool {
        self.stream.as_ref().is_some_and(|stream| {
            stream
                .desired
                .iter()
                .any(|key| !self.replacements.contains_key(key))
        })
    }
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&Frustum>,
    ) -> (usize, usize) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        if frustum.is_none() && self.replacements.is_empty() {
            pass.draw_indexed(0..self.count, 0, 0..1);
            return (self.batches.len(), self.count as usize / 3);
        }
        let mut stats = (0, 0);
        for batch in &self.batches {
            if batch
                .chunk
                .is_some_and(|key| self.replacements.contains_key(&key))
                || frustum.is_some_and(|f| !f.contains_sphere(batch.center, batch.radius))
            {
                continue;
            }
            pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            stats.0 += 1;
            stats.1 += batch.indices.len() / 3;
        }
        let chunks: Vec<_> = if let Some(stream) = &self.stream {
            stream
                .order
                .iter()
                .filter_map(|key| self.replacements.get(key).and_then(Option::as_ref))
                .collect()
        } else {
            self.replacements.values().flatten().collect()
        };
        for chunk in chunks {
            if frustum.is_some_and(|f| !f.contains_sphere(chunk.center, 3.0_f32.sqrt())) {
                continue;
            }
            if let Some(indices) = &chunk.indices {
                pass.set_vertex_buffer(0, chunk.vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.count, 0, 0..1);
                stats.0 += 1;
                stats.1 += chunk.count as usize / 3;
            }
        }
        stats
    }
    pub fn draw_transparent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frustum: &Frustum,
        camera: Vec3,
    ) -> (usize, usize) {
        let mut draws = Vec::new();
        if let Some(indices) = &self.transparent {
            for batch in &self.transparent_batches {
                if batch
                    .chunk
                    .is_some_and(|key| self.replacements.contains_key(&key))
                    || !frustum.contains_sphere(batch.center, batch.radius)
                {
                    continue;
                }
                draws.push((&self.vertices, indices, batch.indices.clone(), batch.center));
            }
        }
        for chunk in self.replacements.values().flatten() {
            if let Some(indices) = &chunk.transparent {
                if frustum.contains_sphere(chunk.center, 3.0_f32.sqrt()) {
                    draws.push((
                        &chunk.vertices,
                        indices,
                        0..chunk.transparent_count,
                        chunk.center,
                    ));
                }
            }
        }
        draws.sort_by(|a, b| {
            b.3.distance_squared(camera)
                .total_cmp(&a.3.distance_squared(camera))
        });
        let mut triangles = 0;
        let count = draws.len();
        for (vertices, indices, range, _) in draws {
            triangles += range.len() / 3;
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(range, 0, 0..1);
        }
        (count, triangles)
    }
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}
fn optional_indices(device: &wgpu::Device, indices: &[u32]) -> Option<wgpu::Buffer> {
    if indices.is_empty() {
        None
    } else {
        Some(buffer(
            device,
            "chunk indices",
            bytemuck::cast_slice(indices),
            wgpu::BufferUsages::INDEX,
        ))
    }
}

fn gpu_chunk(device: &wgpu::Device, key: [i32; 3], mesh: &Mesh) -> Option<GpuChunk> {
    if mesh.indices.is_empty() && mesh.transparent_indices.is_empty() {
        return None;
    }
    Some(GpuChunk {
        vertices: buffer(
            device,
            "chunk vertices",
            bytemuck::cast_slice(&crate::gpu_vertex::pack(&mesh.vertices)),
            wgpu::BufferUsages::VERTEX,
        ),
        indices: optional_indices(device, &mesh.indices),
        transparent: optional_indices(device, &mesh.transparent_indices),
        count: mesh.indices.len() as u32,
        transparent_count: mesh.transparent_indices.len() as u32,
        center: World::meters(key[0] * 32 + 16, key[1] * 32 + 16, key[2] * 32 + 16),
    })
}
