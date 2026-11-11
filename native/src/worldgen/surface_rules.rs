//! JSON rule order and condition semantics follow the pinned SurfaceRules port.
use super::biomes::{Id, Table};
use serde_json::Value;
use std::collections::BTreeMap;
pub enum Rule {
    Sequence(Vec<Rule>),
    If(Condition, Box<Rule>),
    Block(u8),
    Bands,
}
pub enum Condition {
    Biome(Vec<Id>),
    Noise {
        index: usize,
        min: f64,
        max: f64,
    },
    Not(Box<Condition>),
    Y {
        anchor: f64,
        mult: f64,
        depth: bool,
    },
    Water {
        offset: f64,
        mult: f64,
        depth: bool,
    },
    Stone {
        offset: f64,
        surface: bool,
        secondary: f64,
        ceiling: bool,
    },
    Gradient {
        name: String,
        low: f64,
        high: f64,
    },
    Steep,
    Hole,
    Temperature,
    Preliminary,
}
pub trait Context {
    fn y(&self) -> f64;
    fn biome(&mut self) -> Id;
    fn cold(&mut self) -> bool;
    fn noise(&mut self, index: usize) -> f64;
    fn surface_depth(&mut self) -> f64;
    fn secondary(&mut self) -> f64;
    fn above(&mut self) -> f64;
    fn below(&mut self) -> f64;
    fn water(&mut self) -> Option<f64>;
    fn steep(&mut self) -> bool;
    fn preliminary(&mut self) -> f64;
    fn random(&self, name: &str) -> f64;
    fn band(&mut self) -> u8;
}
fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.0)
}
fn flag(v: &Value, key: &str) -> bool {
    v[key].as_bool().unwrap_or(false)
}
fn anchor(v: &Value) -> Result<f64, String> {
    if let Some(n) = v["absolute"].as_f64() {
        Ok(n)
    } else if let Some(n) = v["above_bottom"].as_f64() {
        Ok(-64.0 + n)
    } else if let Some(n) = v["below_top"].as_f64() {
        Ok(319.0 - n)
    } else {
        Err("Unknown surface height anchor".into())
    }
}
impl Rule {
    pub fn parse(
        v: &Value,
        biomes: &Table,
        noises: &BTreeMap<String, usize>,
    ) -> Result<Self, String> {
        let kind = v["type"]
            .as_str()
            .ok_or("Missing surface rule type")?
            .trim_start_matches("minecraft:");
        Ok(match kind {
            "sequence" => Self::Sequence(
                v["sequence"]
                    .as_array()
                    .ok_or("Missing surface sequence")?
                    .iter()
                    .map(|v| Self::parse(v, biomes, noises))
                    .collect::<Result<_, _>>()?,
            ),
            "condition" => Self::If(
                Condition::parse(&v["if_true"], biomes, noises)?,
                Box::new(Self::parse(&v["then_run"], biomes, noises)?),
            ),
            "block" => Self::Block(crate::materials::from_source(
                v["result_state"]["Name"]
                    .as_str()
                    .ok_or("Missing surface block")?,
            )?),
            "bandlands" | "badlands" => Self::Bands,
            _ => return Err(format!("Unsupported surface rule {kind}")),
        })
    }
    pub fn apply(&self, c: &mut impl Context) -> Option<u8> {
        match self {
            Self::Sequence(rules) => rules.iter().find_map(|r| r.apply(c)),
            Self::If(condition, rule) => {
                if condition.test(c) {
                    rule.apply(c)
                } else {
                    None
                }
            }
            Self::Block(m) => Some(*m),
            Self::Bands => Some(c.band()),
        }
    }
    /// Unknown depth/water/preliminary conditions are kept conservative here.
    pub fn changes_opacity(&self, c: &mut impl Context) -> bool {
        match self {
            Self::Sequence(r) => r.iter().any(|r| r.changes_opacity(c)),
            Self::If(condition, r) => condition.partial(c) != Some(false) && r.changes_opacity(c),
            Self::Block(m) => matches!(*m, crate::world::AIR | crate::world::WATER),
            Self::Bands => false,
        }
    }
    pub fn transparent_biomes(&self) -> Vec<Id> {
        fn collect(rule: &Rule, parents: &[Id], out: &mut Vec<Id>) {
            match rule {
                Rule::Sequence(rules) => {
                    for r in rules {
                        collect(r, parents, out)
                    }
                }
                Rule::If(Condition::Biome(ids), r) => collect(r, ids, out),
                Rule::If(_, r) => collect(r, parents, out),
                Rule::Block(m) if matches!(*m, crate::world::AIR | crate::world::WATER) => {
                    out.extend_from_slice(parents)
                }
                _ => {}
            }
        }
        let mut ids = Vec::new();
        collect(self, &[], &mut ids);
        ids.sort();
        ids.dedup();
        ids
    }
}
impl Condition {
    fn parse(v: &Value, biomes: &Table, noises: &BTreeMap<String, usize>) -> Result<Self, String> {
        let kind = v["type"]
            .as_str()
            .ok_or("Missing surface condition")?
            .trim_start_matches("minecraft:");
        Ok(match kind {
            "biome" => Self::Biome(
                v["biome_is"]
                    .as_array()
                    .ok_or("Missing biome list")?
                    .iter()
                    .map(|s| biomes.id(s.as_str().unwrap()))
                    .collect::<Result<_, _>>()?,
            ),
            "noise_threshold" => Self::Noise {
                index: *noises
                    .get(
                        v["noise"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:"),
                    )
                    .ok_or("Missing surface noise")?,
                min: num(v, "min_threshold"),
                max: num(v, "max_threshold"),
            },
            "not" => Self::Not(Box::new(Self::parse(&v["invert"], biomes, noises)?)),
            "y_above" => Self::Y {
                anchor: anchor(&v["anchor"])?,
                mult: num(v, "surface_depth_multiplier"),
                depth: flag(v, "add_stone_depth"),
            },
            "water" => Self::Water {
                offset: num(v, "offset"),
                mult: num(v, "surface_depth_multiplier"),
                depth: flag(v, "add_stone_depth"),
            },
            "stone_depth" => Self::Stone {
                offset: num(v, "offset"),
                surface: flag(v, "add_surface_depth"),
                secondary: num(v, "secondary_depth_range"),
                ceiling: v["surface_type"] == "ceiling",
            },
            "vertical_gradient" => Self::Gradient {
                name: v["random_name"]
                    .as_str()
                    .unwrap_or("minecraft:surface")
                    .to_owned(),
                low: anchor(&v["true_at_and_below"])?,
                high: anchor(&v["false_at_and_above"])?,
            },
            "steep" => Self::Steep,
            "hole" => Self::Hole,
            "temperature" => Self::Temperature,
            "above_preliminary_surface" => Self::Preliminary,
            _ => return Err(format!("Unsupported surface condition {kind}")),
        })
    }
    fn partial(&self, c: &mut impl Context) -> Option<bool> {
        Some(match self {
            Self::Biome(ids) => ids.contains(&c.biome()),
            Self::Noise { index, min, max } => {
                let v = c.noise(*index);
                v >= *min && v <= *max
            }
            Self::Not(condition) => return condition.partial(c).map(|v| !v),
            Self::Hole => c.surface_depth() <= 0.0,
            Self::Temperature => c.cold(),
            Self::Y {
                anchor,
                mult,
                depth: false,
            } => c.y() >= anchor + c.surface_depth() * mult,
            Self::Gradient { name, low, high } => {
                c.y() <= *low
                    || (c.y() < *high && c.random(name) < 1.0 - (c.y() - low) / (high - low))
            }
            _ => return None,
        })
    }
    pub fn test(&self, c: &mut impl Context) -> bool {
        if let Some(v) = self.partial(c) {
            return v;
        }
        match self {
            Self::Not(condition) => !condition.test(c),
            Self::Y {
                anchor,
                mult,
                depth,
            } => c.y() + if *depth { c.above() } else { 0.0 } >= anchor + c.surface_depth() * mult,
            Self::Water {
                offset,
                mult,
                depth,
            } => match c.water() {
                None => *offset <= 0.0,
                Some(w) => {
                    c.y() + if *depth { c.above() } else { 0.0 }
                        >= w + offset + c.surface_depth() * mult
                }
            },
            Self::Stone {
                offset,
                surface,
                secondary,
                ceiling,
            } => {
                let d = if *ceiling { c.below() } else { c.above() };
                let s = if *surface { c.surface_depth() } else { 0.0 };
                let second = if *secondary == 0.0 {
                    0.0
                } else {
                    ((c.secondary() + 1.0) * 0.5 * secondary) as i32 as f64
                };
                d <= 1.0 + offset + s + second
            }
            Self::Steep => c.steep(),
            Self::Preliminary => c.y() >= c.preliminary(),
            _ => unreachable!(),
        }
    }
}
