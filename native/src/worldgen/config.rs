use super::{
    density::{Binary, Density, Id, Node, SplinePoint, Unary},
    noise::{Blended, Normal},
    random::Random,
};
use serde_json::Value;
use std::collections::HashMap;

struct Builder<'a> {
    seed: i64,
    data: &'a Value,
    graph: Density,
    references: HashMap<String, Id>,
    noises: HashMap<String, usize>,
}

impl Density {
    pub fn new(seed: i64) -> Result<Self, String> {
        let data: Value =
            serde_json::from_str(include_str!("../../assets/worldgen/overworld.json"))
                .map_err(|e| e.to_string())?;
        let mut builder = Builder {
            seed,
            data: &data,
            graph: Self {
                nodes: Vec::new(),
                noises: Vec::new(),
                blended: Vec::new(),
                roots: [0; 5],
                min_y: number(&data["noise"], "min_y")? as i32,
                height: number(&data["noise"], "height")? as i32,
                sea_level: number(&data, "sea_level")? as i32,
            },
            references: HashMap::new(),
            noises: HashMap::new(),
        };
        for (index, name) in [
            "final_density",
            "initial_density_without_jaggedness",
            "barrier",
            "fluid_level_floodedness",
            "fluid_level_spread",
        ]
        .iter()
        .enumerate()
        {
            builder.graph.roots[index] = builder.parse(&data["router"][name])?;
        }
        Ok(builder.graph)
    }
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    value[key]
        .as_f64()
        .ok_or_else(|| format!("Missing worldgen number {key}"))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("Missing worldgen string {key}"))
}
fn path(value: &str) -> &str {
    value.strip_prefix("minecraft:").unwrap_or(value)
}

impl Builder<'_> {
    fn noise(&mut self, name: &str) -> Result<usize, String> {
        let name = path(name);
        if let Some(index) = self.noises.get(name) {
            return Ok(*index);
        }
        let params = &self.data["noises"][name];
        let octave = params
            .get("base_octave")
            .or_else(|| params.get("firstOctave"))
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;
        let values = params
            .get("amplitudes")
            .or_else(|| params.get("amplitude_modifiers"))
            .and_then(Value::as_array)
            .ok_or_else(|| format!("Missing noise {name}"))?;
        let mut amplitudes: Vec<f64> = values
            .iter()
            .map(|v| v.as_f64().ok_or("Invalid amplitude".to_string()))
            .collect::<Result<_, _>>()?;
        if let Some(count) = params["octave_count"].as_u64() {
            amplitudes.resize(amplitudes.len().max(count as usize), 0.0);
        }
        if let Some(base) = params["base_amplitude"].as_f64() {
            amplitudes.insert(0, base);
        }
        let mut source = Random::new(self.seed).named(&format!("minecraft:{name}"));
        let index = self.graph.noises.len();
        self.graph
            .noises
            .push(Normal::new(&mut source, octave, &amplitudes));
        self.noises.insert(name.to_owned(), index);
        Ok(index)
    }

    fn reference(&mut self, name: &str) -> Result<Id, String> {
        let name = path(name);
        if let Some(id) = self.references.get(name) {
            return Ok(*id);
        }
        let value = self.data["density_functions"]
            .get(name)
            .ok_or_else(|| format!("Missing density {name}"))?;
        let id = self.parse(value)?;
        self.references.insert(name.to_owned(), id);
        Ok(id)
    }

    fn parse(&mut self, v: &Value) -> Result<Id, String> {
        let node = if let Some(value) = v.as_f64() {
            Node::Constant(value)
        } else if let Some(name) = v.as_str() {
            match path(name) {
                "y" => Node::Y,
                "zero" => Node::Constant(0.0),
                _ => return self.reference(name),
            }
        } else {
            let kind = if v.get("type").is_none() && v.get("coordinate").is_some() {
                "spline_value"
            } else {
                path(text(v, "type")?)
            };
            match kind {
                "noise" => Node::Noise {
                    noise: self.noise(text(v, "noise")?)?,
                    xz: v["xz_scale"].as_f64().unwrap_or(1.0),
                    y: v["y_scale"].as_f64().unwrap_or(1.0),
                },
                "old_blended_noise" => {
                    let index = self.graph.blended.len();
                    self.graph.blended.push(Blended::new(
                        self.seed,
                        number(v, "xz_scale")?,
                        number(v, "y_scale")?,
                        number(v, "xz_factor")?,
                        number(v, "y_factor")?,
                        number(v, "smear_scale_multiplier")?,
                    ));
                    Node::Blended(index)
                }
                "shifted_noise" => Node::Shifted {
                    noise: self.noise(text(v, "noise")?)?,
                    shift: [
                        self.parse(&v["shift_x"])?,
                        self.parse(&v["shift_y"])?,
                        self.parse(&v["shift_z"])?,
                    ],
                    xz: number(v, "xz_scale")?,
                    y: number(v, "y_scale")?,
                },
                "shift_a" | "shift_b" => Node::Shift {
                    noise: self.noise(text(v, "argument")?)?,
                    b: kind == "shift_b",
                },
                "add" | "mul" | "min" | "max" => {
                    let op = match kind {
                        "add" => Binary::Add,
                        "mul" => Binary::Mul,
                        "min" => Binary::Min,
                        _ => Binary::Max,
                    };
                    Node::Binary(
                        op,
                        self.parse(&v["argument1"])?,
                        self.parse(&v["argument2"])?,
                    )
                }
                "abs" | "square" | "cube" | "half_negative" | "quarter_negative" | "squeeze" => {
                    let op = match kind {
                        "abs" => Unary::Abs,
                        "square" => Unary::Square,
                        "cube" => Unary::Cube,
                        "half_negative" => Unary::HalfNegative,
                        "quarter_negative" => Unary::QuarterNegative,
                        _ => Unary::Squeeze,
                    };
                    Node::Unary(op, self.parse(&v["argument"])?)
                }
                "clamp" => Node::Clamp(
                    self.parse(&v["input"])?,
                    number(v, "min")?,
                    number(v, "max")?,
                ),
                "range_choice" => Node::Range {
                    input: self.parse(&v["input"])?,
                    min: number(v, "min_inclusive")?,
                    max: number(v, "max_exclusive")?,
                    inside: self.parse(&v["when_in_range"])?,
                    outside: self.parse(&v["when_out_of_range"])?,
                },
                "y_clamped_gradient" => Node::Gradient {
                    from_y: number(v, "from_y")?,
                    to_y: number(v, "to_y")?,
                    from: number(v, "from_value")?,
                    to: number(v, "to_value")?,
                },
                "vertical_gradient" => Node::Gradient {
                    from_y: number(&v["from_y"], "absolute")?,
                    to_y: number(&v["to_y"], "absolute")?,
                    from: number(v, "from_value")?,
                    to: number(v, "to_value")?,
                },
                "interpolated" => Node::Interpolated(self.parse(&v["argument"])?),
                "cache_2d" | "cache_once" | "flat_cache" | "blend_density" => {
                    return self.parse(&v["argument"])
                }
                "blend_alpha" => Node::Constant(1.0),
                "blend_offset" => Node::Constant(0.0),
                "spline" | "spline_value" => {
                    let spline = if kind == "spline" { &v["spline"] } else { v };
                    let coordinate = self.parse(&spline["coordinate"])?;
                    let mut points = Vec::new();
                    for point in spline["points"].as_array().ok_or("Missing spline points")? {
                        points.push(SplinePoint {
                            location: number(point, "location")? as f32,
                            derivative: number(point, "derivative")? as f32,
                            value: self.parse(&point["value"])?,
                        });
                    }
                    if points.is_empty() {
                        return Err("Empty spline".into());
                    }
                    Node::Spline { coordinate, points }
                }
                "weird_scaled_sampler" => Node::Weird {
                    input: self.parse(&v["input"])?,
                    noise: self.noise(text(v, "noise")?)?,
                    type_one: text(v, "rarity_value_mapper")? == "type_1",
                },
                _ => return Err(format!("Unsupported density function {kind}")),
            }
        };
        let id = self.graph.nodes.len();
        self.graph.nodes.push(node);
        Ok(id)
    }
}
