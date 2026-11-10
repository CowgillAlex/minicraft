use glam::Vec3;
use std::{collections::HashSet, sync::Arc};

pub const CHUNK: usize = 32;
pub const NX: usize = 20;
pub const NY: usize = 6;
pub const NZ: usize = 20;
pub const WIDTH: i32 = (NX * CHUNK) as i32;
pub const DEPTH: i32 = (NZ * CHUNK) as i32;
pub const HEIGHT: i32 = (NY * CHUNK) as i32;
pub const VOXEL: f32 = 1.0 / 16.0;
pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const ROAD: u8 = 4;
pub const PAINT: u8 = 5;
pub const WOOD: u8 = 6;
pub const LEAF: u8 = 7;
pub const ROOF: u8 = 8;
pub const LIGHT: u8 = 9;
pub const WATER: u8 = 10;
pub const FLOWER: u8 = 11;
pub const WALL: u8 = 12;
pub const SAND: u8 = 13;

pub const GLASS: u8 = 14;

pub fn opaque(material: u8) -> bool {
    material != AIR && material != GLASS
}

#[derive(Clone)]
pub struct World {
    chunks: Vec<Option<Arc<[u8]>>>,
    pub emitters: HashSet<[i32; 3]>,
    pub voxels: usize,
}
impl World {
    pub fn empty() -> Self {
        Self {
            chunks: vec![None; NX * NY * NZ],
            voxels: 0,
            emitters: HashSet::new(),
        }
    }
    fn address(x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
        if !(0..WIDTH).contains(&x) || !(0..HEIGHT).contains(&y) || !(0..DEPTH).contains(&z) {
            return None;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        Some((
            (x / CHUNK) + NX * (z / CHUNK) + NX * NZ * (y / CHUNK),
            (x % CHUNK) + CHUNK * (z % CHUNK) + CHUNK * CHUNK * (y % CHUNK),
        ))
    }
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        Self::address(x, y, z)
            .and_then(|(chunk, index)| self.chunks[chunk].as_ref().map(|data| data[index]))
            .unwrap_or(AIR)
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, material: u8) {
        if let Some((chunk, index)) = Self::address(x, y, z) {
            if material == AIR && self.chunks[chunk].is_none() {
                return;
            }
            let data = self.chunks[chunk].get_or_insert_with(|| Arc::from(vec![0; CHUNK.pow(3)]));
            let data = Arc::make_mut(data);
            let previous = data[index];
            self.voxels += usize::from(previous == AIR && material != AIR);
            self.voxels -= usize::from(previous != AIR && material == AIR);
            data[index] = material;
            if previous == LIGHT && material != LIGHT {
                self.emitters.remove(&[x, y, z]);
            }
            if material == LIGHT {
                self.emitters.insert([x, y, z]);
            }
        }
    }
    pub fn chunk_exists(&self, x: usize, y: usize, z: usize) -> bool {
        self.chunks[x + NX * z + NX * NZ * y].is_some()
    }
    pub fn meters(x: i32, y: i32, z: i32) -> Vec3 {
        Vec3::new((x - WIDTH / 2) as f32, y as f32, (z - DEPTH / 2) as f32) * VOXEL
    }
    pub fn generate() -> Self {
        let mut world = Self::empty();
        for z in 0..DEPTH {
            for x in 0..WIDTH {
                let h = terrain_height(x, z);
                let lake = lake_distance(x, z) < 1.0;
                let h = if lake {
                    (h - ((1.0 - lake_distance(x, z)) * 18.0) as i32).max(6)
                } else {
                    h
                };
                for y in 0..=h {
                    world.set(
                        x,
                        y,
                        z,
                        if y == h {
                            if lake {
                                SAND
                            } else {
                                GRASS
                            }
                        } else if y > h - 4 {
                            DIRT
                        } else {
                            STONE
                        },
                    );
                }
                if lake {
                    for y in h + 1..=20 {
                        world.set(x, y, z, WATER);
                    }
                }
                let road = road_center(x);
                if (z - road).abs() < 22 {
                    for y in 26..=40 {
                        world.set(x, y, z, AIR);
                    }
                    for y in h.min(24)..=24 {
                        world.set(x, y, z, DIRT);
                    }
                    let material = if (z - road).abs() >= 19 {
                        STONE
                    } else if (z - road).abs() <= 1 && x.rem_euclid(44) < 23 {
                        PAINT
                    } else {
                        ROAD
                    };
                    world.set(x, 25, z, material);
                }
                if !lake && (z - road).abs() > 35 && hash(x, z, 17).is_multiple_of(233) {
                    world.set(x, h + 1, z, LEAF);
                    world.set(x, h + 2, z, FLOWER);
                }
            }
        }
        for (x, z) in [(155, 205), (348, 445), (490, 230)] {
            world.cabin(x, z);
        }
        for (i, (x, z)) in [
            (57, 70),
            (128, 95),
            (230, 72),
            (340, 90),
            (425, 105),
            (566, 80),
            (600, 210),
            (58, 235),
            (265, 230),
            (54, 430),
            (152, 490),
            (220, 545),
            (320, 560),
            (445, 540),
            (560, 490),
            (595, 595),
            (85, 585),
        ]
        .iter()
        .enumerate()
        {
            world.tree(*x, *z, i as i32);
        }
        for x in (65..WIDTH - 40).step_by(90) {
            let z = road_center(x) + if x % 2 == 0 { 28 } else { -28 };
            for y in 25..74 {
                world.cuboid([x, y, z], [x + 2, y, z + 2], WOOD);
            }
            world.cuboid([x - 2, 70, z - 2], [x + 4, 75, z + 4], STONE);
            world.cuboid([x - 1, 70, z - 1], [x + 3, 74, z + 3], LIGHT);
        }
        // Footbridge over the pond, with narrow rails and individual planks.
        for x in 362..550 {
            let z = 165;
            for zz in z - 9..=z + 9 {
                world.set(x, 25, zz, if x % 7 == 0 { DIRT } else { WOOD });
            }
            for zz in [z - 10, z + 10] {
                world.set(x, 37, zz, WOOD);
                if x % 24 < 2 {
                    for y in 22..38 {
                        world.set(x, y, zz, WOOD);
                    }
                }
            }
        }
        world
    }
    fn cuboid(&mut self, min: [i32; 3], max: [i32; 3], material: u8) {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                for x in min[0]..=max[0] {
                    self.set(x, y, z, material);
                }
            }
        }
    }
    fn cabin(&mut self, cx: i32, cz: i32) {
        let base = terrain_height(cx, cz) + 2;
        self.cuboid([cx - 38, 0, cz - 34], [cx + 38, base, cz + 34], STONE);
        self.cuboid(
            [cx - 32, base + 1, cz - 28],
            [cx + 32, base + 43, cz + 28],
            AIR,
        );
        for y in base + 1..base + 43 {
            for x in cx - 32..=cx + 32 {
                for z in cz - 28..=cz + 28 {
                    if (x - cx).abs() >= 30 || (z - cz).abs() >= 26 {
                        let window = y > base + 16
                            && y < base + 32
                            && (((z - cz).abs() >= 26 && ((x - cx).abs() - 18).abs() < 7)
                                || ((x - cx).abs() >= 30 && (z - cz).abs() < 9));
                        let door = z >= cz + 26 && (x - cx).abs() < 6 && y < base + 30;
                        self.set(
                            x,
                            y,
                            z,
                            if window {
                                let edge = y == base + 17
                                    || y == base + 31
                                    || ((z - cz).abs() >= 26
                                        && ((x - cx).abs() == 12 || (x - cx).abs() == 24))
                                    || ((x - cx).abs() >= 30 && (z - cz).abs() == 8);
                                if edge {
                                    WOOD
                                } else {
                                    GLASS
                                }
                            } else if door || y % 9 == 0 {
                                WOOD
                            } else {
                                WALL
                            },
                        );
                    }
                }
            }
        }
        self.cuboid(
            [cx - 29, base + 1, cz - 25],
            [cx + 29, base + 1, cz + 25],
            WOOD,
        );
        self.cuboid(
            [cx - 4, base + 40, cz - 4],
            [cx + 4, base + 41, cz + 4],
            WOOD,
        );
        self.cuboid(
            [cx - 3, base + 39, cz - 3],
            [cx + 3, base + 39, cz + 3],
            LIGHT,
        );
        // A low wooden table gives the interior light visible surfaces to fall on.
        self.cuboid(
            [cx - 12, base + 12, cz - 6],
            [cx + 12, base + 13, cz + 6],
            WOOD,
        );
        for x in [cx - 10, cx + 10] {
            for z in [cz - 4, cz + 4] {
                self.cuboid([x, base + 2, z], [x + 1, base + 11, z + 1], WOOD);
            }
        }
        for x in cx - 37..=cx + 37 {
            let roof_y = base + 43 + (37 - (x - cx).abs()) * 3 / 4;
            for z in cz - 33..=cz + 33 {
                self.set(x, roof_y, z, ROOF);
                self.set(x, roof_y + 1, z, ROOF);
                if (z - cz).abs() == 27 && (x - cx).abs() < 31 {
                    for y in base + 43..roof_y {
                        self.set(x, y, z, WALL);
                    }
                }
            }
        }
        self.cuboid(
            [cx + 16, base + 51, cz - 13],
            [cx + 22, base + 83, cz - 7],
            STONE,
        );
        self.cuboid([cx - 18, base, cz + 35], [cx + 18, base + 1, cz + 48], WOOD);
        for z in cz + 49..road_center(cx) - 23 {
            for x in cx - 8..=cx + 8 {
                let h = terrain_height(x, z);
                self.set(x, h + 1, z, SAND);
            }
        }
    }
    fn tree(&mut self, x: i32, z: i32, seed: i32) {
        let base = terrain_height(x, z);
        let height = 65 + (hash(x, z, seed) % 14) as i32;
        self.cuboid([x - 2, base, z - 2], [x + 2, base + height, z + 2], WOOD);
        for y in base + height - 28..=base + height + 27 {
            for dz in -28..=28 {
                for dx in -28..=28 {
                    let shape = (dx * dx + dz * dz) as f32 / 720.0
                        + ((y - base - height) as f32 / 28.0).powi(2);
                    let contour = 0.92
                        + (hash(
                            (x + dx).div_euclid(5),
                            y.div_euclid(5),
                            (z + dz).div_euclid(5),
                        ) % 100) as f32
                            * 0.0016;
                    if shape < contour {
                        self.set(x + dx, y, z + dz, LEAF);
                    }
                }
            }
        }
    }
}
pub fn hash(x: i32, y: i32, z: i32) -> u32 {
    let mut v = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ (z as u32).wrapping_mul(2246822519);
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    v ^ (v >> 16)
}
pub fn terrain_height(x: i32, z: i32) -> i32 {
    22 + ((x as f32 * 0.017).sin() * 4.0
        + (z as f32 * 0.022).cos() * 3.0
        + ((x + z) as f32 * 0.009).sin() * 5.0) as i32
}
fn road_center(x: i32) -> i32 {
    326 + (x as f32 * 0.013).sin().mul_add(22.0, 0.0) as i32
}
fn lake_distance(x: i32, z: i32) -> f32 {
    ((x - 460) as f32 / 105.0).powi(2) + ((z - 155) as f32 / 69.0).powi(2)
}
pub fn color(material: u8) -> [f32; 3] {
    match material {
        GRASS => [0.27, 0.46, 0.26],
        DIRT => [0.32, 0.23, 0.17],
        STONE => [0.43, 0.47, 0.46],
        ROAD => [0.16, 0.20, 0.22],
        PAINT => [0.93, 0.72, 0.35],
        WOOD => [0.34, 0.22, 0.14],
        LEAF => [0.22, 0.43, 0.28],
        ROOF => [0.32, 0.39, 0.43],
        LIGHT => [1.0, 0.69, 0.28],
        WATER => [0.15, 0.43, 0.47],
        FLOWER => [0.86, 0.58, 0.39],
        WALL => [0.77, 0.68, 0.50],
        SAND => [0.59, 0.53, 0.35],
        GLASS => [0.55, 0.77, 0.82],
        _ => [1.0, 0.0, 1.0],
    }
}

#[cfg(test)]
mod lighting_world_tests {
    use super::*;
    #[test]
    fn generated_houses_have_glass_and_interior_lamps() {
        let mut world = World::empty();
        world.cabin(348, 445);
        let base = terrain_height(348, 445) + 2;
        assert_eq!(world.get(348 - 18, base + 24, 445 + 28), GLASS);
        assert_eq!(world.get(348, base + 39, 445), LIGHT);
        assert!(world.emitters.contains(&[348, base + 39, 445]));
        assert_eq!(world.get(348, base + 2, 445), AIR);
    }
}
