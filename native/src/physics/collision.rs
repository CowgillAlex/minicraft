use super::{Aabb, STEP_HEIGHT};
use crate::{
    ray::grid_position,
    world::{self, World, VOXEL},
};
use glam::Vec3;

pub struct Motion {
    pub delta: Vec3,
    pub grounded: bool,
}
fn solid(material: u8) -> bool {
    !matches!(material, world::AIR | world::WATER | world::FLOWER)
}
/// Gather only fine voxels within the swept body and optional step-up region.
fn obstacles(world: &World, body: Aabb, movement: Vec3, stepping: bool) -> Vec<Aabb> {
    let mut swept = body.swept(movement);
    if stepping {
        swept.max.y = swept.max.y.max(body.max.y + STEP_HEIGHT);
    }
    let min = grid_position(swept.min - Vec3::splat(0.001))
        .floor()
        .as_ivec3();
    let max = grid_position(swept.max + Vec3::splat(0.001))
        .floor()
        .as_ivec3();
    let mut boxes = Vec::new();
    for y in min.y.max(0)..=max.y.min(world::HEIGHT - 1) {
        for z in min.z.max(0)..=max.z.min(world::DEPTH - 1) {
            for x in min.x.max(0)..=max.x.min(world::WIDTH - 1) {
                if solid(world.get(x, y, z)) {
                    boxes.push(Aabb::voxel([x, y, z]));
                }
            }
        }
    }
    // The finite world has solid horizontal boundaries and a floor, but no flight ceiling.
    let half_x = world::WIDTH as f32 * VOXEL / 2.0;
    let half_z = world::DEPTH as f32 * VOXEL / 2.0;
    for (min, max) in [
        (
            Vec3::new(-10000.0, -10000.0, -10000.0),
            Vec3::new(-half_x, 10000.0, 10000.0),
        ),
        (Vec3::new(half_x, -10000.0, -10000.0), Vec3::splat(10000.0)),
        (
            Vec3::new(-10000.0, -10000.0, -10000.0),
            Vec3::new(10000.0, 10000.0, -half_z),
        ),
        (Vec3::new(-10000.0, -10000.0, half_z), Vec3::splat(10000.0)),
        (Vec3::splat(-10000.0), Vec3::new(10000.0, 0.0, 10000.0)),
    ] {
        boxes.push(Aabb { min, max });
    }
    boxes
}
fn axis_move(boxes: &[Aabb], body: &mut Aabb, axis: usize, mut amount: f32) -> f32 {
    for obstacle in boxes {
        amount = obstacle.clip(*body, axis, amount);
    }
    let mut delta = Vec3::ZERO;
    delta[axis] = amount;
    *body = body.translated(delta);
    amount
}
fn slide(boxes: &[Aabb], body: &mut Aabb, wanted: Vec3) -> Vec3 {
    let y = axis_move(boxes, body, 1, wanted.y);
    let x = axis_move(boxes, body, 0, wanted.x);
    let z = axis_move(boxes, body, 2, wanted.z);
    Vec3::new(x, y, z)
}
pub fn move_body(world: &World, body: Aabb, wanted: Vec3, was_grounded: bool) -> Motion {
    let stepping = was_grounded && wanted.y <= 0.0;
    let boxes = obstacles(world, body, wanted, stepping);
    let mut moved = body;
    let mut delta = slide(&boxes, &mut moved, wanted);
    let grounded = wanted.y < 0.0 && delta.y > wanted.y + 0.000001;
    if stepping && (delta.x - wanted.x).abs() + (delta.z - wanted.z).abs() > 0.000001 {
        let mut raised = body;
        let up = axis_move(&boxes, &mut raised, 1, STEP_HEIGHT);
        let x = axis_move(&boxes, &mut raised, 0, wanted.x);
        let z = axis_move(&boxes, &mut raised, 2, wanted.z);
        let down = axis_move(&boxes, &mut raised, 1, -up + wanted.y);
        if x * x + z * z > delta.x * delta.x + delta.z * delta.z + 0.0000001 {
            delta = Vec3::new(x, up + down, z);
            return Motion {
                delta,
                grounded: down > -up + wanted.y + 0.000001,
            };
        }
    }
    Motion { delta, grounded }
}
pub fn overlaps_solid(world: &World, body: Aabb) -> bool {
    obstacles(world, body, Vec3::ZERO, false)
        .into_iter()
        .any(|b| b.intersects(body))
}
