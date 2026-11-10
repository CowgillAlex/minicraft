use crate::{
    camera::Frustum,
    editing,
    mesh::{Batch, Mesh},
    world::{World, CHUNK},
};
use glam::Vec3;
use std::collections::HashMap;
use wgpu::util::DeviceExt;
struct GpuChunk {
    vertices: wgpu::Buffer,
    indices: Option<wgpu::Buffer>,
    glass: Option<wgpu::Buffer>,
    count: u32,
    glass_count: u32,
    center: Vec3,
}
/// Static shared buffers plus small replacement buffers for edited chunks.
pub struct SceneMesh {
    pub world: World,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    glass: Option<wgpu::Buffer>,
    count: u32,
    batches: Vec<Batch>,
    glass_batches: Vec<Batch>,
    replacements: HashMap<[i32; 3], Option<GpuChunk>>,
}
impl SceneMesh {
    pub fn new(device: &wgpu::Device, world: World) -> Self {
        let mut mesh = Mesh::build(&world);
        if world.noise_preview.is_none() {
            mesh.add_horizon();
        }
        println!(
            "{} opaque / {} glass triangles in {} chunks",
            mesh.indices.len() / 3,
            mesh.glass_indices.len() / 3,
            mesh.batches.len()
        );
        Self {
            world,
            vertices: buffer(
                device,
                "world vertices",
                bytemuck::cast_slice(&mesh.vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                device,
                "opaque indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            glass: optional_indices(device, &mesh.glass_indices),
            count: mesh.indices.len() as u32,
            batches: mesh.batches,
            glass_batches: mesh.glass_batches,
            replacements: HashMap::new(),
        }
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
            .flat_map(|&p| editing::affected_chunks(p))
            .collect();
        for chunk in chunks {
            let mesh = Mesh::build_chunk(&self.world, chunk);
            let replacement = if mesh.indices.is_empty() && mesh.glass_indices.is_empty() {
                None
            } else {
                Some(GpuChunk {
                    vertices: buffer(
                        device,
                        "edited vertices",
                        bytemuck::cast_slice(&mesh.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: optional_indices(device, &mesh.indices),
                    glass: optional_indices(device, &mesh.glass_indices),
                    count: mesh.indices.len() as u32,
                    glass_count: mesh.glass_indices.len() as u32,
                    center: World::meters(
                        chunk[0] * CHUNK as i32 + 16,
                        chunk[1] * CHUNK as i32 + 16,
                        chunk[2] * CHUNK as i32 + 16,
                    ),
                })
            };
            self.replacements.insert(chunk, replacement);
        }
        Some(edit.light)
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
        for chunk in self.replacements.values().flatten() {
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
    pub fn draw_glass<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frustum: &Frustum,
        camera: Vec3,
    ) -> (usize, usize) {
        let mut draws = Vec::new();
        if let Some(indices) = &self.glass {
            for batch in &self.glass_batches {
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
            if let Some(indices) = &chunk.glass {
                if frustum.contains_sphere(chunk.center, 3.0_f32.sqrt()) {
                    draws.push((&chunk.vertices, indices, 0..chunk.glass_count, chunk.center));
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
