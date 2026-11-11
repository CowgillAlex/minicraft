use crate::mesh::Vertex;
use bytemuck::{Pod, Zeroable};

/// Positions retain full precision. Bounded surface attributes use normalized GPU formats.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuVertex {
    position: [f32; 3],
    normal: [i8; 4],
    color: [u8; 4],
    ao: [u8; 4],
    uv: [u16; 2],
}
impl GpuVertex {
    #[cfg(test)]
    pub fn position(&self) -> [f32; 3] {
        self.position
    }
    pub fn coarse(&mut self) {
        self.normal[3] = 127;
    }
}
pub fn pack(vertices: &[Vertex]) -> Vec<GpuVertex> {
    vertices
        .iter()
        .map(|v| GpuVertex {
            position: v.position,
            normal: [
                (v.normal[0] * 127.0).round() as i8,
                (v.normal[1] * 127.0).round() as i8,
                (v.normal[2] * 127.0).round() as i8,
                0,
            ],
            color: [
                (v.color[0] * 255.0).round() as u8,
                (v.color[1] * 255.0).round() as u8,
                (v.color[2] * 255.0).round() as u8,
                v.material as u8,
            ],
            ao: v.ao.map(|a| (a * 255.0).round() as u8),
            uv: v.uv.map(|a| (a * 65535.0).round() as u16),
        })
        .collect()
}

/// Static streamed geometry is packed on the terrain worker, before GPU upload.
pub struct PackedMesh {
    pub vertices: Vec<GpuVertex>,
    pub indices: Vec<u32>,
    pub transparent_indices: Vec<u32>,
}
impl PackedMesh {
    pub fn bytes(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<GpuVertex>()
            + (self.indices.len() + self.transparent_indices.len()) * 4
    }
}
impl From<crate::mesh::Mesh> for PackedMesh {
    fn from(mesh: crate::mesh::Mesh) -> Self {
        Self {
            vertices: pack(&mesh.vertices),
            indices: mesh.indices,
            transparent_indices: mesh.transparent_indices,
        }
    }
}
