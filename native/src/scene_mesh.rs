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
    indices: wgpu::Buffer,
    count: u32,
    center: Vec3,
}
/// Keep the original shared mesh, replacing only chunks touched by edits.
pub struct SceneMesh {
    pub world: World,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    batches: Vec<Batch>,
    replacements: HashMap<[i32; 3], Option<GpuChunk>>,
}
impl SceneMesh {
    pub fn new(device: &wgpu::Device, world: World) -> Self {
        let mut mesh = Mesh::build(&world);
        mesh.add_horizon();
        println!(
            "{} triangles in {} chunks",
            mesh.indices.len() / 3,
            mesh.batches.len()
        );
        let vertices = buffer(
            device,
            "world vertices",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let indices = buffer(
            device,
            "world indices",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
        Self {
            world,
            vertices,
            indices,
            count: mesh.indices.len() as u32,
            batches: mesh.batches,
            replacements: HashMap::new(),
        }
    }
    pub fn edit(&mut self, device: &wgpu::Device, cell: [i32; 3], material: u8) -> bool {
        if !editing::in_bounds(cell) || self.world.get(cell[0], cell[1], cell[2]) == material {
            return false;
        }
        self.world.set(cell[0], cell[1], cell[2], material);
        for chunk in editing::affected_chunks(cell) {
            let mesh = Mesh::build_chunk(&self.world, chunk);
            let replacement = if mesh.indices.is_empty() {
                None
            } else {
                Some(GpuChunk {
                    vertices: buffer(
                        device,
                        "edited chunk vertices",
                        bytemuck::cast_slice(&mesh.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: buffer(
                        device,
                        "edited chunk indices",
                        bytemuck::cast_slice(&mesh.indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                    count: mesh.indices.len() as u32,
                    center: World::meters(
                        chunk[0] * CHUNK as i32 + 16,
                        chunk[1] * CHUNK as i32 + 16,
                        chunk[2] * CHUNK as i32 + 16,
                    ),
                })
            };
            self.replacements.insert(chunk, replacement);
        }
        true
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
        let mut chunks = 0;
        let mut triangles = 0;
        for batch in &self.batches {
            if batch
                .chunk
                .is_some_and(|key| self.replacements.contains_key(&key))
                || frustum.is_some_and(|f| !f.contains_sphere(batch.center, batch.radius))
            {
                continue;
            }
            pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            chunks += 1;
            triangles += batch.indices.len() / 3;
        }
        for chunk in self.replacements.values().flatten() {
            if frustum.is_some_and(|f| !f.contains_sphere(chunk.center, 3.0_f32.sqrt())) {
                continue;
            }
            pass.set_vertex_buffer(0, chunk.vertices.slice(..));
            pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..chunk.count, 0, 0..1);
            chunks += 1;
            triangles += chunk.count as usize / 3;
        }
        (chunks, triangles)
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
