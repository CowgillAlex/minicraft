pub mod recognition;
#[cfg(test)]
mod tests;

use crate::{
    physics::Aabb,
    world::{self, World, VOXEL},
};
use glam::Vec3;
use std::sync::Arc;

pub const SIZE: [i32; 3] = [16, 32, 4];
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Ready,
    FrameBroken,
    HandleMissing,
}
#[derive(Clone)]
pub struct Door {
    pub origin: [i32; 3],
    pub facing: i32,
    pub hinge_right: bool,
    pub open: bool,
    pub revision: u64,
    cells: Arc<[u8]>,
}
pub fn rotate(p: [i32; 3], quarter: i32) -> [i32; 3] {
    match quarter.rem_euclid(4) {
        0 => p,
        1 => [p[2], p[1], -p[0]],
        2 => [-p[0], p[1], -p[2]],
        _ => [-p[2], p[1], p[0]],
    }
}
fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn inside(p: [i32; 3]) -> bool {
    (0..3).all(|axis| p[axis] >= 0 && p[axis] < SIZE[axis])
}
fn index(p: [i32; 3]) -> usize {
    (p[0] + SIZE[0] * (p[1] + SIZE[1] * p[2])) as usize
}
impl Door {
    pub fn new(origin: [i32; 3], facing: i32, hinge_right: bool, cells: Vec<u8>) -> Self {
        assert_eq!(cells.len(), SIZE.iter().product::<i32>() as usize);
        Self {
            origin,
            facing,
            hinge_right,
            open: false,
            revision: 0,
            cells: Arc::from(cells),
        }
    }
    pub fn prefab(origin: [i32; 3]) -> Self {
        let mut door = Self::new(
            origin,
            0,
            false,
            vec![world::AIR; SIZE.iter().product::<i32>() as usize],
        );
        for y in 0..SIZE[1] {
            for x in 0..SIZE[0] {
                for z in 1..=2 {
                    let rim = x < 2 || x >= SIZE[0] - 2 || y < 2 || y >= SIZE[1] - 2;
                    let material = if rim || y < 15 || y == 22 {
                        world::WOOD
                    } else {
                        world::GLASS
                    };
                    door.set([x, y, z], material);
                }
            }
        }
        door.set([14, 15, 3], world::HANDLE);
        door
    }
    pub fn get(&self, p: [i32; 3]) -> u8 {
        if inside(p) {
            self.cells[index(p)]
        } else {
            world::AIR
        }
    }
    pub fn set(&mut self, p: [i32; 3], material: u8) -> bool {
        if !inside(p) || self.get(p) == material {
            return false;
        }
        Arc::make_mut(&mut self.cells)[index(p)] = material;
        self.revision += 1;
        true
    }
    pub fn status(&self) -> Status {
        for y in 0..SIZE[1] {
            for x in 0..SIZE[0] {
                if (x == 0 || x == SIZE[0] - 1 || y == 0 || y == SIZE[1] - 1)
                    && (self.get([x, y, 1]) != world::WOOD || self.get([x, y, 2]) != world::WOOD)
                {
                    return Status::FrameBroken;
                }
            }
        }
        let handle = (0..SIZE[1]).any(|y| {
            (0..SIZE[0]).any(|x| {
                (x < 2 || x >= SIZE[0] - 2)
                    && ((self.get([x, y, 3]) == world::HANDLE
                        && self.get([x, y, 2]) == world::WOOD)
                        || (self.get([x, y, 0]) == world::HANDLE
                            && self.get([x, y, 1]) == world::WOOD))
            })
        });
        if handle {
            Status::Ready
        } else {
            Status::HandleMissing
        }
    }
    pub fn angle(&self) -> f32 {
        if self.open {
            if self.hinge_right {
                -std::f32::consts::FRAC_PI_2
            } else {
                std::f32::consts::FRAC_PI_2
            }
        } else {
            0.0
        }
    }
    fn pivot(&self) -> Vec3 {
        Vec3::new(
            if self.hinge_right {
                SIZE[0] as f32
            } else {
                0.0
            },
            0.0,
            1.0,
        ) * VOXEL
    }
    pub fn point(&self, local: Vec3, angle: f32) -> Vec3 {
        let base = glam::Quat::from_rotation_y(self.facing as f32 * std::f32::consts::FRAC_PI_2);
        World::meters(self.origin[0], self.origin[1], self.origin[2])
            + base * (self.pivot() + glam::Quat::from_rotation_y(angle) * (local - self.pivot()))
    }
    pub fn local_point(&self, world: Vec3, angle: f32) -> Vec3 {
        let base = glam::Quat::from_rotation_y(-self.facing as f32 * std::f32::consts::FRAC_PI_2);
        self.pivot()
            + glam::Quat::from_rotation_y(-angle)
                * (base * (world - World::meters(self.origin[0], self.origin[1], self.origin[2]))
                    - self.pivot())
    }
    pub fn rotation(&self, angle: f32) -> glam::Quat {
        glam::Quat::from_rotation_y(self.facing as f32 * std::f32::consts::FRAC_PI_2 + angle)
    }
    pub fn world_cell(&self, local: [i32; 3]) -> [i32; 3] {
        let point = self.point(
            Vec3::from_array(local.map(|v| v as f32)) * VOXEL + Vec3::splat(VOXEL * 0.5),
            self.angle(),
        );
        crate::ray::grid_position(point)
            .floor()
            .as_ivec3()
            .to_array()
    }
    pub fn local_cell(&self, cell: [i32; 3]) -> Option<[i32; 3]> {
        let point = World::meters(cell[0], cell[1], cell[2]) + Vec3::splat(VOXEL * 0.5);
        let p = (self.local_point(point, self.angle()) / VOXEL)
            .floor()
            .as_ivec3()
            .to_array();
        inside(p).then_some(p)
    }
    pub fn occupied(&self) -> impl Iterator<Item = ([i32; 3], u8)> + '_ {
        (0..SIZE[2]).flat_map(move |z| {
            (0..SIZE[1]).flat_map(move |y| {
                (0..SIZE[0]).filter_map(move |x| {
                    let p = [x, y, z];
                    let material = self.get(p);
                    (material != world::AIR).then(|| (self.world_cell(p), material))
                })
            })
        })
    }
    pub fn blocked_by(&self, body: Aabb) -> bool {
        self.occupied()
            .any(|(cell, _)| Aabb::voxel(cell).intersects(body))
    }
    pub fn closed_cell(&self, p: [i32; 3]) -> [i32; 3] {
        // Rotate cell centres so negative axes remain on the voxel grid.
        let center = rotate(p.map(|v| v * 2 + 1), self.facing);
        add(self.origin, center.map(|v| (v - 1).div_euclid(2)))
    }
}
