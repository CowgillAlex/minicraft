//! The source surface system at metre scale, extended across X and Z. Surface
//! rules replace raw stone only; density and aquifer sampling stay independent.
use super::{
    biomes::Table,
    config,
    noise::Normal,
    random::{string_hash, JavaRandom},
    surface_rules::Rule,
};
use crate::materials::*;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
#[path = "surface_column.rs"]
mod column;
pub use column::Cache;
pub struct Surface {
    pub(super) rules: Rule,
    pub(super) by_biome: Vec<Rule>,
    pub(super) biomes: Arc<Table>,
    pub(super) noises: Vec<Normal>,
    pub(super) seed: i64,
    pub(super) surface: usize,
    pub(super) secondary: usize,
    pub(super) clay: usize,
    pub(super) bands: [u8; 192],
    pub(super) transparent: Vec<u8>,
}
impl Surface {
    pub fn new(seed: i64) -> Result<Self, String> {
        let data: Value = serde_json::from_str(include_str!("../../assets/worldgen/surface.json"))
            .map_err(|e| e.to_string())?;
        let biomes = Table::shared()?;
        let mut indices = BTreeMap::new();
        let mut noises = Vec::new();
        for (name, params) in data["noises"].as_object().ok_or("Missing surface noises")? {
            indices.insert(name.to_owned(), noises.len());
            noises.push(config::normal(seed, name, params)?);
        }
        let rules = Rule::parse(&data["surface_rule"], &biomes, &indices)?;
        let transparent = rules.transparent_biomes();
        let by_biome = (0..biomes.len())
            .map(|id| {
                let name = biomes.name(id as u8);
                rules
                    .specialize(
                        id as u8,
                        name.contains("snowy") || name.contains("frozen") || name.contains("ice"),
                    )
                    .unwrap_or(Rule::Sequence(Vec::new()))
            })
            .collect();
        Ok(Self {
            rules,
            by_biome,
            biomes,
            noises,
            seed,
            surface: indices["surface"],
            secondary: indices["surface_secondary"],
            clay: indices["clay_bands_offset"],
            bands: bands(seed),
            transparent,
        })
    }
}
fn bands(seed: i64) -> [u8; 192] {
    let mut bands = [TERRACOTTA; 192];
    let mut random = JavaRandom::new(seed ^ string_hash("clay_bands"));
    let mut i = 0;
    while i < 192 {
        i += random.int(5) + 1;
        if i < 192 {
            bands[i] = ORANGE_TERRACOTTA;
        }
        i += 1;
    }
    for (width, color) in [
        (1, YELLOW_TERRACOTTA),
        (2, BROWN_TERRACOTTA),
        (1, RED_TERRACOTTA),
    ] {
        let count = random.int(10) + 6;
        for _ in 0..count {
            let width = width + random.int(3);
            let start = random.int(192);
            bands[start..(start + width).min(192)].fill(color);
        }
    }
    let count = random.int(7) + 9;
    let mut start = 0;
    for _ in 0..count {
        if start >= 192 {
            break;
        }
        bands[start] = WHITE_TERRACOTTA;
        if start > 1 && random.boolean() {
            bands[start - 1] = GRAY_TERRACOTTA;
        }
        if start + 1 < 192 && random.boolean() {
            bands[start + 1] = GRAY_TERRACOTTA;
        }
        start += random.int(16) + 4;
    }
    bands
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
