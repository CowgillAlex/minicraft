use crate::{
    camera::Camera,
    door_gpu::DoorGpu,
    doors::Door,
    editing::{self, Hit},
    ray::VoxelRay,
    world::{self, World},
};
use glam::Vec3;
use std::time::Instant;
#[derive(Clone, Copy)]
pub enum Target {
    Terrain(Hit),
    Door { id: usize, hit: Hit },
}
impl Target {
    pub fn hit(self, world: &World) -> Hit {
        match self {
            Self::Terrain(hit) => hit,
            Self::Door { id, hit } => Hit {
                cell: world.doors[id].world_cell(hit.cell),
                adjacent: hit.adjacent.map(|p| world.doors[id].world_cell(p)),
                distance: hit.distance,
            },
        }
    }
    pub fn material(self, world: &World) -> u8 {
        match self {
            Self::Terrain(hit) => world.get_static(hit.cell[0], hit.cell[1], hit.cell[2]),
            Self::Door { id, hit } => world.doors[id].get(hit.cell),
        }
    }
    pub fn outline(self, world: &World, gpu: &[DoorGpu]) -> [[f32; 3]; 24] {
        match self {
            Self::Terrain(hit) => editing::outline(hit),
            Self::Door { id, hit } => {
                let outline = editing::outline(hit);
                let offset = World::meters(0, 0, 0);
                outline.map(|p| {
                    world.doors[id]
                        .point(Vec3::from_array(p) - offset, gpu[id].angle(Instant::now()))
                        .to_array()
                })
            }
        }
    }
}
fn door_hit(door: &Door, angle: f32, camera: &Camera) -> Option<Hit> {
    let local = door.local_point(camera.position, angle);
    let direction = door.rotation(angle).inverse() * camera.direction();
    let mut adjacent = None;
    for (cell, distance) in VoxelRay::local(local, direction, editing::REACH) {
        if door.get(cell) != world::AIR {
            return Some(Hit {
                cell,
                adjacent,
                distance,
            });
        }
        adjacent = Some(cell);
    }
    None
}
pub fn cast(world: &World, gpu: &[DoorGpu], camera: &Camera) -> Option<Target> {
    let mut result =
        editing::raycast_static(world, camera.position, camera.direction(), editing::REACH)
            .map(Target::Terrain);
    let mut distance = result.map_or(f32::INFINITY, |t| t.hit(world).distance);
    for (id, door) in world.doors.iter().enumerate() {
        let angle = gpu
            .get(id)
            .map_or(door.angle(), |g| g.angle(Instant::now()));
        if let Some(hit) = door_hit(door, angle, camera) {
            if hit.distance < distance {
                distance = hit.distance;
                result = Some(Target::Door { id, hit });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_individual_voxels_during_a_swing_and_rays_pass_through_carved_holes() {
        let mut door = Door::prefab([320, 32, 320]);
        for angle in [0.0, 0.63, std::f32::consts::FRAC_PI_2] {
            let position = door.point(Vec3::new(8.5, 8.5, 12.0) * crate::world::VOXEL, angle);
            let target = door.point(Vec3::new(8.5, 8.5, 2.5) * crate::world::VOXEL, angle);
            let direction = (target - position).normalize();
            let camera = Camera {
                position,
                yaw: direction.z.atan2(direction.x),
                pitch: direction.y.asin(),
                ..Default::default()
            };
            let hit = door_hit(&door, angle, &camera).unwrap();
            assert_eq!(hit.cell, [8, 8, 2]);
        }
        door.set([8, 8, 1], world::AIR);
        door.set([8, 8, 2], world::AIR);
        let position = door.point(Vec3::new(8.5, 8.5, 12.0) * crate::world::VOXEL, 0.0);
        let camera = Camera {
            position,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            ..Default::default()
        };
        assert!(door_hit(&door, 0.0, &camera).is_none());
    }
}
