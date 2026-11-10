use super::density::{Binary, Density, Id, Node, Sampler, Unary};

/// Compile the bundled final-density expression into its five interpolation inputs.
/// Every constant is read from the graph. Other graph shapes use the generic evaluator.
#[derive(Clone, Copy)]
pub struct Plan {
    inputs: [Id; 5],
    terrain_scale: f64,
    noodle_scale: f64,
    gate_min: f64,
    gate_max: f64,
    gate_value: f64,
}

pub struct Prepared {
    plan: Plan,
    values: [[f64; 8]; 5],
    base: [f64; 3],
}

fn multiply(density: &Density, id: Id) -> Option<(f64, Id)> {
    let Node::Binary(Binary::Mul, a, b) = density.nodes[id] else {
        return None;
    };
    if let Node::Constant(value) = density.nodes[a] {
        Some((value, b))
    } else if let Node::Constant(value) = density.nodes[b] {
        Some((value, a))
    } else {
        None
    }
}
fn absolute(density: &Density, id: Id) -> Option<Id> {
    if let Node::Unary(Unary::Abs, input) = density.nodes[id] {
        Some(input)
    } else {
        None
    }
}

impl Plan {
    pub fn compile(density: &Density) -> Option<Self> {
        let Node::Binary(Binary::Min, terrain, noodle) = density.nodes[density.roots[0]] else {
            return None;
        };
        let Node::Unary(Unary::Squeeze, terrain) = density.nodes[terrain] else {
            return None;
        };
        let (terrain_scale, terrain) = multiply(density, terrain)?;
        let Node::Range {
            input,
            min,
            max,
            inside,
            outside,
        } = density.nodes[noodle]
        else {
            return None;
        };
        let Node::Constant(gate_value) = density.nodes[inside] else {
            return None;
        };
        let Node::Binary(Binary::Add, thickness, ridges) = density.nodes[outside] else {
            return None;
        };
        let (noodle_scale, ridges) = multiply(density, ridges)?;
        let Node::Binary(Binary::Max, a, b) = density.nodes[ridges] else {
            return None;
        };
        let inputs = [
            terrain,
            input,
            thickness,
            absolute(density, a)?,
            absolute(density, b)?,
        ];
        if !inputs
            .iter()
            .all(|id| matches!(density.nodes[*id], Node::Interpolated(_)))
        {
            return None;
        }
        Some(Self {
            inputs,
            terrain_scale,
            noodle_scale,
            gate_min: min,
            gate_max: max,
            gate_value,
        })
    }

    pub fn prepare(
        self,
        sampler: &mut Sampler<'_>,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Option<Prepared> {
        let sizes = [4.0, 8.0, 4.0];
        if (0..3).any(|i| (min[i] / sizes[i]).floor() != (max[i] / sizes[i]).floor()) {
            return None;
        }
        let mut values = [[0.0; 8]; 5];
        let mut base = [0.0; 3];
        for (i, id) in self.inputs.into_iter().enumerate() {
            let (key, corners) = sampler.interpolation_cell(id, min)?;
            values[i] = corners;
            base = std::array::from_fn(|axis| f64::from(key[axis]) * sizes[axis]);
        }
        Some(Prepared {
            plan: self,
            values,
            base,
        })
    }
}

impl Prepared {
    #[inline]
    pub fn sample(&self, point: [f64; 3]) -> f64 {
        let t = [
            (point[0] - self.base[0]) * 0.25,
            (point[1] - self.base[1]) * 0.125,
            (point[2] - self.base[2]) * 0.25,
        ];
        let interpolate = |index: usize| {
            let v = &self.values[index];
            let a = (v[0] * (1.0 - t[0]) + v[1] * t[0]) * (1.0 - t[1])
                + (v[2] * (1.0 - t[0]) + v[3] * t[0]) * t[1];
            if t[2] == 0.0 {
                a
            } else {
                let b = (v[4] * (1.0 - t[0]) + v[5] * t[0]) * (1.0 - t[1])
                    + (v[6] * (1.0 - t[0]) + v[7] * t[0]) * t[1];
                a * (1.0 - t[2]) + b * t[2]
            }
        };
        let c = (self.plan.terrain_scale * interpolate(0)).clamp(-1.0, 1.0);
        let terrain = c / 2.0 - c * c * c / 24.0;
        let gate = interpolate(1);
        let noodle = if gate >= self.plan.gate_min && gate < self.plan.gate_max {
            self.plan.gate_value
        } else {
            interpolate(2) + self.plan.noodle_scale * interpolate(3).abs().max(interpolate(4).abs())
        };
        terrain.min(noodle)
    }
}
