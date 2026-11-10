use super::random::Random;

const GRADIENTS: [[f64; 3]; 16] = [
    [1., 1., 0.],
    [-1., 1., 0.],
    [1., -1., 0.],
    [-1., -1., 0.],
    [1., 0., 1.],
    [-1., 0., 1.],
    [1., 0., -1.],
    [-1., 0., -1.],
    [0., 1., 1.],
    [0., -1., 1.],
    [0., 1., -1.],
    [0., -1., -1.],
    [1., 1., 0.],
    [0., -1., 1.],
    [-1., 1., 0.],
    [0., -1., -1.],
];

pub fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

pub fn wrap(value: f64) -> f64 {
    value - (value / 33554432.0 + 0.5).floor() * 33554432.0
}

pub struct Improved {
    permutation: [u8; 256],
    offset: [f64; 3],
}

impl Improved {
    fn new(random: &mut Random) -> Self {
        let offset = std::array::from_fn(|_| random.double() * 256.0);
        let mut permutation = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = random.int((256 - i) as u32);
            permutation.swap(i, i + j);
        }
        Self {
            permutation,
            offset,
        }
    }

    fn p(&self, i: i32) -> i32 {
        self.permutation[(i & 255) as usize] as i32
    }

    pub fn sample(&self, point: [f64; 3], y_scale: f64, y_limit: f64) -> f64 {
        let shifted: [f64; 3] = std::array::from_fn(|i| point[i] + self.offset[i]);
        let cell = shifted.map(|v| v.floor() as i32);
        let fraction: [f64; 3] = std::array::from_fn(|i| shifted[i] - f64::from(cell[i]));
        let [x, y, z] = fraction;
        let y_shift = if y_scale != 0.0 {
            let t = if y_limit >= 0.0 && y_limit < y {
                y_limit
            } else {
                y
            };
            (t / y_scale + 1e-7).floor() * y_scale
        } else {
            0.0
        };
        let adjusted_y = y - y_shift;
        let gradient = |dx: i32, dy: i32, dz: i32| {
            let index = self.p(self.p(self.p(cell[0] + dx) + cell[1] + dy) + cell[2] + dz);
            let g = GRADIENTS[(index & 15) as usize];
            g[0] * (x - f64::from(dx))
                + g[1] * (adjusted_y - f64::from(dy))
                + g[2] * (z - f64::from(dz))
        };
        let fade = |v: f64| v * v * v * (v * (v * 6.0 - 15.0) + 10.0);
        let [tx, ty, tz] = fraction.map(fade);
        let plane = |dz| {
            lerp(
                ty,
                lerp(tx, gradient(0, 0, dz), gradient(1, 0, dz)),
                lerp(tx, gradient(0, 1, dz), gradient(1, 1, dz)),
            )
        };
        lerp(tz, plane(0), plane(1))
    }
}

pub struct Perlin {
    levels: Vec<Option<Improved>>,
    amplitudes: Vec<f64>,
    input_factor: f64,
    value_factor: f64,
}

impl Perlin {
    pub fn new(random: &mut Random, first_octave: i32, amplitudes: &[f64], legacy: bool) -> Self {
        let mut levels: Vec<_> = amplitudes.iter().map(|_| None).collect();
        if legacy {
            for i in (0..=(-first_octave) as usize).rev() {
                if i < amplitudes.len() && amplitudes[i] != 0.0 {
                    levels[i] = Some(Improved::new(random));
                } else {
                    for _ in 0..262 {
                        random.next();
                    }
                }
            }
        } else {
            let forked = random.fork();
            for (i, amplitude) in amplitudes.iter().enumerate() {
                if *amplitude != 0.0 {
                    let mut source = forked.named(&format!("octave_{}", first_octave + i as i32));
                    levels[i] = Some(Improved::new(&mut source));
                }
            }
        }
        Self {
            levels,
            amplitudes: amplitudes.to_vec(),
            input_factor: 2_f64.powi(first_octave),
            value_factor: 2_f64.powi(amplitudes.len() as i32 - 1)
                / (2_f64.powi(amplitudes.len() as i32) - 1.0),
        }
    }

    pub fn sample(&self, point: [f64; 3]) -> f64 {
        let mut value = 0.0;
        let mut input = self.input_factor;
        let mut weight = self.value_factor;
        for (level, amplitude) in self.levels.iter().zip(&self.amplitudes) {
            if let Some(noise) = level {
                value +=
                    amplitude * weight * noise.sample(point.map(|v| wrap(v * input)), 0.0, 0.0);
            }
            input *= 2.0;
            weight /= 2.0;
        }
        value
    }

    fn octave(&self, i: usize) -> Option<&Improved> {
        self.levels[self.levels.len() - 1 - i].as_ref()
    }
}

pub struct Normal {
    first: Perlin,
    second: Perlin,
    factor: f64,
}

impl Normal {
    pub fn new(random: &mut Random, octave: i32, amplitudes: &[f64]) -> Self {
        let first = Perlin::new(random, octave, amplitudes, false);
        let second = Perlin::new(random, octave, amplitudes, false);
        let min = amplitudes.iter().position(|a| *a != 0.0).unwrap();
        let max = amplitudes.iter().rposition(|a| *a != 0.0).unwrap();
        Self {
            first,
            second,
            factor: (1.0 / 6.0) / (0.1 * (1.0 + 1.0 / (max - min + 1) as f64)),
        }
    }

    pub fn sample(&self, point: [f64; 3]) -> f64 {
        (self.first.sample(point) + self.second.sample(point.map(|v| v * 1.0181268882175227)))
            * self.factor
    }
}

pub struct Blended {
    min: Perlin,
    max: Perlin,
    main: Perlin,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear: f64,
}

impl Blended {
    pub fn new(
        seed: i64,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear: f64,
    ) -> Self {
        let mut random = Random::new(seed).named("minecraft:blended_noise");
        Self {
            min: Perlin::new(&mut random, -15, &[1.0; 16], true),
            max: Perlin::new(&mut random, -15, &[1.0; 16], true),
            main: Perlin::new(&mut random, -7, &[1.0; 8], true),
            xz_multiplier: 684.412 * xz_scale,
            y_multiplier: 684.412 * y_scale,
            xz_factor,
            y_factor,
            smear,
        }
    }

    pub fn sample(&self, [x, y, z]: [f64; 3]) -> f64 {
        let limit = [
            x * self.xz_multiplier,
            y * self.y_multiplier,
            z * self.xz_multiplier,
        ];
        let main = [
            limit[0] / self.xz_factor,
            limit[1] / self.y_factor,
            limit[2] / self.xz_factor,
        ];
        let limit_smear = self.y_multiplier * self.smear;
        let mut value = 0.0;
        let mut pow = 1.0;
        for i in 0..8 {
            if let Some(noise) = self.main.octave(i) {
                value += noise.sample(
                    main.map(|v| wrap(v * pow)),
                    limit_smear / self.y_factor * pow,
                    main[1] * pow,
                ) / pow;
            }
            pow /= 2.0;
        }
        let factor = (value / 10.0 + 1.0) / 2.0;
        let mut min = 0.0;
        let mut max = 0.0;
        pow = 1.0;
        for i in 0..16 {
            let point = limit.map(|v| wrap(v * pow));
            if factor < 1.0 {
                if let Some(noise) = self.min.octave(i) {
                    min += noise.sample(point, limit_smear * pow, limit[1] * pow) / pow;
                }
            }
            if factor > 0.0 {
                if let Some(noise) = self.max.octave(i) {
                    max += noise.sample(point, limit_smear * pow, limit[1] * pow) / pow;
                }
            }
            pow /= 2.0;
        }
        lerp(factor.clamp(0.0, 1.0), min / 512.0, max / 512.0) / 128.0
    }
}
