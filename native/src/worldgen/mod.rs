mod aquifer;
mod biome_probe;
mod biomes;
pub use biome_probe::BiomeProbe;
mod config;
mod density;
mod fast_density;
mod noise;
pub(crate) mod page;
mod preliminary;
pub mod preview;
mod random;
pub(crate) mod sky;
mod surface;
mod surface_rules;
#[cfg(test)]
mod tests;

use aquifer::Aquifer;
use density::{Density, Sampler};

pub struct Generator {
    density: Density,
    seed: i64,
    plan: Option<fast_density::Plan>,
    surface: surface::Surface,
}

pub struct TerrainSampler<'a> {
    density: Sampler<'a>,
    aquifer: Aquifer,
    plan: Option<fast_density::Plan>,
    prepared_cells: [Option<([i32; 3], fast_density::Prepared)>; 16],
    uniform_cells: std::collections::HashMap<[i32; 3], Option<u8>>,
    surface: &'a surface::Surface,
    surface_cache: surface::Cache,
}

impl Generator {
    pub fn new(seed: i64) -> Result<Self, String> {
        let density = Density::new(seed)?;
        let plan = fast_density::Plan::compile(&density);
        Ok(Self {
            density,
            seed,
            plan,
            surface: surface::Surface::new(seed)?,
        })
    }

    pub fn sampler(&self) -> TerrainSampler<'_> {
        TerrainSampler {
            density: Sampler::new(&self.density),
            aquifer: Aquifer::new(self.seed),
            plan: self.plan,
            prepared_cells: [None; 16],
            uniform_cells: Default::default(),
            surface: &self.surface,
            surface_cache: Default::default(),
        }
    }
}

impl TerrainSampler<'_> {
    pub fn density(&mut self, point: [f64; 3]) -> f64 {
        self.density.root(0, point)
    }

    pub fn climate(&mut self, point: [f64; 3]) -> [f64; 6] {
        std::array::from_fn(|i| self.density.eval(self.density.density.climate[i], point))
    }
    pub fn depth(&mut self, point: [f64; 3]) -> f64 {
        self.density.eval(self.density.density.climate[4], point)
    }
    pub fn decorate(&mut self, origin: [i32; 3], data: &mut crate::chunk::Chunk) {
        let mut cache = std::mem::take(&mut self.surface_cache);
        cache.decorate(self.surface, self, origin, data);
        self.surface_cache = cache;
    }
    pub fn light_material(&mut self, point: [f64; 3], surfaces: bool) -> u8 {
        let raw = self.material_fast(point);
        if raw != crate::world::STONE || !surfaces {
            return raw;
        }
        let mut cache = std::mem::take(&mut self.surface_cache);
        let m = if cache.affects_light(self.surface, self, point) {
            cache.material(self.surface, self, point)
        } else {
            raw
        };
        self.surface_cache = cache;
        m
    }
    pub fn light_uniform(&mut self, min: [f64; 3], max: [f64; 3], surfaces: bool) -> Option<u8> {
        let raw = self.uniform_material(min, max)?;
        if raw != crate::world::STONE || !surfaces {
            return Some(raw);
        }
        let mut cache = std::mem::take(&mut self.surface_cache);
        let affects = cache.affects_box(self.surface, self, min, max);
        self.surface_cache = cache;
        (!affects).then_some(raw)
    }
    pub fn material(&mut self, point: [f64; 3]) -> u8 {
        let value = self.density(point);
        self.aquifer.substance(&mut self.density, point, value)
    }

    /// The same density expression as page fills, cached by interpolation cell.
    pub fn material_fast(&mut self, point: [f64; 3]) -> u8 {
        let value = self.density_fast(point);
        self.aquifer.substance(&mut self.density, point, value)
    }
    pub fn density_fast(&mut self, point: [f64; 3]) -> f64 {
        let key =
            std::array::from_fn::<_, 3, _>(|i| (point[i] / [4.0, 8.0, 4.0][i]).floor() as i32);
        let slot = ((key[0].wrapping_mul(3) ^ key[1].wrapping_mul(5) ^ key[2].wrapping_mul(7))
            as usize)
            & 15;
        if self.prepared_cells[slot].is_none_or(|(k, _)| k != key) {
            self.prepared_cells[slot] = self
                .plan
                .and_then(|p| p.prepare(&mut self.density, point, point))
                .map(|p| (key, p));
        }
        let value =
            self.prepared_cells[slot].map_or_else(|| self.density(point), |(_, p)| p.sample(point));
        if value.abs() < 1e-10 {
            self.density(point)
        } else {
            value
        }
    }

    pub fn uniform_material(&mut self, min: [f64; 3], max: [f64; 3]) -> Option<u8> {
        let sizes = [4.0, 8.0, 4.0];
        let key = std::array::from_fn::<_, 3, _>(|i| (min[i] / sizes[i]).floor() as i32);
        let lo = std::array::from_fn(|i| f64::from(key[i]) * sizes[i]);
        let hi = std::array::from_fn(|i| lo[i] + sizes[i] - 1.0 / 16.0);
        if (0..3).all(|i| max[i] <= hi[i]) {
            let cached = if let Some(value) = self.uniform_cells.get(&key) {
                *value
            } else {
                let value = self.uniform_raw(lo, hi);
                if self.uniform_cells.len() >= 16384 {
                    self.uniform_cells.clear();
                }
                self.uniform_cells.insert(key, value);
                value
            };
            if cached.is_some() {
                return cached;
            }
        }
        self.uniform_raw(min, max)
    }
    fn uniform_raw(&mut self, min: [f64; 3], max: [f64; 3]) -> Option<u8> {
        let bounds = self.density.bounds(self.density.density.roots[0], min, max);
        // Only classify well away from zero; borderline samples retain the reference path.
        if bounds.low > 1e-9 {
            return Some(crate::world::STONE);
        }
        if bounds.high < -1e-9 {
            let fluid = self.aquifer.uniform_status(&mut self.density, min, max)?;
            return (fluid.at(min[1]) == fluid.at(max[1])).then(|| fluid.at(min[1]));
        }
        None
    }
    fn prepared(&mut self, min: [f64; 3], max: [f64; 3]) -> Option<fast_density::Prepared> {
        self.plan?.prepare(&mut self.density, min, max)
    }
    fn open_fluid(&mut self, min: [f64; 3], max: [f64; 3]) -> Option<aquifer::Fluid> {
        self.aquifer.uniform_status(&mut self.density, min, max)
    }
    #[inline]
    fn prepared_material(
        &mut self,
        point: [f64; 3],
        density: f64,
        fluid: Option<aquifer::Fluid>,
    ) -> u8 {
        if density > 0.0 {
            crate::world::STONE
        } else if let Some(fluid) = fluid {
            fluid.at(point[1])
        } else {
            self.aquifer.substance(&mut self.density, point, density)
        }
    }
}
