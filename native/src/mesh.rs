use crate::world::{self, World, AIR, CHUNK, WATER};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub ao: [f32; 4],
    pub uv: [f32; 2],
    pub material: f32,
}
pub struct Batch {
    pub chunk: Option<[i32; 3]>,
    pub indices: std::ops::Range<u32>,
    pub center: Vec3,
    pub radius: f32,
}
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct Face {
    material: u8,
    ao: [u8; 4],
}

impl Mesh {
    pub fn build(world: &World) -> Self {
        let mut mesh = Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
        };
        for cy in 0..world::NY {
            for cz in 0..world::NZ {
                for cx in 0..world::NX {
                    if !world.chunk_exists(cx, cy, cz) {
                        continue;
                    }
                    let start = mesh.indices.len() as u32;
                    mesh.chunk(
                        world,
                        [
                            cx as i32 * CHUNK as i32,
                            cy as i32 * CHUNK as i32,
                            cz as i32 * CHUNK as i32,
                        ],
                    );
                    if mesh.indices.len() as u32 > start {
                        let center = World::meters(
                            cx as i32 * CHUNK as i32 + 16,
                            cy as i32 * CHUNK as i32 + 16,
                            cz as i32 * CHUNK as i32 + 16,
                        );
                        mesh.batches.push(Batch {
                            chunk: Some([cx as i32, cy as i32, cz as i32]),
                            indices: start..mesh.indices.len() as u32,
                            center,
                            radius: 3.0_f32.sqrt(),
                        });
                    }
                }
            }
        }
        mesh
    }
    pub fn build_chunk(world: &World, chunk: [i32; 3]) -> Self {
        let mut mesh = Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
        };
        if world.chunk_exists(chunk[0] as usize, chunk[1] as usize, chunk[2] as usize) {
            mesh.chunk(world, chunk.map(|v| v * CHUNK as i32));
        }
        mesh
    }
    pub fn add_horizon(&mut self) {
        let start = self.indices.len() as u32;
        let base = self.vertices.len() as u32;
        for position in [
            [-500.0, 0.0, -500.0],
            [500.0, 0.0, -500.0],
            [500.0, 0.0, 500.0],
            [-500.0, 0.0, 500.0],
        ] {
            self.vertices.push(Vertex {
                position,
                normal: [0.0, 1.0, 0.0],
                color: world::color(WATER),
                ao: [1.0; 4],
                uv: [0.0; 2],
                material: WATER as f32,
            });
        }
        self.indices.extend([0, 2, 1, 0, 3, 2].map(|i| base + i));
        self.batches.push(Batch {
            chunk: None,
            indices: start..start + 6,
            center: Vec3::ZERO,
            radius: 710.0,
        });
    }
    fn chunk(&mut self, world: &World, origin: [i32; 3]) {
        let n = CHUNK as i32;
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for sign in [-1, 1] {
                for slice in 0..n {
                    let mut mask = [Face::default(); CHUNK * CHUNK];
                    for j in 0..n {
                        for i in 0..n {
                            let mut p = origin;
                            p[axis] += slice;
                            p[u] += i;
                            p[v] += j;
                            let material = world.get(p[0], p[1], p[2]);
                            if material == AIR {
                                continue;
                            }
                            let mut neighbor = p;
                            neighbor[axis] += sign;
                            if world.get(neighbor[0], neighbor[1], neighbor[2]) != AIR {
                                continue;
                            }
                            let mut ao = [3; 4];
                            if material != WATER {
                                for (corner, (a, b)) in
                                    [(-1, -1), (1, -1), (1, 1), (-1, 1)].iter().enumerate()
                                {
                                    let sample = |du: i32, dv: i32| {
                                        let mut q = neighbor;
                                        q[u] += du;
                                        q[v] += dv;
                                        let m = world.get(q[0], q[1], q[2]);
                                        m != AIR && m != WATER
                                    };
                                    let s1 = sample(*a, 0);
                                    let s2 = sample(0, *b);
                                    let diagonal = sample(*a, *b);
                                    ao[corner] = if s1 && s2 {
                                        0
                                    } else {
                                        3 - u8::from(s1) - u8::from(s2) - u8::from(diagonal)
                                    };
                                }
                            }
                            mask[(i + j * n) as usize] = Face { material, ao };
                        }
                    }
                    for j in 0..n {
                        let mut i = 0;
                        while i < n {
                            let face = mask[(i + j * n) as usize];
                            if face.material == AIR {
                                i += 1;
                                continue;
                            }
                            let mergeable = face.ao.iter().all(|value| *value == face.ao[0]);
                            let mut w = 1;
                            while mergeable && i + w < n && mask[(i + w + j * n) as usize] == face {
                                w += 1;
                            }
                            let mut h = 1;
                            'height: while mergeable && j + h < n {
                                for k in 0..w {
                                    if mask[(i + k + (j + h) * n) as usize] != face {
                                        break 'height;
                                    }
                                }
                                h += 1;
                            }
                            let mut p = origin;
                            p[axis] += slice + i32::from(sign > 0);
                            p[u] += i;
                            p[v] += j;
                            let mut du = [0; 3];
                            du[u] = w;
                            let mut dv = [0; 3];
                            dv[v] = h;
                            let points = [p, add(p, du), add(add(p, du), dv), add(p, dv)];
                            let base = self.vertices.len() as u32;
                            let mut normal = [0.0; 3];
                            normal[axis] = sign as f32;
                            for (corner, point) in points.iter().enumerate() {
                                self.vertices.push(Vertex {
                                    position: World::meters(point[0], point[1], point[2])
                                        .to_array(),
                                    normal,
                                    color: world::color(face.material),
                                    ao: face.ao.map(|value| 0.38 + value as f32 * (0.62 / 3.0)),
                                    uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]][corner],
                                    material: face.material as f32,
                                });
                            }
                            let triangles = if sign > 0 {
                                [0, 1, 2, 0, 2, 3]
                            } else {
                                [0, 2, 1, 0, 3, 2]
                            };
                            self.indices.extend(triangles.map(|index| base + index));
                            for yy in 0..h {
                                for xx in 0..w {
                                    mask[(i + xx + (j + yy) * n) as usize] = Face::default();
                                }
                            }
                            i += w;
                        }
                    }
                }
            }
        }
    }
}
fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_cuboid_merges_to_six_quads() {
        let mut world = World::empty();
        for x in 3..12 {
            for y in 3..12 {
                for z in 3..12 {
                    world.set(x, y, z, world::STONE);
                }
            }
        }
        let mesh = Mesh::build(&world);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = mesh.vertices[triangle[0] as usize];
            let b = mesh.vertices[triangle[1] as usize];
            let c = mesh.vertices[triangle[2] as usize];
            assert!(
                (Vec3::from(b.position) - Vec3::from(a.position))
                    .cross(Vec3::from(c.position) - Vec3::from(a.position))
                    .dot(Vec3::from(a.normal))
                    > 0.0
            );
        }
    }
    #[test]
    fn adjoining_chunks_have_no_internal_faces() {
        let mut world = World::empty();
        world.set(31, 4, 4, world::STONE);
        world.set(32, 4, 4, world::STONE);
        let mesh = Mesh::build(&world);
        assert_eq!(mesh.indices.len(), 60);
        assert!(!mesh
            .vertices
            .iter()
            .any(
                |v| (v.position[0] - World::meters(32, 0, 0).x).abs() < 0.0001
                    && v.normal[0] != 0.0
            ));
    }
    #[test]
    fn negative_coordinates_are_empty_and_updates_track_occupancy() {
        let mut world = World::empty();
        world.set(-1, 0, 0, world::STONE);
        assert_eq!(world.voxels, 0);
        world.set(1, 2, 3, world::STONE);
        world.set(1, 2, 3, world::WOOD);
        assert_eq!(world.voxels, 1);
        world.set(1, 2, 3, AIR);
        assert_eq!(world.voxels, 0);
        assert_eq!(world.get(-1, 0, 0), AIR);
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    #[test]
    fn removing_a_boundary_voxel_reveals_the_neighbor_face() {
        let mut world = World::empty();
        world.set(31, 8, 8, world::STONE);
        world.set(32, 8, 8, world::STONE);
        assert_eq!(Mesh::build_chunk(&world, [1, 0, 0]).indices.len(), 30);
        world.set(31, 8, 8, AIR);
        assert!(Mesh::build_chunk(&world, [0, 0, 0]).indices.is_empty());
        assert_eq!(Mesh::build_chunk(&world, [1, 0, 0]).indices.len(), 36);
        world.set(31, 8, 8, world::STONE);
        assert_eq!(Mesh::build_chunk(&world, [1, 0, 0]).indices.len(), 30);
    }
    #[test]
    fn diagonal_edits_update_neighbor_ambient_occlusion() {
        let mut world = World::empty();
        world.set(31, 31, 31, world::STONE);
        world.set(32, 32, 32, world::STONE);
        let before = Mesh::build_chunk(&world, [0, 0, 0]);
        assert!(before
            .vertices
            .iter()
            .any(|vertex| vertex.ao.iter().any(|value| *value < 0.99)));
        world.set(32, 32, 32, AIR);
        let after = Mesh::build_chunk(&world, [0, 0, 0]);
        assert!(after
            .vertices
            .iter()
            .all(|vertex| vertex.ao.iter().all(|value| *value > 0.99)));
    }
}

#[cfg(test)]
mod occlusion_tests {
    use super::*;
    #[test]
    fn corner_occlusion_is_local_instead_of_stretched_across_merged_faces() {
        let mut world = World::empty();
        for x in 3..14 {
            for z in 3..14 {
                world.set(x, 3, z, world::STONE);
            }
        }
        world.set(8, 4, 8, world::STONE);
        let mesh = Mesh::build(&world);
        let mut shaded_faces = 0;
        for quad in mesh.vertices.as_chunks::<4>().0 {
            let ao = quad[0].ao;
            if ao.iter().any(|value| (*value - ao[0]).abs() > 0.001) {
                shaded_faces += 1;
                let min = quad.iter().fold(Vec3::splat(f32::INFINITY), |min, v| {
                    min.min(Vec3::from(v.position))
                });
                let max = quad.iter().fold(Vec3::splat(f32::NEG_INFINITY), |max, v| {
                    max.max(Vec3::from(v.position))
                });
                assert!((max - min).max_element() <= world::VOXEL + 0.0001);
                assert_eq!(quad[0].uv, [0.0, 0.0]);
                assert_eq!(quad[2].uv, [1.0, 1.0]);
                assert!(quad.iter().all(|vertex| vertex.ao == ao));
            }
        }
        assert!(shaded_faces > 0);
    }
}
