//! Original climate niches selected by nearest range, filtered by terrain and habitat.
use serde_json::Value;
use std::sync::{Arc, OnceLock};
pub type Id = u8;
#[derive(Clone, Copy)]
struct Entry {
    ranges: [[f64; 2]; 6],
    biome: Id,
    habitat: Habitat,
}
#[derive(Clone, Copy)]
enum Habitat {
    Land,
    Ocean,
    Wetland,
    Cave,
    Coast,
    DeepOcean,
    River,
    Pond,
}
const HABITATS: usize = 8;
#[path = "biome_index.rs"]
mod index;
pub struct Table {
    index: [index::Node; HABITATS],
    entries: Vec<Entry>,
    names: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    candidates: [Option<(usize, f64)>; HABITATS],
}
/// Sampled once per four-metre column; no decorated material queries are needed.
#[derive(Clone, Copy)]
pub(super) struct Environment {
    pub ground: f64,
    pub submerged: bool,
    pub water_depth: f64,
    pub coast: bool,
    pub wetland: bool,
    pub river: bool,
    pub pond: bool,
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
                habitat: match r[6].as_str().unwrap() {
                    "tidal_reach" => Habitat::Ocean,
                    "willow_run" | "frostflow" => Habitat::River,
                    "stillwater" => Habitat::Pond,
                    "abyssal_reach" => Habitat::DeepOcean,
                    "reed_basin" => Habitat::Wetland,
                    "rootvault" | "hollowstone" | "glimmerdeep" => Habitat::Cave,
                    "saltstrand" => Habitat::Coast,
                    _ => Habitat::Land,
                },
                biome: names
                    .binary_search(&r[6].as_str().unwrap().to_owned())
                    .unwrap() as Id,
            })
            .collect();
        for habitat in 0..HABITATS {
            if !entries.iter().any(|entry| entry.band() == habitat) {
                return Err(format!("Missing climate niche for habitat {habitat}"));
            }
        }
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
    /// Surface habitats cannot compete with caves or ocean floors by climate alone.
    pub fn sample_environment(&self, column: Column, y: f64, env: Environment) -> Id {
        // Cave climate only competes beneath a real cover depth. Shallow marshes
        // take priority over the ocean treatment at flooded, low-relief margins.
        let habitat = if y <= env.ground - 24.0 {
            Habitat::Cave
        } else if env.river {
            Habitat::River
        } else if env.pond {
            Habitat::Pond
        } else if env.wetland {
            Habitat::Wetland
        } else if env.submerged && env.water_depth >= 32.0 {
            Habitat::DeepOcean
        } else if env.submerged {
            Habitat::Ocean
        } else if env.coast {
            Habitat::Coast
        } else {
            Habitat::Land
        };
        let (entry, _) =
            column.candidates[habitat as usize].expect("Each habitat needs a climate niche");
        self.entries[entry].biome
    }
    #[cfg(test)]
    pub fn sample(&self, column: Column, depth: f64, niche: Id) -> Id {
        let band = self
            .entries
            .iter()
            .find(|entry| entry.biome == niche)
            .unwrap()
            .band();
        let mut best = f64::INFINITY;
        let mut biome = 0;
        for (index, horizontal) in column
            .candidates
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i == band)
            .filter_map(|(_, v)| v)
        {
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
        self.habitat as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn habitat_constraints_override_climate_without_losing_land_alternatives() {
        let table = Table::shared().unwrap();
        let column = table.column([0.0, 0.0, -0.65, 0.0, 0.0, 0.5]);
        let land = Environment {
            ground: 96.0,
            submerged: false,
            water_depth: 0.0,
            coast: false,
            wetland: false,
            river: false,
            pond: false,
        };
        let id = table.sample_environment(column, 96.0, land);
        assert!(!matches!(
            table.name(id),
            "tidal_reach"
                | "abyssal_reach"
                | "saltstrand"
                | "reed_basin"
                | "rootvault"
                | "hollowstone"
                | "glimmerdeep"
        ));
        let cases = [
            (
                Environment {
                    ground: 40.0,
                    submerged: true,
                    ..land
                },
                42.0,
                "tidal_reach",
            ),
            (
                Environment {
                    ground: 65.0,
                    coast: true,
                    ..land
                },
                66.0,
                "saltstrand",
            ),
            (
                Environment {
                    ground: 68.0,
                    wetland: true,
                    ..land
                },
                70.0,
                "reed_basin",
            ),
        ];
        for (env, y, expected) in cases {
            assert_eq!(
                table.name(table.sample_environment(column, y, env)),
                expected
            );
        }
        for y in [72.0, -62.0] {
            let id = table.sample_environment(column, y, land);
            assert!(matches!(
                table.name(id),
                "rootvault" | "hollowstone" | "glimmerdeep"
            ));
        }
        let id = table.sample_environment(column, 72.01, land);
        assert!(!matches!(
            table.name(id),
            "rootvault" | "hollowstone" | "glimmerdeep"
        ));
    }

    #[test]
    fn abyssal_reach_requires_deep_water_and_keeps_caves_beneath_the_seabed() {
        let table = Table::shared().unwrap();
        let column = table.column([0.0, 0.2, -0.7, 0.0, 0.0, 0.5]);
        let env = Environment {
            ground: 31.0,
            submerged: true,
            water_depth: 32.0,
            coast: false,
            wetland: false,
            river: false,
            pond: false,
        };
        assert_eq!(
            table.name(table.sample_environment(column, 32.0, env)),
            "abyssal_reach"
        );
        assert_eq!(
            table.name(table.sample_environment(
                column,
                32.0,
                Environment {
                    water_depth: 31.99,
                    ..env
                }
            )),
            "tidal_reach"
        );
        let dry = table.name(table.sample_environment(
            column,
            32.0,
            Environment {
                submerged: false,
                ..env
            },
        ));
        assert!(!matches!(dry, "tidal_reach" | "abyssal_reach"));
        let cave = table.name(table.sample_environment(column, 0.0, env));
        assert!(matches!(cave, "rootvault" | "hollowstone" | "glimmerdeep"));
    }

    #[test]
    fn raised_ocean_regressions_have_terrestrial_surfaces_and_match_debug() {
        use crate::worldgen::{surface::Cache, BiomeProbe, Generator};
        for (seed, x, z) in [(742, -190.0, -2366.0), (-91, -1470.0, -958.0)] {
            let generator = Generator::new(seed).unwrap();
            let mut sampler = generator.sampler();
            let ground = sampler.feature_ground(x, z).y;
            let p = [x, ground - 0.0625, z];
            let name = sampler.feature_habitat(p).1;
            if ground < 63.0 {
                assert_eq!(
                    sampler.material_fast([x, ground + 0.125, z]),
                    crate::world::WATER
                );
                let mut probe = BiomeProbe::new(seed).unwrap();
                probe.update(p);
                assert_eq!(probe.name(), name);
                continue;
            }
            assert!(!matches!(
                name.as_str(),
                "tidal_reach"
                    | "abyssal_reach"
                    | "saltstrand"
                    | "rootvault"
                    | "hollowstone"
                    | "glimmerdeep"
            ));
            assert_eq!(
                sampler.material_fast([x, ground + 0.125, z]),
                crate::world::AIR
            );
            let material = Cache::default().material(&generator.surface, &mut sampler, p);
            assert_ne!(material, crate::materials::SAND);
            let mut probe = BiomeProbe::new(seed).unwrap();
            probe.update(p);
            assert_eq!(probe.name(), name);
        }
    }

    #[test]
    fn sampled_surface_habitats_fit_the_terrain_across_seeds() {
        use crate::worldgen::Generator;
        for seed in [742, -91, 1, -67, i64::MIN, i64::MAX] {
            let generator = Generator::new(seed).unwrap();
            let mut sampler = generator.sampler();
            for x in (-1536..1536).step_by(256) {
                for z in (-1536..1536).step_by(256) {
                    let (x, z) = (f64::from(x + 2), f64::from(z + 2));
                    let h = sampler.feature_ground(x, z).y;
                    let name = sampler.feature_habitat([x, h - 0.0625, z]).1;
                    assert!(
                        !matches!(name.as_str(), "rootvault" | "hollowstone" | "glimmerdeep"),
                        "cave at surface: {seed}/{x}/{z}"
                    );
                    if matches!(name.as_str(), "tidal_reach" | "abyssal_reach") {
                        assert!(h < 63.0);
                        assert_eq!(
                            sampler.material_fast([x, h + 0.125, z]),
                            crate::world::WATER
                        );
                    }
                    if name == "saltstrand" {
                        assert!((63.0..=66.0).contains(&h));
                    }
                    if name == "reed_basin" {
                        assert!((59.0..=77.0).contains(&h));
                    }
                    let below = sampler.feature_habitat([x, h - 64.0, z]).1;
                    assert!(matches!(
                        below.as_str(),
                        "rootvault" | "hollowstone" | "glimmerdeep"
                    ));
                }
            }
        }
    }
}
