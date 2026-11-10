use super::{density::Sampler, random::Random};
use crate::world::{AIR, STONE, WATER};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fluid {
    level: i32,
    material: u8,
}
impl Fluid {
    fn at(self, y: f64) -> u8 {
        if y < f64::from(self.level) {
            self.material
        } else {
            AIR
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Candidate {
    key: [i32; 3],
    location: [i32; 3],
    fluid: Option<Fluid>,
}

/// Seeded 3D fluid cells, with pressure barriers between incompatible water levels.
/// Keeps Paper-Minecraft's fluid rules and empty lava band; terrain density is unchanged.
pub struct Aquifer {
    random: Random,
    locations: HashMap<[i32; 3], [i32; 3]>,
    statuses: HashMap<[i32; 3], Fluid>,
    surfaces: HashMap<[i32; 2], i32>,
    anchor: Option<[i32; 3]>,
    neighbors: [Candidate; 12],
}

impl Aquifer {
    pub fn new(seed: i64) -> Self {
        Self {
            random: Random::new(seed).named("minecraft:aquifer"),
            locations: HashMap::new(),
            statuses: HashMap::new(),
            surfaces: HashMap::new(),
            anchor: None,
            neighbors: [Candidate::default(); 12],
        }
    }

    pub fn substance(
        &mut self,
        sampler: &mut Sampler<'_>,
        [x, y, z]: [f64; 3],
        density: f64,
    ) -> u8 {
        if density > 0.0 {
            return STONE;
        }
        if y < f64::from((-54).min(sampler.density.sea_level)) {
            return AIR;
        }
        let anchor = [
            ((x - 5.0) / 16.0).floor() as i32,
            ((y + 1.0) / 12.0).floor() as i32,
            ((z - 5.0) / 16.0).floor() as i32,
        ];
        self.prepare_neighbors(anchor);
        let mut nearest = [0; 3];
        let mut distances = [f64::INFINITY; 3];
        for (candidate, entry) in self.neighbors.iter().enumerate() {
            let dx = f64::from(entry.location[0]) - x;
            let dy = f64::from(entry.location[1]) - y;
            let dz = f64::from(entry.location[2]) - z;
            let distance = dx * dx + dy * dy + dz * dz;
            if let Some(index) = distances.iter().position(|d| *d >= distance) {
                for i in ((index + 1)..3).rev() {
                    distances[i] = distances[i - 1];
                    nearest[i] = nearest[i - 1];
                }
                distances[index] = distance;
                nearest[index] = candidate;
            }
        }
        let a = self.status(sampler, nearest[0]);
        let similarity12 = similarity(distances[0], distances[1]);
        let fluid = a.at(y);
        if similarity12 <= 0.0 {
            return fluid;
        }
        let b = self.status(sampler, nearest[1]);
        if density + similarity12 * pressure(sampler, [x, y, z], a, b) > 0.0 {
            return STONE;
        }
        let c = self.status(sampler, nearest[2]);
        let similarity13 = similarity(distances[0], distances[2]);
        if similarity13 > 0.0
            && density + similarity12 * similarity13 * pressure(sampler, [x, y, z], a, c) > 0.0
        {
            return STONE;
        }
        let similarity23 = similarity(distances[1], distances[2]);
        if similarity23 > 0.0
            && density + similarity12 * similarity23 * pressure(sampler, [x, y, z], b, c) > 0.0
        {
            return STONE;
        }
        fluid
    }

    fn location(&mut self, key: [i32; 3]) -> [i32; 3] {
        *self.locations.entry(key).or_insert_with(|| {
            let mut random = self
                .random
                .named(&format!("cell/{}/{}/{}", key[0], key[1], key[2]));
            [
                key[0] * 16 + random.int(10) as i32,
                key[1] * 12 + random.int(9) as i32,
                key[2] * 16 + random.int(10) as i32,
            ]
        })
    }

    // Cache only the current 12 candidates locally; repeated fine samples use no map lookups.
    fn prepare_neighbors(&mut self, anchor: [i32; 3]) {
        if self.anchor == Some(anchor) {
            return;
        }
        self.neighbors = std::array::from_fn(|i| {
            let key = [
                anchor[0] + (i / 6) as i32,
                anchor[1] + ((i / 2) % 3) as i32 - 1,
                anchor[2] + (i % 2) as i32,
            ];
            Candidate {
                key,
                location: self.location(key),
                fluid: self.statuses.get(&key).copied(),
            }
        });
        self.anchor = Some(anchor);
    }

    fn status(&mut self, sampler: &mut Sampler<'_>, index: usize) -> Fluid {
        let candidate = self.neighbors[index];
        if let Some(status) = candidate.fluid {
            return status;
        }
        let [x, y, z] = candidate.location;
        let status = self.compute_fluid(sampler, x, y, z);
        self.statuses.insert(candidate.key, status);
        self.neighbors[index].fluid = Some(status);
        status
    }

    fn compute_fluid(&mut self, sampler: &mut Sampler<'_>, x: i32, y: i32, z: i32) -> Fluid {
        let sea = sampler.density.sea_level;
        let global = global_fluid(y, sea);
        let mut lowest_surface = i32::MAX;
        let mut center_under_water = false;
        for [dx, dz] in SURFACE_SAMPLES {
            let surface = self.surface(sampler, x + dx * 16, z + dz * 16);
            let adjusted = surface + 8;
            let center = dx == 0 && dz == 0;
            if center && y - 12 > adjusted {
                return global;
            }
            let above = y + 12 > adjusted;
            if above || center {
                let surface_fluid = global_fluid(adjusted, sea);
                if surface_fluid.at(f64::from(adjusted)) != AIR {
                    if center {
                        center_under_water = true;
                    }
                    if above {
                        return surface_fluid;
                    }
                }
            }
            lowest_surface = lowest_surface.min(surface);
        }
        let allowed = if center_under_water {
            map(
                f64::from(lowest_surface + 8 - y).clamp(0.0, 64.0),
                0.0,
                64.0,
                1.0,
                0.0,
            )
        } else {
            0.0
        };
        let flooded = sampler
            .root(3, [f64::from(x), f64::from(y), f64::from(z)])
            .clamp(-1.0, 1.0);
        if flooded > map(allowed, 1.0, 0.0, -0.3, 0.8) {
            return global;
        }
        if flooded <= map(allowed, 1.0, 0.0, -0.8, 0.4) {
            return Fluid {
                level: sampler.density.min_y * 2,
                material: AIR,
            };
        }
        let cell_y = y.div_euclid(40);
        let spread = sampler.root(
            4,
            [
                f64::from(x.div_euclid(16)),
                f64::from(cell_y),
                f64::from(z.div_euclid(16)),
            ],
        ) * 10.0;
        let target = cell_y * 40 + 20 + (spread / 3.0 + 0.5).floor() as i32 * 3;
        Fluid {
            level: lowest_surface.min(target),
            material: global.material,
        }
    }

    fn surface(&mut self, sampler: &mut Sampler<'_>, x: i32, z: i32) -> i32 {
        // Preliminary terrain height is evaluated on the reference 4 m horizontal / 8 m vertical lattice.
        let key = [x.div_euclid(4) * 4, z.div_euclid(4) * 4];
        if let Some(y) = self.surfaces.get(&key) {
            return *y;
        }
        let min = sampler.density.min_y;
        let top = min + sampler.density.height;
        let mut surface = min;
        let mut y = top;
        while y >= min {
            if sampler.root(1, [f64::from(key[0]), f64::from(y), f64::from(key[1])]) > 0.390625 {
                surface = y;
                break;
            }
            y -= 8;
        }
        self.surfaces.insert(key, surface);
        surface
    }
}

fn global_fluid(y: i32, sea: i32) -> Fluid {
    if y < (-54).min(sea) {
        Fluid {
            level: -54,
            material: AIR,
        }
    } else {
        Fluid {
            level: sea,
            material: WATER,
        }
    }
}
fn similarity(a: f64, b: f64) -> f64 {
    1.0 - (b - a) / 25.0
}
fn map(v: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (v - from_min) / (from_max - from_min) * (to_max - to_min)
}
fn pressure(sampler: &mut Sampler<'_>, [x, y, z]: [f64; 3], a: Fluid, b: Fluid) -> f64 {
    let difference = (a.level - b.level).abs();
    if difference == 0 {
        return 0.0;
    }
    let average = 0.5 * f64::from(a.level + b.level);
    let distance = y + 0.5 - average;
    let edge = f64::from(difference) / 2.0 - distance.abs();
    let gradient = if distance > 0.0 {
        if edge > 0.0 {
            edge / 1.5
        } else {
            edge / 2.5
        }
    } else {
        let center = 3.0 + edge;
        if center > 0.0 {
            center / 3.0
        } else {
            center / 10.0
        }
    };
    let barrier = if (-2.0..=2.0).contains(&gradient) {
        sampler.root(2, [x, y, z])
    } else {
        0.0
    };
    2.0 * (barrier + gradient)
}

// The full X/Z surface neighborhood, rather than the collapsed five X columns.
const SURFACE_SAMPLES: [[i32; 2]; 13] = [
    [-2, -1],
    [-1, -1],
    [0, -1],
    [1, -1],
    [-3, 0],
    [-2, 0],
    [-1, 0],
    [0, 0],
    [1, 0],
    [-2, 1],
    [-1, 1],
    [0, 1],
    [1, 1],
];

#[cfg(test)]
#[path = "aquifer_tests.rs"]
mod tests;
