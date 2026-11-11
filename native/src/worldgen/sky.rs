use super::{preview::Preview, TerrainSampler};
use crate::world::{self, World};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct Column {
    blocked: i32,
    water: Vec<(i32, i32)>,
}
impl Column {
    fn add_water(&mut self, lo: i32, hi: i32) {
        if let Some(last) = self.water.last_mut() {
            if last.0 == hi + 1 {
                last.0 = lo;
                return;
            }
        }
        self.water.push((lo, hi));
    }

    fn at(&self, y: i32) -> u8 {
        if y <= self.blocked {
            return 0;
        }
        let water: i32 = self
            .water
            .iter()
            .map(|&(lo, hi)| (hi - y.max(lo) + 1).clamp(0, hi - lo + 1))
            .sum();
        255_u8.saturating_sub(water.min(255) as u8)
    }
}
#[derive(Default)]
pub struct Sky {
    columns: HashMap<[i32; 2], Column>,
}
impl Sky {
    pub fn missing(&mut self, origin: [i32; 3], size: [usize; 3]) -> Vec<[i32; 2]> {
        if self.columns.len() + size[0] * size[2] > 131072 {
            self.columns.clear();
        }
        let mut points = Vec::new();
        for z in 0..size[2] {
            for x in 0..size[0] {
                let p = [origin[0] + x as i32 * 4 + 2, origin[2] + z as i32 * 4 + 2];
                if !self.columns.contains_key(&p) {
                    points.push(p);
                }
            }
        }
        points
    }
    pub fn insert(&mut self, p: [i32; 2], column: Column) {
        self.columns.insert(p, column);
    }

    pub fn invalidate(&mut self, cells: &[[i32; 3]]) {
        for p in cells {
            self.columns
                .remove(&[p[0].div_euclid(4) * 4 + 2, p[2].div_euclid(4) * 4 + 2]);
        }
    }
    pub fn values(
        &mut self,
        world: &World,
        sampler: &mut TerrainSampler<'_>,
        point: [i32; 2],
        min_y: i32,
        len: usize,
    ) -> Vec<u8> {
        if self.columns.len() > 131072 {
            self.columns.clear();
        }
        let column = self
            .columns
            .entry(point)
            .or_insert_with(|| sample_column(world, sampler, point));
        (0..len)
            .map(|y| column.at(min_y + y as i32 * 4 + 2))
            .collect()
    }
}
pub(crate) fn sample_column(
    world: &World,
    sampler: &mut TerrainSampler<'_>,
    [x, z]: [i32; 2],
) -> Column {
    let preview = world.noise_preview.expect("procedural sky");
    let overrides = world.column_overrides(x, z);
    let mut column = Column {
        blocked: -1,
        water: Vec::new(),
    };
    let mut water = 0;
    let mut y = world.height() - 1;
    while y >= 0 {
        let lo = (y + preview.origin[1] * 16).div_euclid(128) * 128 - preview.origin[1] * 16;
        let lo = lo.max(0);
        let uniform = if overrides.range(lo..=y).next().is_none() {
            sampler.light_uniform(
                preview.point([x, lo, z]),
                preview.point([x, y, z]),
                preview.surfaces,
            )
        } else {
            None
        };
        match uniform {
            Some(world::AIR) => {
                y = lo - 1;
                continue;
            }
            Some(world::WATER) => {
                column.add_water(lo, y);
                water += y - lo + 1;
                if water >= 255 {
                    column.blocked = lo;
                    break;
                }
                y = lo - 1;
                continue;
            }
            Some(material) if world::opaque(material) => {
                column.blocked = y;
                break;
            }
            _ => {}
        }
        let mut stop = false;
        for yy in (lo..=y).rev() {
            let material = overrides.get(&yy).copied().unwrap_or_else(|| {
                sampler.light_material(preview.point([x, yy, z]), preview.surfaces)
            });
            if world::opaque(material) {
                column.blocked = yy;
                stop = true;
                break;
            }
            if material == world::WATER {
                column.add_water(yy, yy);
                water += 1;
            }
            if water >= 255 {
                column.blocked = yy;
                stop = true;
                break;
            }
        }
        if stop {
            break;
        }
        y = lo - 1;
    }
    column
}

pub fn surface(preview: Preview, sampler: &mut TerrainSampler<'_>, x: i32, z: i32) -> i32 {
    for cy in (0..preview.height_m * 16 / 32).rev() {
        let lo = cy * 32;
        let hi = lo + 31;
        match sampler.uniform_material(preview.point([x, lo, z]), preview.point([x, hi, z])) {
            Some(world::AIR | world::WATER) => continue,
            Some(material) if world::opaque(material) => return hi + 1,
            _ => {
                for y in (lo..=hi).rev() {
                    if world::opaque(sampler.material(preview.point([x, y, z]))) {
                        return y + 1;
                    }
                }
            }
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn column_scan_matches_fine_reference_including_negative_columns_and_water() {
        let preview = Preview::default();
        let generator = super::super::Generator::new(1).unwrap();
        let mut sampler = generator.sampler();
        let mut world = World::with_height(384 * 16);
        world.infinite = true;
        world.noise_preview = Some(preview);
        for [x, z] in [[2, 2], [-30, 18], [322, 322], [642, -94]] {
            let column = sample_column(&world, &mut sampler, [x, z]);
            let mut light = 255_u8;
            for y in (0..world.height()).rev() {
                let material = sampler.material(preview.point([x, y, z]));
                light = light.saturating_sub(crate::light_field::loss(material));
                if y % 4 == 2 {
                    assert_eq!(column.at(y), light, "at {x},{y},{z}");
                }
                if light == 0 {
                    break;
                }
            }
        }
    }
}
