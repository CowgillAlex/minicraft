//! Diagnostic maps sampled from the same terrain and biome queries as the game.
use super::{biomes::Table, Generator};
use serde_json::{json, Value};

/// Read-only diagnostics: local gradients of actual generated ground, not the
/// broad elevation map (which would hide steep short-distance changes).
pub fn slopes(seed: i64) -> Result<Value, String> {
    let generator = Generator::new(seed)?;
    let mut sampler = generator.sampler();
    let mut samples = Vec::new();
    for iz in 0..32 {
        for ix in 0..32 {
            let x = -2048.0 + ix as f64 * 128.0 + 2.0;
            let z = -2048.0 + iz as f64 * 128.0 + 2.0;
            let h = sampler.feature_ground(x, z).y;
            let mut angles = [0.0; 2];
            let mut bank = false;
            for (i, radius) in [1.0, 4.0].into_iter().enumerate() {
                let neighbours = [
                    [x - radius, z],
                    [x + radius, z],
                    [x, z - radius],
                    [x, z + radius],
                ]
                .map(|[x, z]| sampler.feature_ground(x, z).y);
                let dx = (neighbours[1] - neighbours[0]) / (2.0 * radius);
                let dz = (neighbours[3] - neighbours[2]) / (2.0 * radius);
                angles[i] = dx.hypot(dz).atan().to_degrees();
                if i == 1 {
                    bank = neighbours.into_iter().any(|v| v < 63.0);
                }
            }
            samples.push(json!([
                x,
                z,
                h,
                (angles[0] * 100.0).round() / 100.0,
                (angles[1] * 100.0).round() / 100.0,
                bank
            ]));
        }
    }
    Ok(
        json!({"seed":seed.to_string(),"origin":[-2048,-2048],"size":32,"step_m":128,
        "derivative_spans_m":[2,8],"sea_level_m":63,"samples":samples,
        "fields":["x_m","z_m","ground_height_m","slope_2m_degrees","slope_8m_degrees","waterline_within_4m"]}),
    )
}

pub fn sample(seed: i64, size: usize, step: i32) -> Result<Value, String> {
    if !(2..=256).contains(&size) || !(4..=1024).contains(&step) || step % 4 != 0 {
        return Err("Map size must be 2..256; step must be a multiple of 4 in 4..1024".into());
    }
    let generator = Generator::new(seed)?;
    let mut sampler = generator.sampler();
    let table = Table::shared()?;
    let names: Vec<_> = (0..table.len()).map(|id| table.name(id as u8)).collect();
    let origin = -(size as i32 / 2) * step;
    let mut cells = Vec::with_capacity(size * size);
    let mut counts = vec![0; table.len()];
    let round = |v: f64| (v * 100.0).round() / 100.0;
    for z in 0..size {
        for x in 0..size {
            // Align with the centres of the game's four-metre biome cells.
            let px = f64::from(origin + x as i32 * step + 2);
            let pz = f64::from(origin + z as i32 * step + 2);
            let height = sampler.feature_ground(px, pz).y;
            let (_, surface) = sampler.feature_habitat([px, height - 0.0625, pz]);
            let (_, cave) = sampler.feature_habitat([px, height - 64.0, pz]);
            let id = table.id(&surface)?;
            counts[id as usize] += 1;
            let climate = sampler.climate([px, height, pz]);
            cells.push(json!([
                round(height),
                id,
                table.id(&cave)?,
                round(climate[0]),
                round(climate[1]),
                round(climate[2]),
                round(climate[3])
            ]));
        }
    }
    let mut section = String::with_capacity(size * 96);
    for y in (0..96).rev() {
        for x in 0..size {
            let p = [
                f64::from(origin + x as i32 * step + 2),
                -62.0 + y as f64 * 4.0,
                2.0,
            ];
            section.push(if sampler.density_fast(p) > 0.0 {
                '1'
            } else {
                '0'
            });
        }
    }
    Ok(
        json!({"seed": seed.to_string(), "size": size, "step": step, "origin": origin,
        "sea_level": 63, "names": names, "counts": counts, "cells": cells,
        "section": section, "section_z": 2, "section_top": 318, "section_step": 4,
        "fields": ["surface_height_m", "surface_biome_id", "biome_64m_below_surface_id", "temperature", "moisture", "landmass", "erosion"]}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn surface_has_mostly_rolling_land_without_losing_relief_or_deep_water() {
        for seed in [742, 42] {
            let generator = super::Generator::new(seed).unwrap();
            let mut sampler = generator.sampler();
            let (mut land, mut gentle, mut steep) = (0, 0, 0);
            let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
            for iz in 0..16 {
                for ix in 0..16 {
                    let x = -2046.0 + ix as f64 * 256.0;
                    let z = -2046.0 + iz as f64 * 256.0;
                    let h = sampler.feature_ground(x, z).y;
                    low = low.min(h);
                    high = high.max(h);
                    if h < 63.0 {
                        continue;
                    }
                    let dx = (sampler.feature_ground(x + 4.0, z).y
                        - sampler.feature_ground(x - 4.0, z).y)
                        / 8.0;
                    let dz = (sampler.feature_ground(x, z + 4.0).y
                        - sampler.feature_ground(x, z - 4.0).y)
                        / 8.0;
                    let slope = dx.hypot(dz).atan().to_degrees();
                    land += 1;
                    gentle += usize::from(slope < 25.0);
                    steep += usize::from(slope > 30.0);
                }
            }
            assert!(
                land > 64 && land < 192,
                "Keep both land and water: {seed}/{land}"
            );
            assert!(
                gentle * 100 > land * 60,
                "Most land should roll gently: {seed}/{gentle}/{land}"
            );
            assert!(
                steep * 100 > land * 2,
                "Retain some steep terrain: {seed}/{steep}/{land}"
            );
            assert!(
                low < 31.0 && high > 110.0,
                "Keep deep water and uplands: {seed}/{low}/{high}"
            );
        }
    }

    #[test]
    fn surface_water_pages_match_literal_sampling_and_have_sealed_sides() {
        use crate::world::{AIR, WATER};
        for (seed, x, z) in [(742, 624, -752), (1794608088350652100, -560, -736)] {
            let generator = super::Generator::new(seed).unwrap();
            let mut optimized = generator.sampler();
            let mut reference = generator.sampler();
            let preview = super::super::preview::Preview {
                seed,
                origin: [0; 3],
                ..Default::default()
            };
            let mut water = 0;
            for y in [58, 62] {
                let origin = [x * 16, y * 16, z * 16];
                let page = super::super::page::generate_raw(preview, &mut optimized, origin);
                let literal = super::super::page::reference(preview, &mut reference, origin);
                for (i, m) in literal.into_iter().enumerate() {
                    assert_eq!(page.data.get(i), m, "{seed}/{origin:?}/{i}");
                    if m != WATER {
                        continue;
                    }
                    water += 1;
                    if i % 127 != 0 {
                        continue;
                    }
                    let cell = [
                        origin[0] + (i % 32) as i32,
                        origin[1] + (i / 1024) as i32,
                        origin[2] + ((i / 32) % 32) as i32,
                    ];
                    for delta in [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 0, -1], [0, 0, 1]] {
                        let neighbour = std::array::from_fn(|a| cell[a] + delta[a]);
                        assert_ne!(
                            reference.material(preview.point(neighbour)),
                            AIR,
                            "Open water side {seed}/{cell:?}/{delta:?}"
                        );
                    }
                }
            }
            assert!(water > 0);
        }
    }
    #[test]
    fn seeded_rivers_and_ponds_have_water_beds_and_a_level_surface() {
        for seed in [742, 1794608088350652100] {
            let generator = super::Generator::new(seed).unwrap();
            let mut sampler = generator.sampler();
            let mut river = None;
            let mut pond = None;
            for z in (-768..768).step_by(16) {
                for x in (-768..768).step_by(16) {
                    let p = [x as f64, 63.0, z as f64];
                    let ids = sampler.density.density.waterways.unwrap();
                    let route = ids.map(|id| sampler.density.eval(id, p));
                    if route[0] > 0.014 && route[1] < 0.4 {
                        continue;
                    }
                    let ground = sampler.feature_ground(p[0], p[2]).y;
                    let name = sampler.feature_habitat([p[0], ground - 0.0625, p[2]]).1;
                    if !matches!(name.as_str(), "willow_run" | "frostflow" | "stillwater") {
                        continue;
                    }
                    assert!(ground < 63.0);
                    for y in [ground + 0.125, 62.9375] {
                        assert_eq!(sampler.material_fast([p[0], y, p[2]]), crate::world::WATER);
                    }
                    assert_eq!(sampler.material_fast([p[0], 63.0, p[2]]), crate::world::AIR);
                    assert_eq!(
                        sampler.material_fast([p[0], ground - 0.0625, p[2]]),
                        crate::world::STONE
                    );
                    if name == "stillwater" {
                        pond = Some((x, z, ground));
                    } else {
                        river = Some((x, z, ground));
                    }
                    if river.is_some() && pond.is_some() {
                        break;
                    }
                }
                if river.is_some() && pond.is_some() {
                    break;
                }
            }
            println!("SURFACE WATER {seed}: river={river:?}, pond={pond:?}");
            assert!(
                river.is_some() && pond.is_some(),
                "Expected both rivers and ponds for seed {seed}"
            );
        }
    }
    #[test]
    fn map_reports_actual_surface_and_signed_coordinates() {
        let map = super::sample(742, 2, 64).unwrap();
        assert_eq!(map["origin"], -64);
        assert_eq!(map["cells"].as_array().unwrap().len(), 4);
        let generator = super::Generator::new(742).unwrap();
        let mut sampler = generator.sampler();
        let h = sampler.feature_ground(-62.0, -62.0).y;
        assert!((map["cells"][0][0].as_f64().unwrap() - h).abs() < 0.01);
        let name = sampler.feature_habitat([-62.0, h - 0.0625, -62.0]).1;
        let id = map["cells"][0][1].as_u64().unwrap() as usize;
        assert_eq!(map["names"][id], name);
        assert_eq!(map["section"].as_str().unwrap().len(), 192);
    }
}
