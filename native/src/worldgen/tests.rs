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
fn original_java_slice_matches_density_and_stone_water_air() {
    let data: Value =
        serde_json::from_str(include_str!("../../assets/worldgen/java-slice.json")).unwrap();
    let mut previous_seed = None;
    let mut generator = Generator::new(0).unwrap();
    let mut sampler = generator.sampler();
    let mut counts = [0; 3];
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
        let material = sampler.material(p);
        assert_eq!(
            material,
            row[5].as_u64().unwrap() as u8,
            "seed {seed}, {p:?}"
        );
        counts[match material {
            STONE => 0,
            WATER => 1,
            AIR => 2,
            _ => panic!("Unexpected material"),
        }] += 1;
    }
    assert!(
        counts.into_iter().all(|count| count > 0),
        "Fixtures must cover all three materials"
    );
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
        assert_eq!(
            sampler.material(p),
            row[5].as_u64().unwrap() as u8,
            "seed {seed}, {p:?}"
        );
    }
}

#[test]
fn generated_page_samples_at_voxel_spacing_with_the_requested_origin() {
    let preview = preview::Preview {
        seed: 1,
        origin: [125, 60, -1],
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
