//! Seeded feature recipes are independent of page loading order. Each page
//! rasterizes only intersecting primitives; all point/optical queries share it.
mod aquatic;
pub mod ecology;
mod geometry;
mod ores;
mod placement;
mod plants;
use placement::{cave_area, surface_area};
mod query;
pub mod showcase;
mod structures;
#[cfg(test)]
mod tests;
mod trees;
use super::{random::Random, TerrainSampler};
use crate::{chunk::Chunk, materials::*};
use ecology::Habitat;
use geometry::{Primitive, Replace};
use glam::DVec3 as V;
use std::{collections::HashMap, sync::Arc};

pub struct Rng(Random);
impl Rng {
    pub fn new(seed: i64, name: &str, key: [i32; 3]) -> Self {
        Self(Random::new(seed).named(&format!(
            "minicraft:{name}/{}/{}/{}",
            key[0], key[1], key[2]
        )))
    }
    pub fn unit(&mut self) -> f64 {
        self.0.double()
    }
    pub fn signed(&mut self) -> f64 {
        self.unit() * 2. - 1.
    }
    pub fn index(&mut self, n: usize) -> usize {
        self.0.int(n as u32)
    }
}
#[derive(Default)]
pub struct Cache {
    areas: HashMap<[i32; 2], Arc<Vec<Primitive>>>,
    cave: HashMap<[i32; 3], Arc<Vec<Primitive>>>,
    ores: HashMap<[i32; 3], Arc<Vec<Primitive>>>,
    optical_blocks: HashMap<[i32; 3], Arc<Vec<Primitive>>>,
    blocks: HashMap<[i32; 3], Arc<Vec<Primitive>>>,
    pages: HashMap<[i32; 3], Arc<Overlay>>,
    last: Option<([i32; 3], Arc<Overlay>)>,
}
pub struct Overlay {
    cells: Vec<(u16, u16)>,
}
impl Overlay {
    pub fn apply(&self, data: &mut Chunk) {
        for &(i, paint) in &self.cells {
            let base = data.get(i as usize);
            let m = geometry::apply(paint, base);
            if m != base {
                data.set(i as usize, m);
            }
        }
        data.compact();
    }
    pub fn at(&self, index: usize, base: u8) -> u8 {
        self.cells
            .binary_search_by_key(&(index as u16), |v| v.0)
            .map_or(base, |i| geometry::apply(self.cells[i].1, base))
    }
}
impl Cache {
    pub fn page(&mut self, sampler: &mut TerrainSampler<'_>, key: [i32; 3]) -> Arc<Overlay> {
        if let Some((k, p)) = &self.last {
            if *k == key {
                return p.clone();
            }
        }
        if let Some(p) = self.pages.get(&key) {
            let p = p.clone();
            self.last = Some((key, p.clone()));
            return p;
        }
        if self.pages.len() >= 512 {
            self.pages.clear();
        }
        let origin = key.map(|v| v * 32);
        let recipes = self.block(sampler, key.map(|v| v.div_euclid(4)), false);
        let mut paint = vec![0_u16; 32768];
        let mut priority = vec![0_u8; 32768];
        for p in recipes.iter() {
            p.raster(origin, &mut paint, &mut priority);
        }
        let cells: Vec<_> = paint
            .into_iter()
            .enumerate()
            .filter(|(_, v)| *v != 0)
            .map(|(i, v)| (i as u16, v))
            .collect();
        let result = Arc::new(Overlay { cells });
        self.pages.insert(key, result.clone());
        self.last = Some((key, result.clone()));
        result
    }
    fn block(
        &mut self,
        sampler: &mut TerrainSampler<'_>,
        key: [i32; 3],
        optical: bool,
    ) -> Arc<Vec<Primitive>> {
        if optical {
            if let Some(p) = self.optical_blocks.get(&key) {
                return p.clone();
            }
        }
        if let Some(p) = self.blocks.get(&key) {
            return p.clone();
        }
        if self.blocks.len() >= 2048 {
            self.blocks.clear();
        }
        let seed = sampler.seed;
        let min = key.map(|v| v as f64 * 8.);
        let max = min.map(|v| v + 8.);
        let mut selected = Vec::new();
        // A feature's horizontal reach is bounded to eight metres from its cell.
        for z in ((min[2] - 8.) / 8.).floor() as i32..=((max[2] + 8.) / 8.).floor() as i32 {
            for x in ((min[0] - 8.) / 8.).floor() as i32..=((max[0] + 8.) / 8.).floor() as i32 {
                let k = [x, z];
                let recipes = if let Some(p) = self.areas.get(&k) {
                    p.clone()
                } else {
                    if self.areas.len() >= 128 {
                        self.areas.clear();
                    }
                    let p = Arc::new(surface_area(sampler, k));
                    self.areas.insert(k, p.clone());
                    p
                };
                for primitive in recipes.iter() {
                    if intersects(primitive, min, max) {
                        selected.push(*primitive);
                    }
                }
            }
        }
        if !optical {
            // Ore veins originate once per 16m column, with original metre-scale counts.
            for z in (min[2] / 16.).floor() as i32 - 1..=(max[2] / 16.).floor() as i32 + 1 {
                for x in (min[0] / 16.).floor() as i32 - 1..=(max[0] / 16.).floor() as i32 + 1 {
                    let k = [x, (min[1] / 16.).floor() as i32, z];
                    let recipes = if let Some(p) = self.ores.get(&k) {
                        p.clone()
                    } else {
                        if self.ores.len() >= 512 {
                            self.ores.clear();
                        }
                        let p = Arc::new(ores::column(sampler, k, seed));
                        self.ores.insert(k, p.clone());
                        p
                    };
                    for primitive in recipes.iter() {
                        if intersects(primitive, min, max) {
                            selected.push(*primitive);
                        }
                    }
                }
            }
        }
        if min[1] < 60. {
            for y in (min[1] / 8.).floor() as i32 - 1..=(max[1] / 8.).floor() as i32 {
                for z in (min[2] / 8.).floor() as i32..=(max[2] / 8.).floor() as i32 {
                    for x in (min[0] / 8.).floor() as i32..=(max[0] / 8.).floor() as i32 {
                        let k = [x, y, z];
                        let recipes = if let Some(p) = self.cave.get(&k) {
                            p.clone()
                        } else {
                            if self.cave.len() >= 1024 {
                                self.cave.clear();
                            }
                            let p = Arc::new(cave_area(sampler, k));
                            self.cave.insert(k, p.clone());
                            p
                        };
                        for primitive in recipes.iter() {
                            if intersects(primitive, min, max) {
                                selected.push(*primitive);
                            }
                        }
                    }
                }
            }
        }

        let result = Arc::new(selected);
        if optical {
            if self.optical_blocks.len() >= 2048 {
                self.optical_blocks.clear();
            }
            self.optical_blocks.insert(key, result.clone());
        } else {
            self.blocks.insert(key, result.clone());
        }
        result
    }
    pub fn emitters(
        &mut self,
        sampler: &mut TerrainSampler<'_>,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Vec<[i32; 3]> {
        let mut result = std::collections::BTreeSet::new();
        let lo = min.map(|v| (v / 8.).floor() as i32);
        let hi = max.map(|v| (v / 8.).floor() as i32);
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    for p in self
                        .block(sampler, [x, y, z], true)
                        .iter()
                        .filter(|p| p.material == LIGHT)
                    {
                        let (a, b) = p.bounds();
                        let a = (a * 16.).floor().as_ivec3().to_array();
                        let b = (b * 16.).ceil().as_ivec3().to_array();
                        for yy in a[1]..b[1] {
                            for zz in a[2]..b[2] {
                                for xx in a[0]..b[0] {
                                    let c = [xx, yy, zz];
                                    let v = (V::new(xx as f64, yy as f64, zz as f64)
                                        + V::splat(0.5))
                                        / 16.;
                                    if (0..3).all(|i| v[i] >= min[i] && v[i] <= max[i])
                                        && p.contains(v)
                                    {
                                        result.insert(c);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result.into_iter().collect()
    }
    pub fn affects(
        &mut self,
        sampler: &mut TerrainSampler<'_>,
        min: [f64; 3],
        max: [f64; 3],
    ) -> bool {
        let lo = min.map(|v| (v / 8.).floor() as i32);
        let hi = max.map(|v| (v / 8.).floor() as i32);
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    if self
                        .block(sampler, [x, y, z], true)
                        .iter()
                        .any(|p| !matches!(p.replace, Replace::Rock) && intersects(p, min, max))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}
fn intersects(p: &Primitive, min: [f64; 3], max: [f64; 3]) -> bool {
    let (lo, hi) = p.bounds();
    (0..3).all(|i| lo[i] <= max[i] + 0.0625 && hi[i] >= min[i])
}
