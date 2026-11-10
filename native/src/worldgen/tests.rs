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
