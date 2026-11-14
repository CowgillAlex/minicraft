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
        matches!(n, "frostmere" | "rime_crown" | "frostflow")
    }
    fn noise(&mut self, i: usize) -> f64 {
        self.surface.noises[i].sample([f64::from(self.x), 0.0, 0.0])
    }
    fn cave_growth(&mut self) -> f64 {
        let point = [self.x as f64, self.y as f64, 0.0];
        let value = self.surface.cave_growth(point);
        if matches!(
            self.surface.biomes.name(self.biome),
            "glimmerdeep" | "hollowstone"
        ) {
            value + self.surface.noises[self.surface.blend].sample(point.map(|v| v * 2.0)) * 0.15
        } else {
            value
        }
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
        if self.profile == "fine" {
            return 0.0625;
        }
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
fn cave_surfaces_keep_stone_between_frequent_seeded_patches() {
    for seed in [742, -91, 1794607392029456600] {
        let surface = Surface::new(seed).unwrap();
        for name in ["rootvault", "hollowstone", "glimmerdeep"] {
            let biome = surface.biomes.id(name).unwrap();
            let mut stone = 0;
            let mut special = 0;
            for x in (-160..160).step_by(2) {
                for y in [-32, 0, 32, 64] {
                    let mut c = Fixture {
                        surface: &surface,
                        biome,
                        profile: "fine",
                        x,
                        top: y + 1,
                        y,
                    };
                    let material = surface.rules.apply(&mut c).unwrap();
                    assert_eq!(
                        surface.by_biome[biome as usize].apply(&mut c),
                        Some(material)
                    );
                    if material == STONE {
                        stone += 1;
                    } else {
                        special += 1;
                    }
                    // A bare cave's subsoil must remain rock, including under a green patch.
                    c.profile = "dry";
                    c.top = y + 4;
                    assert_eq!(surface.rules.apply(&mut c), Some(STONE));
                }
            }
            println!("CAVE PATCH {seed}/{name}: stone={stone}, special={special}");
            assert!(stone > special, "Caves should remain predominantly stone");
            assert!(
                special > 20,
                "Patches should be frequent, not isolated rarities"
            );
        }
    }
}
#[test]
fn original_surfaces_match_biome_materials() {
    let surface = Surface::new(17).unwrap();
    for (name, material) in [
        ("tidal_reach", "sand"),
        ("saltstrand", "sand"),
        ("abyssal_reach", "deepslate"),
        ("rainveil", "grass_block"),
        ("ochre_shelves", "terracotta"),
        ("rime_crown", "snow_block"),
        ("reed_basin", "mud"),
    ] {
        let mut c = Fixture {
            surface: &surface,
            biome: surface.biomes.id(name).unwrap(),
            profile: "dry",
            x: 0,
            top: 80,
            y: 79,
        };
        let result = surface.rules.apply(&mut c).unwrap();
        assert_eq!(crate::materials::get(result).source, material);
        assert_eq!(
            surface.by_biome[c.biome as usize].apply(&mut c),
            Some(result)
        );
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

#[test]
fn biome_boundaries_can_cross_inside_the_old_four_metre_cells() {
    use crate::worldgen::{Generator, TerrainSampler};
    let generator = Generator::new(1794608088350652100).unwrap();
    let mut sampler = generator.sampler();
    let mut cache = Cache::default();
    let mut fine_transition = false;
    let sample = |cache: &mut Cache, sampler: &mut TerrainSampler<'_>, x: f64, z: f64| {
        let ground = sampler.feature_ground(x, z).y;
        cache.biome(&generator.surface, sampler, [x, ground - 0.0625, z])
    };
    // Locate a real surface edge before testing its voxel-scale position. A
    // fixed Y/short strip can become a single biome when terrain is retuned.
    for z in [-256.0, -128.0, 0.0, 128.0, 256.0] {
        let mut previous = sample(&mut cache, &mut sampler, -512.0, z);
        for cell in -128..128 {
            let x = cell as f64 * 4.0;
            let end = sample(&mut cache, &mut sampler, x + 4.0, z);
            if previous != end {
                for voxel in 1..64 {
                    let id = sample(&mut cache, &mut sampler, x + voxel as f64 / 16.0, z);
                    if previous != id {
                        fine_transition = true;
                        break;
                    }
                    previous = id;
                }
            }
            previous = end;
            if fine_transition {
                break;
            }
        }
        if fine_transition {
            break;
        }
    }
    assert!(
        fine_transition,
        "Biome edges must not be pinned to the old coarse grid"
    );
}
