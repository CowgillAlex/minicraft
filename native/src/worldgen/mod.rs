mod aquifer;
mod config;
mod density;
mod fast_density;
mod noise;
pub(crate) mod page;
pub mod preview;
mod random;
pub(crate) mod sky;
#[cfg(test)]
mod tests;

use aquifer::Aquifer;
use density::{Density, Sampler};

pub struct Generator {
    density: Density,
    seed: i64,
    plan: Option<fast_density::Plan>,
}

pub struct TerrainSampler<'a> {
    density: Sampler<'a>,
    aquifer: Aquifer,
    plan: Option<fast_density::Plan>,
}

impl Generator {
    pub fn new(seed: i64) -> Result<Self, String> {
        let density = Density::new(seed)?;
        let plan = fast_density::Plan::compile(&density);
        Ok(Self {
            density,
            seed,
            plan,
        })
    }

    pub fn sampler(&self) -> TerrainSampler<'_> {
        TerrainSampler {
            density: Sampler::new(&self.density),
            aquifer: Aquifer::new(self.seed),
            plan: self.plan,
        }
    }
}

impl TerrainSampler<'_> {
    pub fn density(&mut self, point: [f64; 3]) -> f64 {
        self.density.root(0, point)
    }

    pub fn material(&mut self, point: [f64; 3]) -> u8 {
        let value = self.density(point);
        self.aquifer.substance(&mut self.density, point, value)
    }

    pub fn uniform_material(&mut self, min: [f64; 3], max: [f64; 3]) -> Option<u8> {
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
