use super::*;
impl World {
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
    pub(super) fn cabin(&mut self, cx: i32, cz: i32) {
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
                        let door = z >= cz + 26 && x >= cx - 8 && x < cx + 8 && y < base + 34;
                        self.set(
                            x,
                            y,
                            z,
                            if door {
                                AIR
                            } else if window {
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
                            } else if y % 9 == 0 {
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
        for x in [cx - 9, cx + 8] {
            self.cuboid([x, base + 2, cz + 26], [x, base + 34, cz + 28], WOOD);
        }
        self.cuboid(
            [cx - 9, base + 34, cz + 26],
            [cx + 8, base + 35, cz + 28],
            WOOD,
        );
        self.install_door(crate::doors::Door::prefab([cx - 8, base + 2, cz + 26]));
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
