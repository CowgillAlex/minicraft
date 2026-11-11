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
    crate::materials::collides(material)
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
    let lo = [
        if world.infinite { min.x } else { min.x.max(0) },
        min.y.max(0),
        if world.infinite { min.z } else { min.z.max(0) },
    ];
    let hi = [
        if world.infinite {
            max.x
        } else {
            max.x.min(world::WIDTH - 1)
        },
        max.y.min(world.height() - 1),
        if world.infinite {
            max.z
        } else {
            max.z.min(world::DEPTH - 1)
        },
    ];
    for cy in lo[1].div_euclid(32)..=hi[1].div_euclid(32) {
        for cz in lo[2].div_euclid(32)..=hi[2].div_euclid(32) {
            for cx in lo[0].div_euclid(32)..=hi[0].div_euclid(32) {
                let key = [cx, cy, cz];
                let page = world.page(key);
                if (world.infinite && page.is_none())
                    || matches!(page,Some(crate::chunk::Chunk::Uniform(m)) if solid(*m))
                {
                    let min = World::meters(cx * 32, cy * 32, cz * 32);
                    boxes.push(Aabb {
                        min,
                        max: min + Vec3::splat(2.0),
                    });
                    continue;
                }
                if (page.is_none() || matches!(page, Some(crate::chunk::Chunk::Uniform(_))))
                    && !world.has_overlay(key)
                {
                    continue;
                }
                for y in lo[1].max(cy * 32)..=hi[1].min(cy * 32 + 31) {
                    for z in lo[2].max(cz * 32)..=hi[2].min(cz * 32 + 31) {
                        for x in lo[0].max(cx * 32)..=hi[0].min(cx * 32 + 31) {
                            if solid(world.get(x, y, z)) {
                                boxes.push(Aabb::voxel([x, y, z]));
                            }
                        }
                    }
                }
            }
        }
    }
    // The finite world has solid horizontal boundaries and a floor, but no flight ceiling.
    let half_x = world::WIDTH as f32 * VOXEL / 2.0;
    let half_z = world::DEPTH as f32 * VOXEL / 2.0;
    let boundaries = [
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
    ];
    for (i, (min, max)) in boundaries.into_iter().enumerate() {
        if world.infinite && i < 4 {
            continue;
        }
        if world.infinite {
            boxes.push(Aabb {
                min: Vec3::new(swept.min.x - 1.0, -10000.0, swept.min.z - 1.0),
                max: Vec3::new(swept.max.x + 1.0, 0.0, swept.max.z + 1.0),
            });
        } else {
            boxes.push(Aabb { min, max });
        }
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
