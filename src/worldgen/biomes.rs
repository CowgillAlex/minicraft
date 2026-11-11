//! The pinned Java BiomeManager exports its parameter table. Four depth-band
//! candidates preserve the source column sampler and its first-entry tie order.
use serde_json::Value;
use std::sync::{Arc, OnceLock};
pub type Id = u8;
#[derive(Clone, Copy)]
struct Entry {
    ranges: [[f64; 2]; 6],
    biome: Id,
}
#[path = "biome_index.rs"]
mod index;
pub struct Table {
    index: [index::Node; 4],
    entries: Vec<Entry>,
    names: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    candidates: [Option<(usize, f64)>; 4],
}
fn distance([min, max]: [f64; 2], v: f64) -> f64 {
    if v < min {
        min - v
    } else if v > max {
        v - max
    } else {
        0.0
    }
}
impl Table {
    pub fn shared() -> Result<Arc<Self>, String> {
        static TABLE: OnceLock<Result<Arc<Table>, String>> = OnceLock::new();
        TABLE.get_or_init(Self::load).clone()
    }
    fn load() -> Result<Arc<Self>, String> {
        let rows: Vec<Value> =
            serde_json::from_str(include_str!("../../assets/worldgen/biomes.json"))
                .map_err(|e| e.to_string())?;
        let mut names: Vec<_> = rows
            .iter()
            .map(|r| r[6].as_str().unwrap().to_owned())
            .collect();
        names.sort();
        names.dedup();
        if names.len() > u8::MAX as usize {
            return Err("Too many biomes".into());
        }
        let entries: Vec<Entry> = rows
            .iter()
            .map(|r| Entry {
                ranges: std::array::from_fn(|a| {
                    [r[a][0].as_f64().unwrap(), r[a][1].as_f64().unwrap()]
                }),
                biome: names
                    .binary_search(&r[6].as_str().unwrap().to_owned())
                    .unwrap() as Id,
            })
            .collect();
        let index = std::array::from_fn(|band| {
            index::Node::build(
                &entries,
                (0..entries.len())
                    .filter(|&i| entries[i].band() == band)
                    .collect(),
            )
        });
        Ok(Arc::new(Self {
            entries,
            names,
            index,
        }))
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn id(&self, name: &str) -> Result<Id, String> {
        self.names
            .binary_search(&name.strip_prefix("minecraft:").unwrap_or(name).to_owned())
            .map(|i| i as Id)
            .map_err(|_| format!("Unknown biome {name}"))
    }
    pub fn name(&self, id: Id) -> &str {
        &self.names[id as usize]
    }
    pub fn column(&self, climate: [f64; 6]) -> Column {
        let candidates =
            std::array::from_fn(|band| self.index[band].nearest(&self.entries, climate));
        Column { candidates }
    }
    pub fn sample(&self, column: Column, depth: f64) -> Id {
        let mut best = f64::INFINITY;
        let mut biome = 0;
        for (index, horizontal) in column.candidates.into_iter().flatten() {
            let entry = self.entries[index];
            let score = horizontal + distance(entry.ranges[4], depth).powi(2);
            if score < best {
                best = score;
                biome = entry.biome;
            }
        }
        biome
    }
}

impl Entry {
    fn band(&self) -> usize {
        match self.ranges[4][0] {
            0.0 => 0,
            1.0 => 1,
            1.1 => 3,
            _ => 2,
        }
    }
}
