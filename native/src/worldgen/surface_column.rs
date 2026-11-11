use super::Surface;
use crate::{
    chunk::Chunk,
    materials::*,
    worldgen::{biomes, random, surface_rules::Context as RuleContext, TerrainSampler},
};
use std::collections::HashMap;
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
    candidate: biomes::Column,
    values: [Option<u8>; 96],
}
#[derive(Default)]
pub struct Cache {
    columns: HashMap<Xz, Column>,
    heights: HashMap<Xz, f64>,
    biomes: HashMap<Xz, BiomeColumn>,
}
fn point([x, z]: Xz, y: i32) -> [f64; 3] {
    [x as f64 / 16.0, y as f64 / 16.0, z as f64 / 16.0]
}
fn xz(p: [f64; 3]) -> Xz {
    [(p[0] * 16.0).round() as i32, (p[2] * 16.0).round() as i32]
}
impl Cache {
    fn column(&mut self, surface: &Surface, key: Xz) -> Column {
        if self.columns.len() >= 65536 {
            self.columns.clear();
        }
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
    pub fn material(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
    ) -> u8 {
        let key = xz(p);
        let mut column = self.column(surface, key);
        let result = surface
            .rules
            .apply(&mut Context {
                surface,
                sampler,
                cache: self,
                column: &mut column,
                p,
                biome: None,
            })
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
    pub(super) fn biome(
        &mut self,
        surface: &Surface,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
    ) -> u8 {
        if self.biomes.len() >= 4096 {
            self.biomes.clear();
        }
        let key = [(p[0] / 4.0).floor() as i32, (p[2] / 4.0).floor() as i32];
        let qy = ((p[1] + 64.0) / 4.0).floor().clamp(0.0, 95.0) as usize;
        let col = self.biomes.entry(key).or_insert_with(|| {
            let p = [f64::from(key[0] * 4 + 2), 63.0, f64::from(key[1] * 4 + 2)];
            BiomeColumn {
                candidate: surface.biomes.column(sampler.climate(p)),
                values: [None; 96],
            }
        });
        *col.values[qy].get_or_insert_with(|| {
            let p = [
                f64::from(key[0] * 4 + 2),
                -64.0 + qy as f64 * 4.0 + 2.0,
                f64::from(key[1] * 4 + 2),
            ];
            surface.biomes.sample(col.candidate, sampler.depth(p))
        })
    }
    pub(super) fn height(&mut self, sampler: &mut TerrainSampler<'_>, key: Xz) -> f64 {
        if self.heights.len() >= 65536 {
            self.heights.clear();
        }
        *self.heights.entry(key).or_insert_with(|| {
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
        })
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
        let key = ((self.p[1] + 64.0) / 4.0).floor() as i32;
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
        name.contains("snowy") || name.contains("frozen") || name.contains("ice")
    }
    fn noise(&mut self, index: usize) -> f64 {
        *self.column.noise[index]
            .get_or_insert_with(|| self.surface.noises[index].sample([self.p[0], 0.0, self.p[2]]))
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
