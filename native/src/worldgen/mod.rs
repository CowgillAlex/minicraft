mod aquifer;
mod config;
mod density;
mod noise;
pub mod preview;
mod random;
#[cfg(test)]
mod tests;

use aquifer::Aquifer;
use density::{Density, Sampler};

pub struct Generator {
    density: Density,
    seed: i64,
}

pub struct TerrainSampler<'a> {
    density: Sampler<'a>,
    aquifer: Aquifer,
}

impl Generator {
    pub fn new(seed: i64) -> Result<Self, String> {
        Ok(Self {
            density: Density::new(seed)?,
            seed,
        })
    }

    pub fn sampler(&self) -> TerrainSampler<'_> {
        TerrainSampler {
            density: Sampler::new(&self.density),
            aquifer: Aquifer::new(self.seed),
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

    pub fn clear_density_cache(&mut self) {
        self.density.clear();
    }
}
