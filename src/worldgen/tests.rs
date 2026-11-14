use super::*;
use crate::world::{AIR, STONE, WATER};
use serde_json::Value;

#[test]
fn seed_stream_matches_java_including_signed_extremes() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-slice.json")).unwrap();
    for row in data["random"].as_array().unwrap() {
        let row = row.as_array().unwrap();
        let mut source = random::Random::new(row[0].as_i64().unwrap()).named("minecraft:terrain");
        for expected in &row[1..] {
            assert_eq!(
                source.next(),
                expected.as_str().unwrap().parse::<u64>().unwrap()
            );
        }
    }
}

#[test]
fn generated_page_samples_at_voxel_spacing_with_the_requested_origin() {
    let preview = preview::Preview {
        seed: 1,
        origin: [125, 60, -1],
        ..Default::default()
    };
    let generator = Generator::new(preview.seed).unwrap();
    let mut sampler = generator.sampler();
    let page = preview.page(&mut sampler, [32, 0, 32]);
    assert_eq!(page.len(), crate::world::CHUNK.pow(3));
    assert!(page.iter().all(|v| [AIR, STONE, WATER].contains(v)));
    let mut independent = generator.sampler();
    for [x, y, z] in [[0, 0, 0], [1, 0, 0], [15, 15, 15], [31, 31, 31]] {
        let p = [
            127.0 + x as f64 / 16.0,
            60.0 + y as f64 / 16.0,
            1.0 + z as f64 / 16.0,
        ];
        assert_eq!(page[x + 32 * (z + 32 * y)], independent.material(p));
    }
    assert_eq!(preview.point([1, 1, 1]), [125.0625, 60.0625, -0.9375]);
}

#[test]
fn uniform_page_skipping_matches_every_reference_voxel() {
    for seed in [1, 12345, -928341] {
        let generator = Generator::new(seed).unwrap();
        let mut optimized = generator.sampler();
        let mut reference = generator.sampler();
        for origin in [
            [120, -64, -20],
            [120, -54, -20],
            [120, -32, -20],
            [120, 60, -20],
            [120, 62, -20],
            [120, 64, -20],
            [120, 160, -20],
            [125, 61, -17],
        ] {
            let preview = preview::Preview {
                seed,
                origin,
                ..Default::default()
            };
            let actual = super::page::generate(preview, &mut optimized, [0; 3]);
            let expected = super::page::reference(preview, &mut reference, [0; 3]);
            for (index, expected) in expected.into_iter().enumerate() {
                assert_eq!(
                    actual.data.get(index),
                    expected,
                    "seed {seed}, origin {origin:?}, voxel {index}"
                );
            }
        }
    }
}

#[test]
fn reported_aquifer_pages_match_reference_and_have_no_exposed_water_sides() {
    let seed = 1794358123416304200;
    let generator = Generator::new(seed).unwrap();
    let mut optimized = generator.sampler();
    let mut reference = generator.sampler();
    let preview = preview::Preview {
        seed,
        origin: [0; 3],
        ..Default::default()
    };
    for origin in [[4288, 352, -2304], [5024, 992, 384], [5024, 992, 416]] {
        let actual = super::page::generate_raw(preview, &mut optimized, origin);
        let expected = super::page::reference(preview, &mut reference, origin);
        for (index, material) in expected.into_iter().enumerate() {
            assert_eq!(
                actual.data.get(index),
                material,
                "{origin:?}, voxel {index}"
            );
            if material != WATER {
                continue;
            }
            let cell = [
                origin[0] + (index % 32) as i32,
                origin[1] + (index / 1024) as i32,
                origin[2] + ((index / 32) % 32) as i32,
            ];
            // The upper water surface is free. Sides and undersides need a seal.
            for delta in [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 0, -1], [0, 0, 1]] {
                let neighbor = std::array::from_fn(|i| cell[i] + delta[i]);
                assert_ne!(
                    reference.material(preview.point(neighbor)),
                    AIR,
                    "exposed water at {cell:?} toward {delta:?}"
                );
            }
        }
    }
}

#[test]
fn preview_height_defaults_to_the_full_reference_range_and_old_origins_remain_valid() {
    let parse = |args: &[&str]| {
        preview::Scene::from_args(&args.iter().map(|v| v.to_string()).collect::<Vec<_>>())
    };
    let preview::Scene::Noise(full) = parse(&[]).unwrap() else {
        panic!("noise expected")
    };
    assert_eq!(full.height_m, 384);
    assert_eq!(full.origin[1], -64);
    let preview::Scene::Noise(small) = parse(&["--height", "12"]).unwrap() else {
        panic!("noise expected")
    };
    assert_eq!(small.height_m, 12);
    assert_eq!(small.origin[1], 58);
    assert!(parse(&["--height", "3"]).is_err());
    assert!(parse(&["--height", "384", "--origin", "120,58,-20"]).is_err());
    assert!(parse(&["--origin", "0,-32,0"]).is_ok());
}

#[test]
fn prepared_bounds_match_graph_intervals_and_enclose_density() {
    for seed in [0, 1, 12345, -928341, i64::MIN, i64::MAX] {
        let generator = Generator::new(seed).unwrap();
        let mut sampler = generator.sampler();
        for cell in [
            [0, -8, 0],
            [-450, 8, -514],
            [10, 4, 3],
            [34, 7, -10],
            [0, 39, 0],
        ] {
            for dimensions in [[3.5, 7.0, 3.5], [0.0, 7.0, 0.0], [1.5, 0.0, 1.5]] {
                let min = std::array::from_fn(|i| cell[i] as f64 * [4., 8., 4.][i] + 0.125);
                let max = std::array::from_fn(|i| min[i] + dimensions[i]);
                let prepared = sampler.prepared(min, max).unwrap();
                let actual = prepared.bounds(min, max);
                let expected = sampler.density.bounds(generator.density.roots[0], min, max);
                assert!(
                    (actual[0] - expected.low).abs() < 1e-12,
                    "{seed} {min:?}: {:?} vs {}",
                    actual,
                    expected.low
                );
                assert!((actual[1] - expected.high).abs() < 1e-12);
                for step in 0..=16 {
                    let point = std::array::from_fn(|i| min[i] + dimensions[i] * step as f64 / 16.);
                    let value = sampler.density(point);
                    assert!(value >= actual[0] - 1e-12 && value <= actual[1] + 1e-12);
                }
            }
        }
    }
}

#[test]
fn infinite_pages_preserve_signed_coordinates_and_session_edits_after_unload() {
    use crate::{
        chunk::Chunk,
        world::{World, AIR, LIGHT, STONE},
    };
    let mut world = World::with_height(384 * 16);
    world.infinite = true;
    let key = [-1, 3, -2];
    world.insert_page(key, Chunk::Uniform(STONE));
    let old = world.clone();
    world.set(-1, 100, -33, AIR);
    world.set(-2, 100, -33, LIGHT);
    assert_eq!(old.get(-1, 100, -33), STONE);
    assert_eq!(world.get(-1, 100, -33), AIR);
    world.unload(key);
    assert!(!world.is_loaded([-1, 100, -33]));
    world.insert_page(key, Chunk::Uniform(STONE));
    assert_eq!(world.get(-1, 100, -33), AIR);
    assert_eq!(world.get(-2, 100, -33), LIGHT);
    assert_eq!(world.voxels, 32_usize.pow(3) - 1);
    world.set(672, 100, 900, STONE);
    assert_eq!(world.get(672, 100, 900), STONE);
}

#[test]
fn infinite_pages_match_reference_on_both_sides_of_the_old_window() {
    for seed in [1, -12345] {
        let generator = Generator::new(seed).unwrap();
        let mut optimized = generator.sampler();
        let mut reference = generator.sampler();
        let preview = preview::Preview {
            seed,
            ..Default::default()
        };
        for origin in [[-32, 2000, 0], [640, 2000, -32], [672, 1000, 640]] {
            let page = super::page::generate(preview, &mut optimized, origin);
            let expected = super::page::reference(preview, &mut reference, origin);
            for (i, expected) in expected.into_iter().enumerate() {
                assert_eq!(page.data.get(i), expected);
            }
        }
    }
}

#[test]
fn infinite_worker_loads_positive_and_negative_regions_without_a_gpu() {
    use crate::{
        terrain_region::{self, center},
        terrain_stream::Stream,
        world::World,
        worldgen::sky::Sky,
    };
    use std::{collections::HashMap, time::Duration};
    let mut world = World::with_height(384 * 16);
    world.infinite = true;
    world.noise_preview = Some(Default::default());
    let mut stream = Stream::new(
        world,
        Sky::default(),
        center([5, 47, 0]),
        0.0,
        HashMap::new(),
    );
    for expected in [[5, 47, 0], [-7, 47, 13]] {
        if expected[0] < 0 {
            stream.request(center(expected), 0.0, HashMap::new(), None);
        }
        let collision = stream
            .results
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap();
        stream.received();
        let crate::terrain_stream::Update::Pages(collision) = collision else {
            panic!("player data must arrive before visual work");
        };
        let feet = crate::ray::grid_position(
            center(expected) - glam::Vec3::Y * crate::physics::EYE_HEIGHT,
        )
        .floor()
        .as_ivec3()
        .to_array()
        .map(|v| v.div_euclid(32));
        assert!(collision.iter().any(|(key, _)| *key == feet));
        let mut published = Vec::new();
        let region = loop {
            let result = stream
                .results
                .recv_timeout(Duration::from_secs(30))
                .unwrap()
                .unwrap();
            stream.received();
            match result {
                crate::terrain_stream::Update::Region(region) => break region,
                crate::terrain_stream::Update::Pages(pages) => published.extend(pages),
                _ => panic!("unexpected cache eviction"),
            }
        };
        assert_eq!(region.key, expected);
        assert!(region.pages.is_empty());
        assert!(!published.is_empty());
        assert!(published
            .iter()
            .all(|(key, _)| terrain_region::page_region(*key) == expected));
    }
    let target = [-7 * 128 + 64, 47 * 128 + 64, 13 * 128 + 64];
    let mut edited = World::with_height(384 * 16);
    edited.infinite = true;
    edited.noise_preview = Some(Default::default());
    edited.set(target[0], target[1], target[2], crate::world::LIGHT);
    stream.request(
        center([-7, 47, 13]),
        0.0,
        HashMap::new(),
        Some((&edited, &[target])),
    );
    let region = loop {
        let result = stream
            .results
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap();
        stream.received();
        match result {
            crate::terrain_stream::Update::Region(r) => break r,
            crate::terrain_stream::Update::Geometry(_) => {}
            crate::terrain_stream::Update::Cancelled { .. } => {}
            crate::terrain_stream::Update::Pages(_) => {}
            _ => panic!("unexpected eviction"),
        }
    };
    assert_eq!(region.revision, 1);
    let light = region
        .light
        .expect("placed lamps require a regional light texture");
    assert!(light.as_chunks::<2>().0.iter().any(|p| p[0] > 100));
}

#[test]
fn original_world_is_seeded_bounded_and_matches_fast_sampling() {
    let a = Generator::new(742).unwrap();
    let b = Generator::new(742).unwrap();
    let c = Generator::new(-91).unwrap();
    let (mut a, mut b, mut c) = (a.sampler(), b.sampler(), c.sampler());
    let mut differs = false;
    for x in [-1024.25, -19.5, 0.0, 127.75, 2048.0] {
        for z in [-517.5, 0.0, 823.25] {
            assert!(a.density([x, -64.0, z]) > 0.0);
            assert!(a.density([x, 319.0, z]) < 0.0);
            for y in [-32.5, 24.25, 63.0, 112.5, 200.0] {
                let p = [x, y, z];
                let value = a.density(p);
                assert!(value.is_finite());
                assert_eq!(value, b.density(p));
                assert!((value - a.density_fast(p)).abs() < 1e-10);
                differs |= (value - c.density(p)).abs() > 1e-6;
                assert_eq!(a.material(p), a.material_fast(p));
            }
        }
    }
    assert!(differs);
}

#[test]
fn original_biome_niches_select_their_own_centres() {
    let table = biomes::Table::shared().unwrap();
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("../../assets/worldgen/biomes.json")).unwrap();
    for row in rows {
        let climate = std::array::from_fn(|i| {
            (row[i][0].as_f64().unwrap() + row[i][1].as_f64().unwrap()) / 2.0
        });
        let id = table.sample(
            table.column(climate),
            climate[4],
            table.id(row[6].as_str().unwrap()).unwrap(),
        );
        assert_eq!(table.name(id), row[6].as_str().unwrap());
    }
}
