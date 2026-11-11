//! Placement frequencies, height laws, exposure reduction and biome lists come
//! from the pinned placed/configured features. Deposits contain fine inclusions.
use super::{
    geometry::{Primitive as P, Replace},
    Rng,
};
use crate::{materials::*, worldgen::TerrainSampler};
use glam::DVec3 as V;
use serde_json::Value;
use std::sync::OnceLock;

pub fn rules() -> &'static [Value] {
    static DATA: OnceLock<Vec<Value>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/worldgen/ores.json"))
            .expect("bundled ore rules")
    })
}
fn anchor(v: &Value) -> i32 {
    if let Some(y) = v["absolute"].as_i64() {
        y as i32
    } else if let Some(y) = v["above_bottom"].as_i64() {
        -64 + y as i32
    } else {
        319 - v["below_top"].as_i64().unwrap() as i32
    }
}
pub fn height(rng: &mut Rng, height: &Value) -> i32 {
    let lo = anchor(&height["min_inclusive"]);
    let hi = anchor(&height["max_inclusive"]);
    let span = (hi - lo).max(0);
    if height["type"] == "minecraft:trapezoid" {
        let plateau = height["plateau"].as_i64().unwrap_or(0) as i32;
        let slope = (span - plateau).max(0) / 2;
        lo + rng.index((span - slope + 1) as usize) as i32 + rng.index((slope + 1) as usize) as i32
    } else {
        lo + rng.index((span + 1) as usize) as i32
    }
}
fn material(name: &str) -> u8 {
    if name.contains("coal") {
        COAL
    } else if name.contains("iron") {
        IRON
    } else if name.contains("copper") {
        COPPER
    } else if name.contains("gold") {
        GOLD
    } else if name.contains("redstone") {
        REDSTONE
    } else if name.contains("diamond") {
        DIAMOND
    } else if name.contains("lapis") {
        LAPIS
    } else {
        EMERALD
    }
}
pub fn column(sampler: &mut TerrainSampler<'_>, key: [i32; 3], seed: i64) -> Vec<P> {
    let mut out = Vec::new();
    for rule in rules() {
        let name = rule["name"].as_str().unwrap();
        let mut rng = Rng::new(seed, name, [key[0], 0, key[2]]);
        let placement = rule["placement"].as_array().unwrap();
        let mut count = 1;
        for modifier in placement {
            match modifier["type"].as_str().unwrap() {
                "minecraft:count" => {
                    let c = &modifier["count"];
                    count = c.as_u64().map_or_else(
                        || {
                            let lo = c["min_inclusive"].as_u64().unwrap() as usize;
                            lo + rng.index((c["max_inclusive"].as_u64().unwrap() as usize) - lo + 1)
                        },
                        |n| n as usize,
                    )
                }
                "minecraft:rarity_filter" => {
                    if rng.index(modifier["chance"].as_u64().unwrap() as usize) != 0 {
                        count = 0;
                    }
                }
                _ => {}
            }
        }
        let range = &placement
            .iter()
            .find(|v| v["type"] == "minecraft:height_range")
            .unwrap()["height"];
        for attempt in 0..count {
            let x = key[0] * 16 + rng.index(16) as i32;
            let z = key[2] * 16 + rng.index(16) as i32;
            let y = height(&mut rng, range);
            if !(-64..320).contains(&y) || y < key[1] * 16 - 8 || y > key[1] * 16 + 24 {
                continue;
            }
            let mut rng = Rng::new(
                seed,
                &format!("{name}/cluster"),
                [key[0], attempt as i32, key[2]],
            );
            let centre = V::new(x as f64, y as f64, z as f64);
            let biomes = rule["biomes"].as_array().unwrap();
            if biomes.len() < 56 {
                let biome = sampler.feature_habitat(centre.to_array()).1;
                if !biomes.iter().any(|v| v.as_str() == Some(biome.as_str())) {
                    continue;
                }
            }
            let size = rule["config"]["size"].as_u64().unwrap() as usize;
            let discard = rule["config"]["discard_chance_on_air_exposure"]
                .as_f64()
                .unwrap();
            let mut host = centre;
            for _ in 0..size {
                host += V::new(rng.signed(), rng.signed() * 0.6, rng.signed()) * 0.32;
                if sampler.material_fast(host.to_array()) != STONE {
                    continue;
                }
                if rng.unit() < discard
                    && [V::X, -V::X, V::Y, -V::Y, V::Z, -V::Z]
                        .into_iter()
                        .any(|d| sampler.material_fast((host + d).to_array()) == AIR)
                {
                    continue;
                }
                for _ in 0..3 {
                    let p = host + V::new(rng.signed(), rng.signed(), rng.signed()) * 0.32;
                    let r = 0.075 + rng.unit() * 0.085;
                    out.push(P::ellipsoid(
                        p,
                        V::new(r, r * (0.6 + rng.unit() * 0.8), r),
                        material(name),
                        Replace::Rock,
                        5,
                    ));
                }
            }
        }
    }
    out
}
