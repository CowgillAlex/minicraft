use crate::worldgen::TerrainSampler;
impl TerrainSampler<'_> {
    pub(crate) fn feature_habitat(&mut self, p: [f64; 3]) -> (super::ecology::Habitat, String) {
        let mut cache = std::mem::take(&mut self.surface_cache);
        let id = cache.biome(self.surface, self, p);
        self.surface_cache = cache;
        let name = self.surface.biomes.name(id);
        (super::ecology::habitat(name), name.to_owned())
    }
    pub(crate) fn feature_ground(&mut self, x: f64, z: f64) -> glam::DVec3 {
        let key = [(x * 16.).floor() as i32, (z * 16.).floor() as i32];
        let mut cache = std::mem::take(&mut self.surface_cache);
        let h = cache.height(self, key);
        self.surface_cache = cache;
        let x = key[0] as f64 / 16.;
        let z = key[1] as f64 / 16.;
        for yy in (((h - 2.) * 16.) as i32..=((h + 2.) * 16.) as i32).rev() {
            if self.material_fast([x, yy as f64 / 16., z]) == crate::world::STONE {
                return glam::DVec3::new(x, (yy + 1) as f64 / 16., z);
            }
        }
        glam::DVec3::new(x, h, z)
    }
    pub fn decorate_features(&mut self, origin: [i32; 3], data: &mut crate::chunk::Chunk) {
        let mut cache = std::mem::take(&mut self.features);
        cache
            .page(self, origin.map(|v| v.div_euclid(32)))
            .apply(data);
        self.features = cache;
    }
    pub fn world_optical_material(&mut self, p: [f64; 3], surfaces: bool, features: bool) -> u8 {
        if !features {
            return self.light_material(p, surfaces);
        }
        let mut cache = std::mem::take(&mut self.features);
        let result = cache.optical(self, p, surfaces);
        self.features = cache;
        result
    }
    #[cfg(test)]
    pub fn world_material(&mut self, p: [f64; 3], surfaces: bool, features: bool) -> u8 {
        let base = self.light_material(p, surfaces);
        if !features {
            return base;
        }
        let cell = p.map(|v| (v * 16.).floor() as i32);
        let key = cell.map(|v| v.div_euclid(32));
        let index = (cell[0].rem_euclid(32)
            + 32 * (cell[2].rem_euclid(32) + 32 * cell[1].rem_euclid(32)))
            as usize;
        let mut cache = std::mem::take(&mut self.features);
        let result = cache.page(self, key).at(index, base);
        self.features = cache;
        result
    }
    pub fn feature_emitters(&mut self, min: [f64; 3], max: [f64; 3]) -> Vec<[i32; 3]> {
        let mut cache = std::mem::take(&mut self.features);
        let result = cache.emitters(self, min, max);
        self.features = cache;
        result
    }
    /// A representative optical material, not an exact visual material ID.
    pub fn world_uniform(
        &mut self,
        min: [f64; 3],
        max: [f64; 3],
        surfaces: bool,
        features: bool,
    ) -> Option<u8> {
        if features {
            let sizes = [4., 8., 4.];
            let key = std::array::from_fn::<_, 3, _>(|i| (min[i] / sizes[i]).floor() as i32);
            let lo = std::array::from_fn(|i| key[i] as f64 * sizes[i]);
            let hi = std::array::from_fn(|i| lo[i] + sizes[i] - 1. / 16.);
            if (0..3).all(|i| max[i] <= hi[i]) {
                let cache_key = (key, surfaces, features);
                let cached = if let Some(value) = self.optical_uniform_cells.get(&cache_key) {
                    *value
                } else {
                    let value = self.world_uniform_uncached(lo, hi, surfaces, features);
                    self.optical_uniform_cells.insert(cache_key, value);
                    value
                };
                if cached.is_some() {
                    return cached;
                }
            }
        }
        self.world_uniform_uncached(min, max, surfaces, features)
    }

    fn world_uniform_uncached(
        &mut self,
        min: [f64; 3],
        max: [f64; 3],
        surfaces: bool,
        features: bool,
    ) -> Option<u8> {
        let base = self.light_uniform(min, max, surfaces)?;
        if !features {
            return Some(base);
        }
        let mut cache = std::mem::take(&mut self.features);
        let affects = cache.affects(self, min, max, base);
        self.features = cache;
        (!affects).then_some(base)
    }
}
