use crate::materials::*;
use glam::DVec3;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Replace {
    Air = 1,
    Water = 2,
    Rock = 3,
    Any = 4,
}
#[derive(Clone, Copy)]
pub enum Shape {
    Ellipsoid {
        center: DVec3,
        radius: DVec3,
    },
    Branch {
        a: DVec3,
        b: DVec3,
        ra: f64,
        rb: f64,
    },
    Box {
        lo: DVec3,
        hi: DVec3,
    },
}
#[derive(Clone, Copy)]
pub struct Primitive {
    pub shape: Shape,
    pub material: u8,
    pub replace: Replace,
    pub priority: u8,
}
impl Primitive {
    pub fn ellipsoid(
        center: DVec3,
        radius: DVec3,
        material: u8,
        replace: Replace,
        priority: u8,
    ) -> Self {
        Self {
            shape: Shape::Ellipsoid { center, radius },
            material,
            replace,
            priority,
        }
    }
    pub fn branch(
        a: DVec3,
        b: DVec3,
        ra: f64,
        rb: f64,
        material: u8,
        replace: Replace,
        priority: u8,
    ) -> Self {
        Self {
            shape: Shape::Branch { a, b, ra, rb },
            material,
            replace,
            priority,
        }
    }
    pub fn cuboid(lo: DVec3, hi: DVec3, material: u8, priority: u8) -> Self {
        Self {
            shape: Shape::Box { lo, hi },
            material,
            replace: Replace::Any,
            priority,
        }
    }
    pub fn bounds(&self) -> (DVec3, DVec3) {
        match self.shape {
            Shape::Ellipsoid { center, radius } => (center - radius, center + radius),
            Shape::Branch { a, b, ra, rb } => {
                let r = DVec3::splat(ra.max(rb));
                (a.min(b) - r, a.max(b) + r)
            }
            Shape::Box { lo, hi } => (lo, hi),
        }
    }
    pub fn contains(&self, p: DVec3) -> bool {
        match self.shape {
            Shape::Ellipsoid { center, radius } => ((p - center) / radius).length_squared() <= 1.0,
            Shape::Branch { a, b, ra, rb } => {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                (p - a - ab * t).length_squared() <= (ra + (rb - ra) * t).powi(2)
            }
            Shape::Box { lo, hi } => p.cmpge(lo).all() && p.cmplt(hi).all(),
        }
    }
    pub fn raster(&self, origin: [i32; 3], paint: &mut [u16], priorities: &mut [u8]) {
        let (lo, hi) = self.bounds();
        let lo = (lo * 16.0).floor().as_ivec3().to_array();
        let hi = (hi * 16.0).ceil().as_ivec3().to_array();
        let start = std::array::from_fn::<_, 3, _>(|i| lo[i].max(origin[i]));
        let end = std::array::from_fn::<_, 3, _>(|i| hi[i].min(origin[i] + 32));
        if (0..3).any(|i| start[i] >= end[i]) {
            return;
        }
        let value = u16::from(self.material) | (u16::from(self.replace as u8) << 8);
        for y in start[1]..end[1] {
            for z in start[2]..end[2] {
                for x in start[0]..end[0] {
                    let index =
                        ((x - origin[0]) + 32 * ((z - origin[2]) + 32 * (y - origin[1]))) as usize;
                    if self.priority >= priorities[index]
                        && self.contains(
                            (DVec3::new(x as f64, y as f64, z as f64) + DVec3::splat(0.5)) / 16.0,
                        )
                    {
                        paint[index] = value;
                        priorities[index] = self.priority;
                    }
                }
            }
        }
    }
}
pub fn apply(paint: u16, base: u8) -> u8 {
    let material = paint as u8;
    match paint >> 8 {
        1 if base == AIR => material,
        2 if base == WATER => material,
        3 if matches!(base, STONE | DEEPSLATE) => material,
        4 => material,
        _ => base,
    }
}
