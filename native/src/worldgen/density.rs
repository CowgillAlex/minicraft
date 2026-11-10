use super::noise::{lerp, Blended, Normal};
use std::collections::HashMap;

pub type Id = usize;
#[path = "density_range.rs"]
mod range;

#[derive(Clone, Copy)]
struct Cell {
    key: [i32; 3],
    values: [f64; 8],
}

pub enum Node {
    Constant(f64),
    Y,
    Binary(Binary, Id, Id),
    Unary(Unary, Id),
    Clamp(Id, f64, f64),
    Range {
        input: Id,
        min: f64,
        max: f64,
        inside: Id,
        outside: Id,
    },
    Gradient {
        from_y: f64,
        to_y: f64,
        from: f64,
        to: f64,
    },
    Noise {
        noise: usize,
        xz: f64,
        y: f64,
    },
    Shift {
        noise: usize,
        b: bool,
    },
    Shifted {
        noise: usize,
        shift: [Id; 3],
        xz: f64,
        y: f64,
    },
    Weird {
        input: Id,
        noise: usize,
        type_one: bool,
    },
    Spline {
        coordinate: Id,
        points: Vec<SplinePoint>,
    },
    Blended(usize),
    Interpolated(Id),
}

pub struct SplinePoint {
    pub location: f32,
    pub derivative: f32,
    pub value: Id,
}

pub enum Binary {
    Add,
    Mul,
    Min,
    Max,
}
pub enum Unary {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Squeeze,
}

/// Immutable graph shared by workers. All mutable sampling caches belong to Sampler.
pub struct Density {
    pub nodes: Vec<Node>,
    pub noises: Vec<Normal>,
    pub blended: Vec<Blended>,
    pub roots: [Id; 5],
    pub min_y: i32,
    pub height: i32,
    pub sea_level: i32,
}

pub struct Sampler<'a> {
    pub density: &'a Density,
    last: Vec<Option<Cell>>,
    cells: HashMap<(Id, [i32; 3]), [f64; 8]>,
    corners: HashMap<(Id, [i32; 3]), f64>,
}

impl<'a> Sampler<'a> {
    pub fn new(density: &'a Density) -> Self {
        Self {
            density,
            last: vec![None; density.nodes.len()],
            cells: HashMap::new(),
            corners: HashMap::new(),
        }
    }

    pub fn root(&mut self, root: usize, point: [f64; 3]) -> f64 {
        self.eval(self.density.roots[root], point)
    }
    pub(super) fn interpolation_cell(
        &mut self,
        id: Id,
        point: [f64; 3],
    ) -> Option<([i32; 3], [f64; 8])> {
        if !matches!(self.density.nodes[id], Node::Interpolated(_)) {
            return None;
        }
        self.eval(id, point);
        self.last[id].map(|cell| (cell.key, cell.values))
    }

    pub fn eval(&mut self, id: Id, p: [f64; 3]) -> f64 {
        let graph = self.density;
        match &graph.nodes[id] {
            Node::Constant(value) => *value,
            Node::Y => p[1],
            Node::Binary(op, a, b) => {
                let a = self.eval(*a, p);
                let b = self.eval(*b, p);
                match op {
                    Binary::Add => a + b,
                    Binary::Mul => a * b,
                    Binary::Min => a.min(b),
                    Binary::Max => a.max(b),
                }
            }
            Node::Unary(op, input) => {
                let v = self.eval(*input, p);
                match op {
                    Unary::Abs => v.abs(),
                    Unary::Square => v * v,
                    Unary::Cube => v * v * v,
                    Unary::HalfNegative => {
                        if v > 0.0 {
                            v
                        } else {
                            v * 0.5
                        }
                    }
                    Unary::QuarterNegative => {
                        if v > 0.0 {
                            v
                        } else {
                            v * 0.25
                        }
                    }
                    Unary::Squeeze => {
                        let c = v.clamp(-1.0, 1.0);
                        c / 2.0 - c * c * c / 24.0
                    }
                }
            }
            Node::Clamp(input, min, max) => self.eval(*input, p).clamp(*min, *max),
            Node::Range {
                input,
                min,
                max,
                inside,
                outside,
            } => {
                let v = self.eval(*input, p);
                self.eval(
                    if v >= *min && v < *max {
                        *inside
                    } else {
                        *outside
                    },
                    p,
                )
            }
            Node::Gradient {
                from_y,
                to_y,
                from,
                to,
            } => {
                if p[1] <= *from_y {
                    *from
                } else if p[1] >= *to_y {
                    *to
                } else {
                    lerp((p[1] - from_y) / (to_y - from_y), *from, *to)
                }
            }
            Node::Noise { noise, xz, y } => {
                graph.noises[*noise].sample([p[0] * xz, p[1] * y, p[2] * xz])
            }
            Node::Shift { noise, b } => {
                let point = if *b {
                    [p[2] * 0.25, p[0] * 0.25, 0.0]
                } else {
                    [p[0] * 0.25, 0.0, p[2] * 0.25]
                };
                graph.noises[*noise].sample(point) * 4.0
            }
            Node::Shifted {
                noise,
                shift,
                xz,
                y,
            } => {
                let point = [
                    p[0] * xz + self.eval(shift[0], p),
                    p[1] * y + self.eval(shift[1], p),
                    p[2] * xz + self.eval(shift[2], p),
                ];
                graph.noises[*noise].sample(point)
            }
            Node::Weird {
                input,
                noise,
                type_one,
            } => {
                let v = self.eval(*input, p);
                let rarity = if *type_one {
                    if v < -0.5 {
                        0.75
                    } else if v < 0.0 {
                        1.0
                    } else if v < 0.5 {
                        1.5
                    } else {
                        2.0
                    }
                } else if v < -0.75 {
                    0.5
                } else if v < -0.5 {
                    0.75
                } else if v < 0.5 {
                    1.0
                } else if v < 0.75 {
                    2.0
                } else {
                    3.0
                };
                rarity * graph.noises[*noise].sample(p.map(|v| v / rarity)).abs()
            }
            Node::Spline { coordinate, points } => {
                // Java intentionally rounds coordinates and spline tangents to float.
                let input = self.eval(*coordinate, p) as f32;
                let upper = points.partition_point(|v| v.location <= input);
                if upper == 0 || upper == points.len() {
                    let point = if upper == 0 {
                        &points[0]
                    } else {
                        points.last().unwrap()
                    };
                    let value = self.eval(point.value, p);
                    return if point.derivative == 0.0 {
                        value
                    } else {
                        value + f64::from(point.derivative * (input - point.location))
                    };
                }
                let a = &points[upper - 1];
                let b = &points[upper];
                let t = f64::from((input - a.location) / (b.location - a.location));
                let y1 = self.eval(a.value, p);
                let y2 = self.eval(b.value, p);
                let slope = f64::from(b.location - a.location);
                let d1 = f64::from(a.derivative) * slope - (y2 - y1);
                let d2 = -f64::from(b.derivative) * slope + (y2 - y1);
                lerp(t, y1, y2) + t * (1.0 - t) * lerp(t, d1, d2)
            }
            Node::Blended(index) => graph.blended[*index].sample(p),
            Node::Interpolated(argument) => self.interpolate(id, *argument, p),
        }
    }

    fn interpolate(&mut self, id: Id, argument: Id, p: [f64; 3]) -> f64 {
        let sizes = [4.0, 8.0, 4.0];
        let cell: [i32; 3] = std::array::from_fn(|i| (p[i] / sizes[i]).floor() as i32);
        let key = (id, cell);
        let values = if let Some(cached) = self.last[id].filter(|v| v.key == cell) {
            cached.values
        } else {
            let values = if let Some(values) = self.cells.get(&key) {
                *values
            } else {
                let values = std::array::from_fn(|i| {
                    let corner = std::array::from_fn(|axis| cell[axis] + ((i >> axis) & 1) as i32);
                    if let Some(value) = self.corners.get(&(argument, corner)) {
                        return *value;
                    }
                    let value = self.eval(
                        argument,
                        std::array::from_fn(|axis| f64::from(corner[axis]) * sizes[axis]),
                    );
                    if self.corners.len() >= 8192 {
                        self.corners.clear();
                    }
                    self.corners.insert((argument, corner), value);
                    value
                });
                if self.cells.len() >= 2048 {
                    self.cells.clear();
                }
                self.cells.insert(key, values);
                values
            };
            self.last[id] = Some(Cell { key: cell, values });
            values
        };
        let t: [f64; 3] =
            std::array::from_fn(|i| (p[i] - f64::from(cell[i]) * sizes[i]) / sizes[i]);
        // Preserve the source's weighted bilinear arithmetic on each Z plane.
        let plane = |i| {
            (values[i] * (1.0 - t[0]) + values[i + 1] * t[0]) * (1.0 - t[1])
                + (values[i + 2] * (1.0 - t[0]) + values[i + 3] * t[0]) * t[1]
        };
        let a = plane(0);
        if t[2] == 0.0 {
            a
        } else {
            a * (1.0 - t[2]) + plane(4) * t[2]
        }
    }
}
