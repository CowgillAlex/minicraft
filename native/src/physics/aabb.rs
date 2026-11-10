use crate::world::{World, VOXEL};
use glam::Vec3;

pub const EPSILON: f32 = 0.00001;
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}
impl Aabb {
    pub fn voxel(cell: [i32; 3]) -> Self {
        let min = World::meters(cell[0], cell[1], cell[2]);
        Self {
            min,
            max: min + Vec3::splat(VOXEL),
        }
    }
    pub fn translated(self, offset: Vec3) -> Self {
        Self {
            min: self.min + offset,
            max: self.max + offset,
        }
    }
    /// The union of the starting box and its full movement, rather than just its endpoint.
    pub fn swept(self, movement: Vec3) -> Self {
        Self {
            min: self.min + movement.min(Vec3::ZERO),
            max: self.max + movement.max(Vec3::ZERO),
        }
    }
    pub fn intersects(self, other: Self) -> bool {
        (self.max.cmpgt(other.min + Vec3::splat(EPSILON))
            & self.min.cmplt(other.max - Vec3::splat(EPSILON)))
        .all()
    }
    /// Clip one movement component against this obstacle, leaving tangential motion free.
    pub fn clip(self, moving: Self, axis: usize, amount: f32) -> f32 {
        for other in 0..3 {
            if other != axis
                && (moving.max[other] <= self.min[other] + EPSILON
                    || moving.min[other] >= self.max[other] - EPSILON)
            {
                return amount;
            }
        }
        if amount > 0.0 && moving.max[axis] <= self.min[axis] + EPSILON {
            amount.min((self.min[axis] - moving.max[axis] - EPSILON).max(0.0))
        } else if amount < 0.0 && moving.min[axis] >= self.max[axis] - EPSILON {
            amount.max((self.max[axis] - moving.min[axis] + EPSILON).min(0.0))
        } else {
            amount
        }
    }
}
