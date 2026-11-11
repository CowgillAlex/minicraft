use super::*;
use crate::worldgen::{random, surface_rules::Context};
struct Fixture<'a> {
    surface: &'a Surface,
    biome: u8,
    profile: &'a str,
    top: i32,
    x: i32,
    y: i32,
}
impl Fixture<'_> {
    fn raw(&self, y: i32) -> u8 {
        if !(-64..320).contains(&y) {
            return AIR;
        }
        if self.profile == "thin" && y < 88 {
            return AIR;
        }
        if self.profile == "ocean" && (54..63).contains(&y) {
            return WATER;
        }
        if self.profile == "cave" && (58..=60).contains(&y) {
            return AIR;
        }
        if self.profile == "deep" && (-36..=-28).contains(&y) {
            return AIR;
        }
        if y < self.top {
            STONE
        } else {
            AIR
        }
    }
}
impl Context for Fixture<'_> {
    fn y(&self) -> f64 {
        f64::from(self.y)
    }
    fn biome(&mut self) -> u8 {
        self.biome
    }
    fn cold(&mut self) -> bool {
        let n = self.surface.biomes.name(self.biome);
        n.contains("snowy") || n.contains("frozen") || n.contains("ice")
    }
    fn noise(&mut self, i: usize) -> f64 {
        self.surface.noises[i].sample([f64::from(self.x), 0.0, 0.0])
    }
    fn surface_depth(&mut self) -> f64 {
        (self.noise(self.surface.surface) * 2.75
            + 3.0
            + random::positional(self.surface.seed, "minecraft:surface_depth", [self.x, 0, 0])
                * 0.25) as i32 as f64
    }
    fn secondary(&mut self) -> f64 {
        self.noise(self.surface.secondary)
    }
    fn above(&mut self) -> f64 {
        let mut y = self.y + 1;
        while y < 320 && self.raw(y) == STONE {
            y += 1;
        }
        (y - self.y) as f64
    }
    fn below(&mut self) -> f64 {
        let mut y = self.y - 1;
        while y >= -64 && self.raw(y) == STONE {
            y -= 1;
        }
        (self.y - y) as f64
    }
    fn water(&mut self) -> Option<f64> {
        let mut height = None;
        for y in ((self.y + 1)..320).rev() {
            match self.raw(y) {
                AIR => height = None,
                WATER if height.is_none() => height = Some(f64::from(y + 1)),
                _ => {}
            }
        }
        height
    }
    fn steep(&mut self) -> bool {
        self.profile == "steep" && self.x == 0
    }
    fn preliminary(&mut self) -> f64 {
        f64::from(
            self.top - 1
                + if self.profile == "steep" && self.x > 0 {
                    5
                } else {
                    0
                },
        ) + self.surface_depth()
            - 8.0
    }
    fn random(&self, name: &str) -> f64 {
        random::positional(self.surface.seed, name, [self.x, self.y + 64, 0])
    }
    fn band(&mut self) -> u8 {
        let offset =
            self.surface.noises[self.surface.clay].sample([f64::from(self.x) * 0.05, 0.0, 0.0])
                * 4.0;
        self.surface.bands
            [((f64::from(self.y + 64) + offset).floor() as i32).rem_euclid(192) as usize]
    }
}
#[test]
fn surface_rules_and_bands_match_pinned_java_on_synthetic_columns() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-surface.json")).unwrap();
    let mut systems = std::collections::HashMap::new();
    for row in data["columns"].as_array().unwrap() {
        let seed = row[0].as_i64().unwrap();
        let surface = systems
            .entry(seed)
            .or_insert_with(|| Surface::new(seed).unwrap());
        let mut c = Fixture {
            surface,
            biome: surface.biomes.id(row[1].as_str().unwrap()).unwrap(),
            profile: row[2].as_str().unwrap(),
            x: row[3].as_i64().unwrap() as i32,
            top: row[4].as_i64().unwrap() as i32,
            y: 0,
        };
        for sample in row[5].as_array().unwrap() {
            c.y = sample[0].as_i64().unwrap() as i32;
            let raw = c.raw(c.y);
            let opacity_candidate = surface.rules.changes_opacity(&mut c);
            let actual = if raw == STONE {
                surface.rules.apply(&mut c).unwrap_or(STONE)
            } else {
                raw
            };
            if raw == STONE {
                let rule = &surface.by_biome[c.biome as usize];
                assert_eq!(rule.apply(&mut c).unwrap_or(STONE), actual);
                let high = c.y;
                let low = (high - 2).max(-64);
                if (low..=high).all(|y| c.raw(y) == STONE) {
                    if let Some(proven) = rule.uniform_span(&mut c, low as f64) {
                        for y in low..=high {
                            c.y = y;
                            assert_eq!(surface.rules.apply(&mut c), proven, "span {low}..{high}");
                        }
                        c.y = high;
                    }
                }
            }
            if raw == STONE && matches!(actual, AIR | WATER) {
                assert!(opacity_candidate);
                assert!(surface.transparent.contains(&c.biome));
            }
            assert_eq!(
                crate::materials::get(actual).source,
                sample[1].as_str().unwrap(),
                "seed {seed}, biome {}, {}, x={}, y={}",
                row[1],
                c.profile,
                c.x,
                c.y
            );
        }
    }
    for row in data["bands"].as_array().unwrap() {
        let seed = row[0].as_i64().unwrap();
        let surface = &systems[&seed];
        let x = row[1].as_i64().unwrap() as i32;
        let offset = surface.noises[surface.clay].sample([f64::from(x) * 0.05, 0.0, 0.0]) * 4.0;
        for (y, expected) in row[2].as_array().unwrap().iter().enumerate() {
            let actual =
                surface.bands[((y as f64 + offset).floor() as i32).rem_euclid(192) as usize];
            assert_eq!(
                crate::materials::get(actual).source,
                expected.as_str().unwrap(),
                "bands {seed}/{x}/{y}"
            );
        }
    }
}

#[test]
fn surface_biomes_match_java_column_selection() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-surface.json")).unwrap();
    let table = Table::shared().unwrap();
    for row in data["biomes"].as_array().unwrap() {
        let climate = std::array::from_fn(|i| row[i].as_f64().unwrap());
        let actual = table.sample(table.column(climate), climate[4]);
        assert_eq!(table.name(actual), row[6].as_str().unwrap(), "{climate:?}");
    }
}
#[test]
#[ignore = "manual surface generation timing and material survey"]
fn surface_page_benchmark() {
    use crate::worldgen::{page, preview::Preview, Generator};
    use std::{collections::BTreeMap, time::Instant};
    for seed in [1, 12345, -928341] {
        let generator = Generator::new(seed).unwrap();
        let mut sampler = generator.sampler();
        let mut cache = Cache::default();
        for [x, z] in [[120, -20], [0, 0], [512, 512], [-512, 256], [2048, -2048]] {
            let y = cache.height(&mut sampler, [x * 16, z * 16]) as i32;
            let biome = cache.biome(
                &generator.surface,
                &mut sampler,
                [x as f64, y as f64, z as f64],
            );
            let preview = Preview {
                seed,
                origin: [x, (y - 2).div_euclid(2) * 2, z],
                height_m: 8,
                surfaces: true,
                ..Preview::default()
            };
            let start = Instant::now();
            let mut counts = BTreeMap::new();
            for py in 0..4 {
                let page = page::generate(preview, &mut sampler, [0, py * 32, 0]);
                for m in page.data.to_vec() {
                    *counts
                        .entry(crate::materials::get(m).name)
                        .or_insert(0usize) += 1;
                }
            }
            println!(
                "SURFACE {seed} {x}/{y}/{z} {} {:.3}s {counts:?}",
                generator.surface.biomes.name(biome),
                start.elapsed().as_secs_f64()
            );
        }
    }
}

#[test]
fn surface_pages_are_independent_of_neighbor_and_generation_order() {
    use crate::worldgen::{page, preview::Preview, Generator};
    for (seed, origin) in [
        (12345, [-2, 62, -2]),
        (1, [120, 58, -20]),
        (1, [-2, -64, -2]),
    ] {
        let generator = Generator::new(seed).unwrap();
        let preview = Preview {
            seed,
            origin,
            surfaces: true,
            ..Preview::default()
        };
        let pages = [[0, 0, 0], [32, 0, 0], [0, 32, 0], [32, 32, 0]];
        let mut forward = generator.sampler();
        let expected: Vec<_> = pages
            .iter()
            .map(|p| page::generate(preview, &mut forward, *p).data.to_vec())
            .collect();
        let mut reverse = generator.sampler();
        let mut independent = generator.sampler();
        let mut cache = Cache::default();
        for i in (0..pages.len()).rev() {
            let actual = page::generate(preview, &mut reverse, pages[i])
                .data
                .to_vec();
            assert_eq!(actual, expected[i], "surface order {seed}/{origin:?}/{i}");
            for z in [0, 15, 31] {
                for x in [0, 15, 31] {
                    for y in [0, 15, 31] {
                        let p = preview.point([pages[i][0] + x, pages[i][1] + y, pages[i][2] + z]);
                        let raw = independent.material_fast(p);
                        let material = if raw == STONE {
                            cache.material(&generator.surface, &mut independent, p)
                        } else {
                            raw
                        };
                        let index = (x + 32 * (z + 32 * y)) as usize;
                        assert_eq!(actual[index], material, "surface point {seed}/{p:?}");
                        let light = independent.light_material(p, true);
                        // Opaque surface materials share the same transport response.
                        assert_eq!(
                            crate::light_field::loss(light),
                            crate::light_field::loss(material),
                            "surface light {seed}/{p:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn surface_height_skips_preserve_the_source_one_metre_scan() {
    use crate::worldgen::Generator;
    for (seed, x, z) in [
        (1, 120, -20),
        (1, 0, 0),
        (12345, -512, 256),
        (-928341, 2048, -2048),
    ] {
        let generator = Generator::new(seed).unwrap();
        let mut sampler = generator.sampler();
        let mut cache = Cache::default();
        for key in [
            [x * 16, z * 16],
            [x * 16 + 1, z * 16 - 1],
            [x * 16 - 16, z * 16 + 16],
        ] {
            let mut above =
                sampler.density_fast([key[0] as f64 / 16.0, 319.0, key[1] as f64 / 16.0]);
            let mut expected = 63.0;
            for y in (-63..=318).rev() {
                let here =
                    sampler.density_fast([key[0] as f64 / 16.0, y as f64, key[1] as f64 / 16.0]);
                if here > 0.0 && above <= 0.0 {
                    expected = y as f64;
                    break;
                }
                above = here;
            }
            assert_eq!(
                cache.height(&mut sampler, key),
                expected,
                "height {seed}/{key:?}"
            );
        }
    }
}

#[test]
fn debug_biome_follows_the_generated_grid_across_horizontal_and_vertical_travel() {
    use crate::worldgen::{BiomeProbe, Generator};
    for seed in [1, -67] {
        let generator = Generator::new(seed).unwrap();
        let mut sampler = generator.sampler();
        let mut cache = Cache::default();
        let mut probe = BiomeProbe::new(seed).unwrap();
        for p in [
            [-0.0625, 63.0, -4.0625],
            [-0.0625, -40.0, -4.0625],
            [-0.0625, -39.99, -4.0625],
            [0.0, -40.0, -4.0],
            [120.0, 63.0, -20.0],
            [120.0, -64.0, -20.0],
            [-0.0625, 63.0, -4.0625],
        ] {
            probe.update(p);
            let id = cache.biome(&generator.surface, &mut sampler, p);
            assert_eq!(
                probe.name(),
                generator.surface.biomes.name(id),
                "debug biome {seed}/{p:?}"
            );
        }
    }
}
