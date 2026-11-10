use crate::{
    camera::Frustum,
    doors::Door,
    mesh::{Mesh, Vertex},
    world::{World, VOXEL},
};
use glam::Vec3;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

/// Door meshes are rebuilt only after voxel edits. Swings update their small vertex buffers.
pub struct DoorGpu {
    source: Vec<Vertex>,
    transformed: Vec<Vertex>,
    vertices: wgpu::Buffer,
    opaque: wgpu::Buffer,
    transparent: wgpu::Buffer,
    count: u32,
    transparent_count: u32,
    revision: u64,
    target: f32,
    start: f32,
    since: Instant,
    last_angle: f32,
    center: Vec3,
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    if bytes.is_empty() {
        return device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: 4,
            usage,
            mapped_at_creation: false,
        });
    }
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage,
    })
}
impl DoorGpu {
    pub fn new(device: &wgpu::Device, door: &Door) -> Self {
        let mut gpu = Self {
            source: Vec::new(),
            transformed: Vec::new(),
            vertices: buffer(
                device,
                "empty door",
                &[],
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            opaque: buffer(device, "empty door indices", &[], wgpu::BufferUsages::INDEX),
            transparent: buffer(
                device,
                "empty door transparent",
                &[],
                wgpu::BufferUsages::INDEX,
            ),
            count: 0,
            transparent_count: 0,
            revision: door.revision,
            target: door.angle(),
            start: door.angle(),
            since: Instant::now(),
            last_angle: door.angle(),
            center: Vec3::ZERO,
        };
        gpu.rebuild(device, door);
        gpu
    }
    fn rebuild(&mut self, device: &wgpu::Device, door: &Door) {
        let mut local = World::empty();
        for z in 0..4 {
            for y in 0..32 {
                for x in 0..16 {
                    let material = door.get([x, y, z]);
                    if material != 0 {
                        local.set(320 + x, y, 320 + z, material);
                    }
                }
            }
        }
        let mesh = Mesh::build_chunk(&local, [10, 0, 10]);
        self.source = mesh.vertices;
        self.transformed = self.source.clone();
        self.vertices = buffer(
            device,
            "editable door vertices",
            bytemuck::cast_slice(&self.source),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        self.opaque = buffer(
            device,
            "door opaque faces",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
        self.transparent = buffer(
            device,
            "door transparent faces",
            bytemuck::cast_slice(&mesh.transparent_indices),
            wgpu::BufferUsages::INDEX,
        );
        self.count = mesh.indices.len() as u32;
        self.transparent_count = mesh.transparent_indices.len() as u32;
        self.revision = door.revision;
    }
    pub fn angle(&self, now: Instant) -> f32 {
        let t = (now.duration_since(self.since).as_secs_f32() / 0.2).clamp(0.0, 1.0);
        self.start + (self.target - self.start) * t * t * (3.0 - 2.0 * t)
    }
    pub fn settle(&mut self) {
        self.since = Instant::now() - Duration::from_secs(1);
    }
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, door: &Door) -> bool {
        let now = Instant::now();
        if self.target != door.angle() {
            self.start = self.angle(now);
            self.target = door.angle();
            self.since = now;
        }
        let rebuilt = self.revision != door.revision || self.center == Vec3::ZERO;
        if rebuilt {
            self.rebuild(device, door);
        }
        let angle = self.angle(now);
        if !rebuilt && (angle - self.last_angle).abs() < 0.000001 {
            return false;
        }
        let rotation = door.rotation(angle);
        for (out, source) in self.transformed.iter_mut().zip(&self.source) {
            *out = *source;
            out.position = door
                .point(Vec3::from_array(source.position), angle)
                .to_array();
            out.normal = (rotation * Vec3::from_array(source.normal)).to_array();
        }
        if !self.transformed.is_empty() {
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&self.transformed));
        }
        self.center = door.point(Vec3::new(8.0, 16.0, 2.0) * VOXEL, angle);
        self.last_angle = angle;
        true
    }
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&Frustum>,
        transparent: bool,
    ) -> usize {
        let count = if transparent {
            self.transparent_count
        } else {
            self.count
        };
        if count == 0 || frustum.is_some_and(|f| !f.contains_sphere(self.center, 1.5)) {
            return 0;
        }
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(
            if transparent {
                self.transparent.slice(..)
            } else {
                self.opaque.slice(..)
            },
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..count, 0, 0..1);
        count as usize / 3
    }
}
