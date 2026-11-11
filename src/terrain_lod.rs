//! Distant mesh simplification keeps page borders and material interfaces fixed.
//! Source voxels remain exact; only the render representation changes.
use crate::{
    gpu_vertex::{pack, PackedMesh},
    mesh::{Mesh, Vertex},
    world::{World, VOXEL},
};
#[path = "mesh_conform.rs"]
mod conform;
use glam::Vec3;
use meshopt::{SimplifyOptions as Options, SimplifyVertex as Lock};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Level {
    #[default]
    Full,
    Medium,
    Far,
}
impl Level {
    pub fn desired(key: [i32; 3], eye: Vec3) -> Self {
        let distance = (crate::terrain_region::center(key).distance(eye)
            - 8.0 * 3.0_f32.sqrt() / 2.0)
            .max(0.0);
        if distance < 24.0 {
            Self::Full
        } else if distance < 56.0 {
            Self::Medium
        } else {
            Self::Far
        }
    }
    pub fn select(key: [i32; 3], eye: Vec3, previous: Option<Self>) -> Self {
        let distance = (crate::terrain_region::center(key).distance(eye)
            - 8.0 * 3.0_f32.sqrt() / 2.0)
            .max(0.0);
        match previous {
            Some(Self::Full) if distance < 28.0 => Self::Full,
            Some(Self::Medium) if (20.0..60.0).contains(&distance) => Self::Medium,
            Some(Self::Far) if distance >= 52.0 => Self::Far,
            _ => Self::desired(key, eye),
        }
    }
    pub fn error(self) -> f32 {
        match self {
            Self::Full => 0.0,
            Self::Medium => VOXEL * 0.5,
            Self::Far => VOXEL,
        }
    }
    fn divisor(self) -> usize {
        match self {
            Self::Full => 1,
            Self::Medium => 2,
            Self::Far => 4,
        }
    }
}
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct Key {
    position: [u32; 3],
    normal: [i8; 3],
    color: [u8; 3],
    ao: u8,
}
fn ao(v: &Vertex) -> f32 {
    let [u, w] = v.uv;
    (v.ao[0] * (1.0 - u) + v.ao[1] * u) * (1.0 - w) + (v.ao[3] * (1.0 - u) + v.ao[2] * u) * w
}
/// Reuse an exact source mesh when distance changes. Full-detail packing does
/// not clone the larger CPU vertex array; simplification owns its working copy.
pub fn reduce_cached(mesh: &Mesh, page: [i32; 3], level: Level) -> PackedMesh {
    if level == Level::Full || mesh.indices.is_empty() {
        return PackedMesh {
            vertices: pack(&mesh.vertices),
            indices: mesh.indices.clone(),
            transparent_indices: mesh.transparent_indices.clone(),
        };
    }
    reduce(
        Mesh {
            vertices: mesh.vertices.clone(),
            indices: mesh.indices.clone(),
            transparent_indices: mesh.transparent_indices.clone(),
            batches: Vec::new(),
            transparent_batches: Vec::new(),
        },
        page,
        level,
    )
}
pub fn reduce(mut mesh: Mesh, page: [i32; 3], level: Level) -> PackedMesh {
    if level == Level::Full || mesh.indices.is_empty() {
        return mesh.into();
    }
    conform::conform(&mut mesh);
    let mut groups = BTreeMap::<u8, Vec<u32>>::new();
    for face in mesh.indices.as_chunks::<3>().0 {
        let material = mesh.vertices[face[0] as usize].material as u8;
        groups.entry(material).or_default().extend_from_slice(face);
    }
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let min = World::meters(page[0] * 32, page[1] * 32, page[2] * 32).to_array();
    for (material, group) in groups {
        let mut lookup = HashMap::new();
        let mut positions = Vec::new();
        let mut attributes = Vec::new();
        let mut locks = Vec::new();
        let mut local = Vec::with_capacity(group.len());
        for index in &group {
            let v = mesh.vertices[*index as usize];
            let occlusion = ao(&v);
            let key = Key {
                position: v.position.map(f32::to_bits),
                normal: v.normal.map(|n| (n * 127.0).round() as i8),
                color: v.color.map(|c| (c * 255.0).round() as u8),
                ao: (occlusion * 255.0).round() as u8,
            };
            let next = positions.len() as u32;
            let index = *lookup.entry(key).or_insert_with(|| {
                positions.push(v.position);
                attributes.extend(v.normal);
                attributes.extend(v.color);
                attributes.push(occlusion);
                let border = (0..3).any(|a| {
                    (v.position[a] - min[a]).abs() < 1e-5
                        || (v.position[a] - min[a] - 2.0).abs() < 1e-5
                });
                locks.push(if border { Lock::Lock } else { Lock::None });
                next
            });
            local.push(index);
        }
        if local.len() >= 144 {
            let target = (local.len() / level.divisor() / 3) * 3;
            let count = meshopt::simplify_with_update(
                &mut local,
                &mut positions,
                &mut attributes,
                &[0.005, 0.005, 0.005, 0.04, 0.04, 0.04, 0.015],
                7 * 4,
                Some(&locks),
                target,
                level.error(),
                Options::LockBorder
                    | Options::ErrorAbsolute
                    | Options::Permissive
                    | Options::ErrorClamped,
                None,
            );
            local.truncate(count);
        }
        let mut remap = vec![u32::MAX; positions.len()];
        for i in local {
            let i = i as usize;
            if remap[i] == u32::MAX {
                remap[i] = vertices.len() as u32;
                let a = &attributes[i * 7..i * 7 + 7];
                let normal = Vec3::new(a[0], a[1], a[2]).normalize_or_zero();
                vertices.push(Vertex {
                    position: positions[i],
                    normal: normal.to_array(),
                    color: [a[3], a[4], a[5]],
                    ao: [a[6]; 4],
                    uv: [0.0; 2],
                    material: material as f32,
                });
            }
            indices.push(remap[i]);
        }
    }
    let mut packed = pack(&vertices);
    for vertex in &mut packed {
        vertex.coarse();
    }
    // Transparent surfaces retain their exact geometry, UVs and material flags.
    let mut transparent_indices = Vec::with_capacity(mesh.transparent_indices.len());
    let mut remap = HashMap::new();
    for i in mesh.transparent_indices {
        let index = *remap.entry(i).or_insert_with(|| {
            let index = packed.len() as u32;
            packed.extend(pack(&[mesh.vertices[i as usize]]));
            index
        });
        transparent_indices.push(index);
    }
    PackedMesh {
        vertices: packed,
        indices,
        transparent_indices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{self, World};
    use std::collections::BTreeSet;
    fn contour(mesh: &PackedMesh, page: [i32; 3]) -> BTreeSet<[i32; 3]> {
        let min = World::meters(page[0] * 32, page[1] * 32, page[2] * 32).to_array();
        let mut segments = BTreeSet::new();
        for face in mesh.indices.as_chunks::<3>().0 {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let p = mesh.vertices[face[a] as usize].position();
                let q = mesh.vertices[face[b] as usize].position();
                if (0..3).any(|i| {
                    [min[i], min[i] + 2.0]
                        .iter()
                        .any(|edge| (p[i] - edge).abs() < 1e-5 && (q[i] - edge).abs() < 1e-5)
                }) {
                    let p = p.map(|v| (v * 32.0).round() as i32);
                    let q = q.map(|v| (v * 32.0).round() as i32);
                    let steps = (0..3)
                        .map(|i| (q[i] - p[i]).unsigned_abs())
                        .max()
                        .unwrap()
                        .max(1) as i32;
                    for step in 0..=steps {
                        segments
                            .insert(std::array::from_fn(|i| p[i] + (q[i] - p[i]) * step / steps));
                    }
                }
            }
        }
        segments
    }
    #[test]
    fn distant_stairs_reduce_geometry_without_opening_page_seams() {
        let mut world = World::empty();
        for x in 240..304 {
            for z in 240..304 {
                for y in 16..32 + (x + z - 480) / 4 {
                    world.set(x, y, z, world::STONE);
                }
            }
        }
        let page = [8, 1, 8];
        let fine: PackedMesh = Mesh::build_chunk(&world, page).into();
        for level in [Level::Medium, Level::Far] {
            let coarse = reduce(Mesh::build_chunk(&world, page), page, level);
            println!(
                "LOD {level:?}: {} -> {} triangles, {} -> {} bytes",
                fine.indices.len() / 3,
                coarse.indices.len() / 3,
                fine.bytes(),
                coarse.bytes()
            );
            assert!(
                coarse.indices.len() < fine.indices.len() * 4 / 5,
                "LOD must reduce terrain geometry"
            );
            assert!(
                coarse.bytes() < fine.bytes(),
                "compact LOD vertices must reduce memory"
            );
            assert_eq!(
                contour(&fine, page),
                contour(&coarse, page),
                "page boundary segments must be identical"
            );
        }
    }
}

#[cfg(test)]
#[test]
fn lod_hysteresis_keeps_nearby_detail_and_avoids_boundary_chatter() {
    let key = [2, 10, 2];
    let center = crate::terrain_region::center(key);
    let eye = |distance| center + Vec3::X * (distance + 8.0 * 3.0_f32.sqrt() / 2.0);
    assert_eq!(
        Level::select(key, eye(26.0), Some(Level::Full)),
        Level::Full
    );
    assert_eq!(
        Level::select(key, eye(30.0), Some(Level::Full)),
        Level::Medium
    );
    assert_eq!(
        Level::select(key, eye(22.0), Some(Level::Medium)),
        Level::Medium
    );
    assert_eq!(Level::select(key, eye(18.0), Some(Level::Far)), Level::Full);
    assert_eq!(Level::select(key, eye(54.0), Some(Level::Far)), Level::Far);
}
