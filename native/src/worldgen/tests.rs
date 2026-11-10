use super::*;
use crate::world::{AIR, STONE, WATER};
use serde_json::Value;

fn close(actual: f64, expected: f64, context: &str) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= 1e-12,
        "{context}: Rust {actual:.17}, Java {expected:.17}"
    );
}

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
fn original_java_slice_matches_density() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-slice.json")).unwrap();
    let mut previous_seed = None;
    let mut generator = Generator::new(0).unwrap();
    let mut sampler = generator.sampler();
    for row in data["samples"].as_array().unwrap() {
        let seed = row[0].as_i64().unwrap();
        if previous_seed != Some(seed) {
            generator = Generator::new(seed).unwrap();
            sampler = generator.sampler();
            previous_seed = Some(seed);
        }
        let p = std::array::from_fn(|i| row[i + 1].as_f64().unwrap());
        close(
            sampler.density(p),
            row[4].as_f64().unwrap(),
            &format!("seed {seed}, {p:?}"),
        );
    }
}

#[test]
fn fractional_and_negative_coordinates_match_java_3d_adapter() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-continuous.json")).unwrap();
    let mut previous_seed = None;
    let mut generator = Generator::new(0).unwrap();
    let mut sampler = generator.sampler();
    for row in data["samples"].as_array().unwrap() {
        let seed = row[0].as_i64().unwrap();
        if previous_seed != Some(seed) {
            generator = Generator::new(seed).unwrap();
            sampler = generator.sampler();
            previous_seed = Some(seed);
        }
        let p = std::array::from_fn(|i| row[i + 1].as_f64().unwrap());
        close(
            sampler.density(p),
            row[4].as_f64().unwrap(),
            &format!("seed {seed}, {p:?}"),
        );
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
fn three_dimensional_aquifer_matches_java_reference_and_cache_order() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-aquifer3d.json")).unwrap();
    for seed in [0, 1, 12345, -928341, i64::MIN, i64::MAX] {
        let generator = Generator::new(seed).unwrap();
        let rows: Vec<_> = data["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row[0].as_i64() == Some(seed))
            .collect();
        for reverse in [false, true] {
            let mut sampler = generator.sampler();
            for i in 0..rows.len() {
                let row = rows[if reverse { rows.len() - 1 - i } else { i }];
                let p = std::array::from_fn(|axis| row[axis + 1].as_f64().unwrap());
                assert_eq!(
                    sampler.material(p),
                    row[5].as_u64().unwrap() as u8,
                    "seed {seed}, {p:?}, reversed {reverse}"
                );
            }
        }
    }
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
fn prepared_fields_match_the_java_density_reference() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-continuous.json")).unwrap();
    for seed in [0, 1, 12345, -928341, i64::MIN, i64::MAX] {
        let generator = Generator::new(seed).unwrap();
        assert!(generator.plan.is_some(), "the bundled graph should compile");
        let mut sampler = generator.sampler();
        for row in data["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v[0].as_i64() == Some(seed))
        {
            let p = std::array::from_fn(|i| row[i + 1].as_f64().unwrap());
            let prepared = sampler.prepared(p, p).unwrap();
            close(
                prepared.sample(p),
                row[4].as_f64().unwrap(),
                &format!("prepared seed {seed}, {p:?}"),
            );
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
        let region = stream
            .results
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap();
        stream.received();
        let crate::terrain_stream::Update::Region(region) = region else {
            panic!("unexpected cache eviction")
        };
        assert_eq!(region.key, expected);
        assert!(!region.pages.is_empty());
        assert!(region
            .pages
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
    let region = stream
        .results
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    stream.received();
    let crate::terrain_stream::Update::Region(region) = region else {
        panic!("unexpected cache eviction")
    };
    assert_eq!(region.revision, 1);
    let light = region
        .light
        .expect("placed lamps require a regional light texture");
    assert!(light.as_chunks::<2>().0.iter().any(|p| p[0] > 100));
}
