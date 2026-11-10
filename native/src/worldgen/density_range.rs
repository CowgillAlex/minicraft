use super::{Binary, Id, Node, Sampler, Unary};

/// A conservative range over a box. Unknown operations deliberately prevent skipping.
#[derive(Clone, Copy)]
pub struct Bounds {
    pub low: f64,
    pub high: f64,
}
impl Bounds {
    fn point(value: f64) -> Self {
        Self {
            low: value,
            high: value,
        }
    }
    fn unknown() -> Self {
        Self {
            low: f64::NEG_INFINITY,
            high: f64::INFINITY,
        }
    }
    fn union(self, other: Self) -> Self {
        Self {
            low: self.low.min(other.low),
            high: self.high.max(other.high),
        }
    }
    fn map(self, f: impl Fn(f64) -> f64) -> Self {
        Self {
            low: f(self.low),
            high: f(self.high),
        }
    }
}

impl Sampler<'_> {
    pub fn bounds(&mut self, id: Id, min: [f64; 3], max: [f64; 3]) -> Bounds {
        let graph = self.density;
        match &graph.nodes[id] {
            Node::Constant(v) => Bounds::point(*v),
            Node::Y => Bounds {
                low: min[1],
                high: max[1],
            },
            Node::Gradient { .. } => {
                let a = self.eval(id, min);
                let b = self.eval(id, max);
                Bounds {
                    low: a.min(b),
                    high: a.max(b),
                }
            }
            Node::Interpolated(_) => {
                // A trilinear field attains extrema at box corners inside one interpolation cell.
                // Boxes crossing a cell boundary fall back to sampling, rather than guessing.
                if (0..3).any(|axis| {
                    (min[axis] / [4.0, 8.0, 4.0][axis]).floor()
                        != (max[axis] / [4.0, 8.0, 4.0][axis]).floor()
                }) {
                    return Bounds::unknown();
                }
                let mut range = Bounds {
                    low: f64::INFINITY,
                    high: f64::NEG_INFINITY,
                };
                for corner in 0..8 {
                    if (0..3).any(|axis| min[axis] == max[axis] && corner & (1 << axis) != 0) {
                        continue;
                    }
                    let p = std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            min[axis]
                        } else {
                            max[axis]
                        }
                    });
                    let value = self.eval(id, p);
                    range = range.union(Bounds::point(value));
                }
                range
            }
            Node::Binary(op, a, b) => {
                let a = self.bounds(*a, min, max);
                let b = self.bounds(*b, min, max);
                match op {
                    Binary::Add => Bounds {
                        low: a.low + b.low,
                        high: a.high + b.high,
                    },
                    Binary::Mul => {
                        let v = [
                            a.low * b.low,
                            a.low * b.high,
                            a.high * b.low,
                            a.high * b.high,
                        ];
                        if v.iter().any(|v| v.is_nan()) {
                            Bounds::unknown()
                        } else {
                            Bounds {
                                low: v.into_iter().fold(f64::INFINITY, f64::min),
                                high: v.into_iter().fold(f64::NEG_INFINITY, f64::max),
                            }
                        }
                    }
                    Binary::Min => Bounds {
                        low: a.low.min(b.low),
                        high: a.high.min(b.high),
                    },
                    Binary::Max => Bounds {
                        low: a.low.max(b.low),
                        high: a.high.max(b.high),
                    },
                }
            }
            Node::Clamp(input, low, high) => {
                self.bounds(*input, min, max).map(|v| v.clamp(*low, *high))
            }
            Node::Range {
                input,
                min: lo,
                max: hi,
                inside,
                outside,
            } => {
                let input = self.bounds(*input, min, max);
                if input.low >= *lo && input.high < *hi {
                    self.bounds(*inside, min, max)
                } else if input.high < *lo || input.low >= *hi {
                    self.bounds(*outside, min, max)
                } else {
                    let a = self.bounds(*inside, min, max);
                    a.union(self.bounds(*outside, min, max))
                }
            }
            Node::Unary(op, input) => {
                let value = self.bounds(*input, min, max);
                let absolute = Bounds {
                    low: if value.low <= 0.0 && value.high >= 0.0 {
                        0.0
                    } else {
                        value.low.abs().min(value.high.abs())
                    },
                    high: value.low.abs().max(value.high.abs()),
                };
                match op {
                    Unary::Abs => absolute,
                    Unary::Square => absolute.map(|v| v * v),
                    Unary::Cube => value.map(|v| v * v * v),
                    Unary::HalfNegative => value.map(|v| if v > 0.0 { v } else { v * 0.5 }),
                    Unary::QuarterNegative => value.map(|v| if v > 0.0 { v } else { v * 0.25 }),
                    Unary::Squeeze => value.map(|v| {
                        let c = v.clamp(-1.0, 1.0);
                        c / 2.0 - c * c * c / 24.0
                    }),
                }
            }
            _ => Bounds::unknown(),
        }
    }
}
