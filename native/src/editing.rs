use crate::world::{self, World, AIR, CHUNK, DEPTH, VOXEL, WIDTH};
use glam::{IVec3, Vec3};
use std::collections::HashSet;

pub const REACH: f32 = 8.0;
pub const PALETTE: [u8; 12] = [
    world::STONE,
    world::GRASS,
    world::DIRT,
    world::WOOD,
    world::LEAF,
    world::ROOF,
    world::WALL,
    world::SAND,
    world::ROAD,
    world::LIGHT,
    world::GLASS,
    world::HANDLE,
];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub cell: [i32; 3],
    pub adjacent: Option<[i32; 3]>,
    pub distance: f32,
}
pub use crate::ray::grid_position;
pub fn in_bounds(cell: [i32; 3], height: i32) -> bool {
    (0..WIDTH).contains(&cell[0]) && (0..height).contains(&cell[1]) && (0..DEPTH).contains(&cell[2])
}

/// Traverse voxel cells exactly, keeping the last empty cell for face-adjacent placement.
#[cfg(test)]
pub fn raycast(world: &World, origin: Vec3, direction: Vec3, reach: f32) -> Option<Hit> {
    cast_cells(origin, direction, reach, |p| world.get(p[0], p[1], p[2]))
}
pub fn raycast_static(world: &World, origin: Vec3, direction: Vec3, reach: f32) -> Option<Hit> {
    cast_cells(origin, direction, reach, |p| {
        world.get_static(p[0], p[1], p[2])
    })
}
fn cast_cells(
    origin: Vec3,
    direction: Vec3,
    reach: f32,
    get: impl Fn([i32; 3]) -> u8,
) -> Option<Hit> {
    let mut adjacent = None;
    for (p, distance) in crate::ray::VoxelRay::new(origin, direction, reach) {
        if get(p) != AIR {
            return Some(Hit {
                cell: p,
                adjacent,
                distance,
            });
        }
        adjacent = Some(p);
    }
    None
}

pub fn edit_cell(hit: Hit, camera: Vec3, place: bool, height: i32) -> Option<[i32; 3]> {
    let cell = if place { hit.adjacent? } else { hit.cell };
    if !in_bounds(cell, height)
        || (place && grid_position(camera).floor().as_ivec3().to_array() == cell)
    {
        return None;
    }
    Some(cell)
}

pub fn affected_world_chunks(world: &World, cell: [i32; 3]) -> Vec<[i32; 3]> {
    if !world.infinite {
        return affected_chunks(cell, world.height());
    }
    let mut keys = HashSet::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let p = (IVec3::from_array(cell) + IVec3::new(x, y, z)).to_array();
                if world.contains(p) {
                    keys.insert(p.map(|v| v.div_euclid(CHUNK as i32)));
                }
            }
        }
    }
    keys.into_iter().collect()
}
/// Faces and corner AO can depend on neighboring chunks, including diagonal ones.
pub fn affected_chunks(cell: [i32; 3], height: i32) -> Vec<[i32; 3]> {
    let mut chunks = HashSet::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let p = (IVec3::from_array(cell) + IVec3::new(x, y, z)).to_array();
                if in_bounds(p, height) {
                    chunks.insert(p.map(|v| v / CHUNK as i32));
                }
            }
        }
    }
    let mut chunks: Vec<_> = chunks.into_iter().collect();
    chunks.sort_unstable();
    chunks
}
pub fn material_name(material: u8) -> &'static str {
    match material {
        world::STONE => "STONE",
        world::GRASS => "GRASS",
        world::DIRT => "DIRT",
        world::WOOD => "WOOD",
        world::LEAF => "LEAVES",
        world::ROOF => "ROOF",
        world::WALL => "PLASTER",
        world::SAND => "SAND",
        world::ROAD => "ASPHALT",
        world::LIGHT => "LAMP",
        world::GLASS => "GLASS",
        world::HANDLE => "HANDLE",
        world::WATER => "WATER",
        world::FLOWER => "FLOWER",
        world::PAINT => "ROAD PAINT",
        _ => "UNKNOWN",
    }
}
pub fn outline(hit: Hit) -> [[f32; 3]; 24] {
    let min = World::meters(hit.cell[0], hit.cell[1], hit.cell[2]) - Vec3::splat(0.001);
    let size = VOXEL + 0.002;
    let corners: Vec<_> = (0..8)
        .map(|i| {
            min + Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32) * size
        })
        .collect();
    let edges = [
        (0, 1),
        (0, 2),
        (0, 4),
        (1, 3),
        (1, 5),
        (2, 3),
        (2, 6),
        (3, 7),
        (4, 5),
        (4, 6),
        (5, 7),
        (6, 7),
    ];
    std::array::from_fn(|i| {
        corners[if i % 2 == 0 {
            edges[i / 2].0
        } else {
            edges[i / 2].1
        }]
        .to_array()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn origin(x: i32, y: i32, z: i32) -> Vec3 {
        World::meters(x, y, z) + Vec3::splat(VOXEL * 0.5)
    }
    #[test]
    fn ray_hits_nearest_voxel_and_correct_placement_face() {
        let mut world = World::empty();
        world.set(8, 8, 8, world::STONE);
        world.set(10, 8, 8, world::WOOD);
        let hit = raycast(&world, origin(4, 8, 8), Vec3::X, 1.0).unwrap();
        assert_eq!(hit.cell, [8, 8, 8]);
        assert_eq!(hit.adjacent, Some([7, 8, 8]));
        assert!((hit.distance - 3.5 * VOXEL).abs() < 0.0001);
        world.set(8, 8, 8, AIR);
        assert_eq!(
            raycast(&world, origin(4, 8, 8), Vec3::X, 1.0).unwrap().cell,
            [10, 8, 8]
        );
    }
    #[test]
    fn negative_rays_and_world_boundary_work() {
        let mut world = World::empty();
        world.set(0, 8, 8, world::STONE);
        let hit = raycast(&world, origin(4, 8, 8), -Vec3::X, 1.0).unwrap();
        assert_eq!(hit.adjacent, Some([1, 8, 8]));
        assert_eq!(
            raycast(&world, origin(-2, 8, 8), Vec3::X, 1.0)
                .unwrap()
                .cell,
            [0, 8, 8]
        );
        assert!(raycast(&world, origin(4, 8, 8), -Vec3::X, 0.1).is_none());
        assert!(raycast(&world, origin(4, 8, 8), Vec3::ZERO, 1.0).is_none());
    }
    #[test]
    fn placement_never_occupies_camera_or_leaves_world() {
        let camera = origin(4, 8, 8);
        let hit = Hit {
            cell: [5, 8, 8],
            adjacent: Some([4, 8, 8]),
            distance: 0.03,
        };
        assert_eq!(edit_cell(hit, camera, true, world::HEIGHT), None);
        assert_eq!(
            edit_cell(hit, camera, false, world::HEIGHT),
            Some([5, 8, 8])
        );
        assert_eq!(
            edit_cell(
                Hit {
                    adjacent: Some([-1, 8, 8]),
                    ..hit
                },
                camera,
                true,
                world::HEIGHT
            ),
            None
        );
    }
    #[test]
    fn dirty_chunks_include_corner_ao_neighbors() {
        assert_eq!(
            affected_chunks([16, 16, 16], world::HEIGHT),
            vec![[0, 0, 0]]
        );
        let dirty = affected_chunks([32, 32, 32], world::HEIGHT);
        assert_eq!(dirty.len(), 8);
        assert!(dirty.contains(&[0, 0, 0]));
        assert!(dirty.contains(&[1, 1, 1]));
        assert_eq!(affected_chunks([0, 0, 0], world::HEIGHT), vec![[0, 0, 0]]);
    }
}
