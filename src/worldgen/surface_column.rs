use super::Surface;
use crate::{
    chunk::Chunk,
    materials::*,
    worldgen::{biomes, random, surface_rules::Context as RuleContext, TerrainSampler},
};
type Xz = [i32; 2];
const MIN: i32 = -64 * 16;
const MAX: i32 = 320 * 16;
#[derive(Clone, Copy)]
struct Run {
    low: i32,
    high: i32,
    water: Option<f64>,
}
struct Column {
    depth: f64,
    noise: Vec<Option<f64>>,
    runs: Vec<Run>,
    lower: f64,
    steep: Option<bool>,
    ceiling_air: Option<bool>,
}
struct BiomeColumn {
    climate: [f64; 6],
    environment: biomes::Environment,
}
#[derive(Default)]
pub struct Cache {
    columns: crate::cache::Cache<Xz, Column, 65536>,
    heights: crate::cache::Cache<Xz, f64, 65536>,
    biomes: crate::cache::Cache<Xz, BiomeColumn, 16384>,
    climates: crate::cache::Cache<Xz, [f64; 8], 4096>,
}
fn point([x, z]: Xz, y: i32) -> [f64; 3] {
    [x as f64 / 16.0, y as f64 / 16.0, z as f64 / 16.0]
}
fn xz(p: [f64; 3]) -> Xz {
    [(p[0] * 16.0).round() as i32, (p[2] * 16.0).round() as i32]
}
impl Cache {
    fn column(&mut self, surface: &Surface, key: Xz) -> Column {
        self.columns.remove(&key).unwrap_or_else(|| {
            let p = point(key, 0);
            let noise = surface.noises[surface.surface].sample(p);
            let mut values = vec![None; surface.noises.len()];
            values[surface.surface] = Some(noise);
            Column {
                depth: (noise * 2.75
                    + 3.0
                    + random::positional(
                        surface.seed,
                        "minecraft:surface_depth",
                        [p[0].floor() as i32, 0, p[2].floor() as i32],
                    ) * 0.25) as i32 as f64,
                noise: values,
                runs: Vec::new(),
                lower: -64.0,
                steep: None,
                ceiling_air: None,
            }
        })
    }
    pub fn decorate(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        origin: [i32; 3],
        data: &mut Chunk,
    ) {
        if matches!(data, Chunk::Uniform(m) if *m != STONE) {
            return;
        }
        data.with_cells_mut(|cells| self.decorate_cells(surface, sampler, origin, cells));
    }
    fn decorate_cells(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        origin: [i32; 3],
        cells: &mut [u8],
    ) {
        for z in 0..32 {
            for x in 0..32 {
                if !(0..32).any(|y| cells[x + 32 * (z + 32 * y)] == STONE) {
                    continue;
                }
                let key = [origin[0] + x as i32, origin[2] + z as i32];
                let mut column = self.column(surface, key);
                {
                    let mut context = Context {
                        surface,
                        sampler,
                        cache: self,
                        column: &mut column,
                        p: point(key, origin[1]),
                        biome: None,
                    };
                    let mut y = 31_i32;
                    while y >= 0 {
                        if cells[x + 32 * (z + 32 * y as usize)] != STONE {
                            y -= 1;
                            continue;
                        }
                        let top = y;
                        let band = (origin[1] + top + 1024).div_euclid(64);
                        while y > 0
                            && cells[x + 32 * (z + 32 * (y - 1) as usize)] == STONE
                            && (origin[1] + y - 1 + 1024).div_euclid(64) == band
                        {
                            y -= 1;
                        }
                        context.p[1] = (origin[1] + top) as f64 / 16.;
                        paint_span(
                            &surface.rules,
                            &mut context,
                            cells,
                            [x, z],
                            origin[1],
                            y,
                            top,
                        );
                        y -= 1;
                    }
                }
                self.columns.insert(key, column);
            }
        }
    }
    pub fn material(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
    ) -> u8 {
        let key = xz(p);
        let mut column = self.column(surface, key);
        let mut context = Context {
            surface,
            sampler,
            cache: self,
            column: &mut column,
            p,
            biome: None,
        };
        let id = RuleContext::biome(&mut context);
        let result = surface.by_biome[id as usize]
            .apply(&mut context)
            .unwrap_or(STONE);
        self.columns.insert(key, column);
        result
    }
    pub fn affects_light(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
    ) -> bool {
        if !surface
            .transparent
            .contains(&self.biome(surface, sampler, p))
        {
            return false;
        }
        let key = xz(p);
        let mut column = self.column(surface, key);
        let result = surface.rules.changes_opacity(&mut Context {
            surface,
            sampler,
            cache: self,
            column: &mut column,
            p,
            biome: None,
        });
        self.columns.insert(key, column);
        result
    }
    pub fn affects_box(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        min: [f64; 3],
        max: [f64; 3],
    ) -> bool {
        let lo = min.map(|v| (v / 4.0).floor() as i32);
        let hi = max.map(|v| (v / 4.0).floor() as i32);
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    if surface.transparent.contains(&self.biome(
                        surface,
                        sampler,
                        [
                            f64::from(x * 4 + 2),
                            f64::from(y * 4 + 2),
                            f64::from(z * 4 + 2),
                        ],
                    )) {
                        return true;
                    }
                }
            }
        }
        false
    }
    /// Noise is cached at four-metre *nodes* and interpolated continuously. The
    /// selected biome and environmental eligibility are sampled at voxel scale.
    fn climate(&mut self, sampler: &mut TerrainSampler<'_>, p: [f64; 3]) -> [f64; 8] {
        let lo = [(p[0] / 4.0).floor() as i32, (p[2] / 4.0).floor() as i32];
        let t = [p[0] / 4.0 - lo[0] as f64, p[2] / 4.0 - lo[1] as f64];
        let mut result = [0.0; 8];
        for z in 0..=1 {
            for x in 0..=1 {
                let key = [lo[0] + x, lo[1] + z];
                let values = if let Some(v) = self.climates.get(&key) {
                    *v
                } else {
                    let point = [key[0] as f64 * 4.0, 63.0, key[1] as f64 * 4.0];
                    let climate = sampler.climate(point);
                    let waterways = sampler
                        .density
                        .density
                        .waterways
                        .map(|ids| ids.map(|id| sampler.density.eval(id, point)))
                        .unwrap_or([1.0, -1.0]);
                    let v =
                        std::array::from_fn(|i| if i < 6 { climate[i] } else { waterways[i - 6] });
                    self.climates.insert(key, v);
                    v
                };
                let weight = (if x == 0 { 1.0 - t[0] } else { t[0] })
                    * (if z == 0 { 1.0 - t[1] } else { t[1] });
                for i in 0..8 {
                    result[i] += weight * values[i];
                }
            }
        }
        result
    }
    pub(in crate::worldgen) fn biome(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
    ) -> u8 {
        let key = [(p[0] * 16.0).floor() as i32, (p[2] * 16.0).floor() as i32];
        let point = [
            key[0] as f64 / 16.0,
            (p[1] * 16.0).floor().clamp(-1024.0, 5119.0) / 16.0,
            key[1] as f64 / 16.0,
        ];
        let col = self.biomes.remove(&key).unwrap_or_else(|| {
            let fields = self.climate(sampler, point);
            let climate = std::array::from_fn(|i| fields[i]);
            let ground = self.ground(sampler, key);
            let sea = f64::from(sampler.density.density.sea_level);
            let submerged = ground < sea
                && sampler.material_fast([point[0], ground + 0.125, point[2]]) == WATER;
            let depth = if submerged {
                (sea - ground).max(0.0)
            } else {
                0.0
            };
            let mut near_water = false;
            let mut relief = 0.0_f64;
            if (sea - 4.0..=sea + 14.0).contains(&ground) {
                for [dx, dz] in [[-12, 0], [12, 0], [0, -12], [0, 12]] {
                    let k = [key[0] + dx * 16, key[1] + dz * 16];
                    let h = self.height(sampler, k);
                    relief = relief.max((h - ground).abs());
                    near_water |= h < sea
                        && sampler.material_fast([
                            k[0] as f64 / 16.0,
                            sea - 0.0625,
                            k[1] as f64 / 16.0,
                        ]) == WATER;
                }
            }
            BiomeColumn {
                climate,
                environment: biomes::Environment {
                    ground,
                    submerged,
                    water_depth: depth,
                    coast: (sea..=sea + 3.0).contains(&ground) && near_water,
                    wetland: (sea - 4.0..=sea + 14.0).contains(&ground)
                        && relief <= 8.0
                        && climate[0] > -0.45
                        && climate[1] > 0.1,
                    river: submerged && depth < 10.0 && fields[6] < 0.045 && climate[2] > -0.3,
                    pond: submerged && depth < 12.0 && (fields[7] > 0.32 || climate[2] > -0.05),
                },
            }
        });
        let mut climate = col.climate;
        // Small coherent variations mix neighbouring materials through a band,
        // including snow/grass and stone/deepslate, without voxel-white-noise.
        climate[0] += surface.noises[surface.blend].sample(point) * 0.035;
        climate[1] += surface.noises[surface.blend].sample([
            point[0] + 37.0,
            point[1] + 11.0,
            point[2] - 19.0,
        ]) * 0.035;
        let result = surface.biomes.sample_environment(
            surface.biomes.column(climate),
            point[1],
            col.environment,
        );
        self.biomes.insert(key, col);
        result
    }
    pub(in crate::worldgen) fn ground(&mut self, sampler: &mut TerrainSampler<'_>, key: Xz) -> f64 {
        let h = self.height(sampler, key);
        for y in (((h - 2.0) * 16.0) as i32..=((h + 2.0) * 16.0) as i32).rev() {
            if sampler.material_fast(point(key, y)) == STONE {
                return (y + 1) as f64 / 16.0;
            }
        }
        h
    }
    pub(in crate::worldgen) fn height(&mut self, sampler: &mut TerrainSampler<'_>, key: Xz) -> f64 {
        if let Some(value) = self.heights.get(&key) {
            return *value;
        }
        let result = (|| {
            // Keep the source estimate's one-metre vertical lattice. Fine X/Z
            // coordinates still sample continuously through the 3D density.
            let mut above = sampler.density_fast(point(key, 319 * 16));
            let mut y: i32 = 318;
            while y >= -63 {
                let lo = (y.div_euclid(8) * 8).max(-63);
                // Air/water proofs require strictly negative density. Skip only
                // that proven empty interval, retaining the source Y lattice.
                if matches!(
                    sampler.uniform_material(point(key, lo * 16), point(key, y * 16)),
                    Some(AIR | WATER)
                ) {
                    above = -1.0;
                    y = lo - 1;
                    continue;
                }
                while y >= lo {
                    let here = sampler.density_fast(point(key, y * 16));
                    if here > 0.0 && above <= 0.0 {
                        return f64::from(y);
                    }
                    above = here;
                    y -= 1;
                }
            }
            63.0
        })();
        self.heights.insert(key, result);
        result
    }
}
struct Context<'a, 'g> {
    surface: &'a Surface,
    sampler: &'a mut TerrainSampler<'g>,
    cache: &'a mut Cache,
    column: &'a mut Column,
    p: [f64; 3],
    biome: Option<(i32, u8)>,
}
impl Context<'_, '_> {
    fn run(&mut self) -> Run {
        let y = (self.p[1] * 16.0).round() as i32;
        if let Some(run) = self.column.runs.iter().find(|r| y >= r.low && y <= r.high) {
            return *run;
        }
        let key = xz(self.p);
        let high = boundary(self.sampler, key, y, 1);
        let low = boundary(self.sampler, key, y, -1);
        let water = if high + 1 < MAX && self.sampler.material_fast(point(key, high + 1)) == WATER {
            water_top(self.sampler, key, high + 1)
        } else {
            None
        };
        let run = Run { low, high, water };
        self.column.runs.push(run);
        run
    }
}
impl RuleContext for Context<'_, '_> {
    fn y(&self) -> f64 {
        self.p[1]
    }
    fn biome(&mut self) -> u8 {
        let key = (self.p[1] * 16.0).floor() as i32;
        if let Some((y, id)) = self.biome {
            if y == key {
                return id;
            }
        }
        let id = self.cache.biome(self.surface, self.sampler, self.p);
        self.biome = Some((key, id));
        id
    }
    fn cold(&mut self) -> bool {
        let id = self.biome();
        let name = self.surface.biomes.name(id);
        matches!(name, "frostmere" | "rime_crown" | "frostflow")
    }
    fn noise(&mut self, index: usize) -> f64 {
        *self.column.noise[index]
            .get_or_insert_with(|| self.surface.noises[index].sample([self.p[0], 0.0, self.p[2]]))
    }
    fn cave_growth(&mut self) -> f64 {
        let value = self.surface.cave_growth(self.p);
        let id = self.biome();
        if matches!(self.surface.biomes.name(id), "glimmerdeep" | "hollowstone") {
            value + self.surface.noises[self.surface.blend].sample(self.p.map(|v| v * 2.0)) * 0.15
        } else {
            value
        }
    }
    fn surface_depth(&mut self) -> f64 {
        self.column.depth
    }
    fn secondary(&mut self) -> f64 {
        self.noise(self.surface.secondary)
    }
    fn above(&mut self) -> f64 {
        (self.run().high as f64 + 1.0) / 16.0 - self.p[1]
    }
    fn below(&mut self) -> f64 {
        self.p[1] - (self.run().low as f64 - 1.0) / 16.0
    }
    fn water(&mut self) -> Option<f64> {
        self.run().water
    }
    fn preliminary(&mut self) -> f64 {
        let key = xz(self.p);
        let depth = self.column.depth;
        if let Some(height) = self.cache.heights.get(&key) {
            return *height + depth - 8.0;
        }
        if self.p[1] >= self.column.lower + depth - 8.0 {
            let probe = (self.p[1] + 16.0).ceil();
            let ceiling_air = *self.column.ceiling_air.get_or_insert_with(|| {
                self.sampler.density_fast([self.p[0], 319.0, self.p[2]]) <= 0.0
            });
            if ceiling_air
                && probe <= 318.0
                && self.sampler.density_fast([self.p[0], probe, self.p[2]]) > 0.0
            {
                self.column.lower = self.column.lower.max(probe);
            } else {
                return self.cache.height(self.sampler, key) + depth - 8.0;
            }
        }
        self.column.lower + depth - 8.0
    }
    fn steep(&mut self) -> bool {
        if let Some(v) = self.column.steep {
            return v;
        }
        let [x, z] = xz(self.p);
        let dx = self.cache.height(self.sampler, [x - 16, z])
            - self.cache.height(self.sampler, [x + 16, z]);
        let dz = self.cache.height(self.sampler, [x, z - 16])
            - self.cache.height(self.sampler, [x, z + 16]);
        let steep = dx.abs() >= 4.0 || dz.abs() >= 4.0;
        self.column.steep = Some(steep);
        steep
    }
    fn random(&self, name: &str) -> f64 {
        random::positional(
            self.surface.seed,
            name,
            [
                self.p[0].floor() as i32,
                (self.p[1] + 64.0).floor() as i32,
                self.p[2].floor() as i32,
            ],
        )
    }
    fn band(&mut self) -> u8 {
        let offset = self.surface.noises[self.surface.clay].sample([
            self.p[0] * 0.05,
            0.0,
            self.p[2] * 0.05,
        ]) * 4.0;
        self.surface.bands[((self.p[1] + 64.0 + offset).floor() as i32).rem_euclid(192) as usize]
    }
}
/// Find the actual segment boundary, skipping only proven uniform stone spans.
/// A partial run could invent a floor at its artificial endpoint, so runs are exact.
fn boundary(sampler: &mut TerrainSampler<'_>, key: Xz, start: i32, sign: i32) -> i32 {
    let end = if sign > 0 { MAX - 1 } else { MIN };
    let mut y = start + sign;
    while if sign > 0 { y <= end } else { y >= end } {
        let edge = if sign > 0 {
            (y.div_euclid(128) * 128 + 127).min(end)
        } else {
            (y.div_euclid(128) * 128).max(end)
        };
        if sampler.uniform_material(point(key, y.min(edge)), point(key, y.max(edge))) == Some(STONE)
        {
            y = edge + sign;
            continue;
        }
        while if sign > 0 { y <= edge } else { y >= edge } {
            if sampler.material_fast(point(key, y)) != STONE {
                return y - sign;
            }
            y += sign;
        }
    }
    end
}
fn water_top(sampler: &mut TerrainSampler<'_>, key: Xz, mut y: i32) -> Option<f64> {
    let mut height = None;
    while y < MAX {
        let edge = (y.div_euclid(128) * 128 + 127).min(MAX - 1);
        match sampler.uniform_material(point(key, y), point(key, edge)) {
            Some(AIR) => break,
            Some(WATER) => {
                height = Some((edge + 1) as f64 / 16.0);
                y = edge + 1;
                continue;
            }
            Some(STONE) => {
                y = edge + 1;
                continue;
            }
            _ => {}
        }
        while y <= edge {
            match sampler.material_fast(point(key, y)) {
                AIR => return height,
                WATER => height = Some((y + 1) as f64 / 16.0),
                _ => {}
            }
            y += 1;
        }
    }
    height
}

/// Descending contiguous stone spans share column context. Split only where a
/// condition can change; uncertain gradients and clay bands retain literal evaluation.
fn paint_span(
    rule: &crate::worldgen::surface_rules::Rule,
    context: &mut Context<'_, '_>,
    cells: &mut [u8],
    [x, z]: [usize; 2],
    origin: i32,
    low: i32,
    high: i32,
) {
    context.p[1] = (origin + high) as f64 / 16.;
    let uniform = rule.uniform_span(context, (origin + low) as f64 / 16.);
    if let Some(material) = uniform {
        if let Some(m) = material.filter(|&m| m != STONE) {
            for y in low..=high {
                cells[x + 32 * (z + 32 * y as usize)] = m;
            }
        }
    } else if low == high {
        if let Some(m) = rule.apply(context).filter(|&m| m != STONE) {
            cells[x + 32 * (z + 32 * low as usize)] = m;
        }
    } else {
        let mid = (low + high) / 2;
        paint_span(rule, context, cells, [x, z], origin, mid + 1, high);
        paint_span(rule, context, cells, [x, z], origin, low, mid);
    }
}

#[cfg(test)]
impl Cache {
    fn decorate_literal(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        origin: [i32; 3],
        data: &mut Chunk,
    ) {
        for z in 0..32 {
            for x in 0..32 {
                if !(0..32).any(|y| data.get(x + 32 * (z + 32 * y)) == STONE) {
                    continue;
                }
                let key = [origin[0] + x as i32, origin[2] + z as i32];
                let mut column = self.column(surface, key);
                {
                    let mut context = Context {
                        surface,
                        sampler,
                        cache: self,
                        column: &mut column,
                        p: point(key, origin[1]),
                        biome: None,
                    };
                    for y in (0..32).rev() {
                        let index = x + 32 * (z + 32 * y);
                        if data.get(index) != STONE {
                            continue;
                        }
                        context.p[1] = (origin[1] + y as i32) as f64 / 16.0;
                        if let Some(material) = surface.rules.apply(&mut context) {
                            if material != STONE {
                                data.set(index, material);
                            }
                        }
                    }
                }
                self.columns.insert(key, column);
            }
        }
        data.compact();
    }
}

#[cfg(test)]
#[path = "surface_page_tests.rs"]
mod page_tests;
