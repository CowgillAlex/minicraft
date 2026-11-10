use crate::world::{DEPTH, VOXEL, WIDTH};
use glam::{IVec3, Vec3};

pub fn grid_position(position: Vec3) -> Vec3 {
    position / VOXEL + Vec3::new((WIDTH / 2) as f32, 0.0, (DEPTH / 2) as f32)
}
/// Exact cell traversal shared by picking, light transport and shadow checks.
pub struct VoxelRay {
    cell: IVec3,
    step: IVec3,
    next: Vec3,
    delta: Vec3,
    distance: f32,
    reach: f32,
}
impl VoxelRay {
    pub fn new(origin: Vec3, direction: Vec3, reach: f32) -> Self {
        Self::from_grid(grid_position(origin), direction, reach)
    }
    pub fn local(origin: Vec3, direction: Vec3, reach: f32) -> Self {
        Self::from_grid(origin / VOXEL, direction, reach)
    }
    fn from_grid(origin: Vec3, direction: Vec3, reach: f32) -> Self {
        let direction = direction.normalize_or_zero();
        let valid = direction != Vec3::ZERO
            && direction.is_finite()
            && origin.is_finite()
            && reach.is_finite()
            && reach >= 0.0;
        let cell = origin.floor().as_ivec3();
        let mut next = Vec3::splat(f32::INFINITY);
        let mut delta = next;
        for axis in 0..3 {
            if direction[axis] != 0.0 {
                let boundary = (cell[axis] + i32::from(direction[axis] > 0.0)) as f32;
                next[axis] = ((boundary - origin[axis]) * VOXEL / direction[axis]).max(0.0);
                delta[axis] = VOXEL / direction[axis].abs();
            }
        }
        Self {
            cell,
            step: direction.signum().as_ivec3(),
            next,
            delta,
            distance: 0.0,
            reach: if valid { reach } else { -1.0 },
        }
    }
}
impl Iterator for VoxelRay {
    type Item = ([i32; 3], f32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.distance > self.reach {
            return None;
        }
        let result = (self.cell.to_array(), self.distance);
        let axis = if self.next.x <= self.next.y && self.next.x <= self.next.z {
            0
        } else if self.next.y <= self.next.z {
            1
        } else {
            2
        };
        self.distance = self.next[axis];
        self.cell[axis] += self.step[axis];
        self.next[axis] += self.delta[axis];
        Some(result)
    }
}
