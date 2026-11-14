use super::{biomes, surface::Cache, Generator};

/// Debug queries share the generator's fine-voxel biome lookup.
pub struct BiomeProbe {
    generator: Generator,
    cache: Cache,
    cell: Option<[i32; 3]>,
    biome: biomes::Id,
}
impl BiomeProbe {
    pub fn new(seed: i64) -> Result<Self, String> {
        Ok(Self {
            generator: Generator::new(seed)?,
            cache: Cache::default(),
            cell: None,
            biome: 0,
        })
    }
    pub fn update(&mut self, point: [f64; 3]) {
        let cell = point.map(|v| (v * 16.0).floor() as i32);
        if self.cell == Some(cell) {
            return;
        }
        let mut sampler = self.generator.sampler();
        self.biome = self
            .cache
            .biome(&self.generator.surface, &mut sampler, point);
        self.cell = Some(cell);
    }
    pub fn name(&self) -> &str {
        self.generator.surface.biomes.name(self.biome)
    }
}
