//! Controlled reference plots use the exact production recipes. They allow
//! inspecting every biome treatment without searching a procedural world.
use super::*;
use crate::world::World;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug)]
pub enum Showcase {
    Biome(u8),
    Plants,
    Ores,
    Cabin,
    Tree,
}
impl Showcase {
    pub fn parse(name: &str) -> Result<Self, String> {
        Ok(match name {
            "plants" => Self::Plants,
            "ores" => Self::Ores,
            "cabin" => Self::Cabin,
            "tree" => Self::Tree,
            _ => Self::Biome(crate::worldgen::biomes::Table::shared()?.id(name)?),
        })
    }
    pub fn build(self) -> Result<World, String> {
        let mut world = World::with_height(256);
        let table = crate::worldgen::biomes::Table::shared()?;
        let kind = match self {
            Self::Biome(id) => ecology::habitat(table.name(id)),
            _ => Habitat::Meadow,
        };
        let biome_name = if let Self::Biome(id) = self {
            table.name(id)
        } else {
            "plains"
        };
        let underwater = kind.aquatic();
        let cave = kind.cave();
        let variation = if let Self::Biome(id) = self {
            i32::from(id)
        } else {
            kind as i32
        };
        let mut rng = Rng::new(7647, "showcase", [variation, 0, 0]);
        let soil = if matches!(
            biome_name,
            "stony_peaks" | "stony_shore" | "windswept_gravelly_hills" | "windswept_hills"
        ) {
            STONE
        } else {
            match kind {
                Habitat::Desert | Habitat::Badlands | Habitat::Reef => SAND,
                Habitat::Snow | Habitat::Alpine | Habitat::FrozenRiver => SNOW,
                Habitat::Mushroom => MYCELIUM,
                _ if underwater => GRAVEL,
                _ if cave => STONE,
                _ => GRASS,
            }
        };
        for z in 48..592 {
            for x in 48..592 {
                let h = 16;
                for y in 0..=h {
                    world.set(
                        x,
                        y,
                        z,
                        if y == h {
                            soil
                        } else if y > h - 4 {
                            DIRT
                        } else {
                            STONE
                        },
                    );
                }
                if underwater {
                    for y in h + 1..80 {
                        world.set(x, y, z, WATER);
                    }
                }
            }
        }
        let mut shapes = Vec::new();
        if cave {
            shapes.push(Primitive::cuboid(
                V::new(6., 5., 7.),
                V::new(34., 5.5, 27.),
                STONE,
                10,
            ));
            shapes.push(Primitive::cuboid(
                V::new(6., 1., 7.),
                V::new(34., 5.5, 7.6),
                STONE,
                10,
            ));
            shapes.push(Primitive::cuboid(
                V::new(6., 1., 7.),
                V::new(6.6, 5.5, 27.),
                STONE,
                10,
            ));
        }
        let base = V::new(20., 17. / 16., 20.);
        match self {
            Self::Tree => trees::tree(&mut shapes, base, Habitat::Forest, &mut rng),
            Self::Cabin => {
                structures::cabin(&mut shapes, base);
                for z in 0..4 {
                    for x in 0..5 {
                        plants::wheat(
                            &mut shapes,
                            base + V::new(3. + x as f64 * 0.55, 0., z as f64 * 0.5),
                            &mut rng,
                        );
                    }
                }
            }
            Self::Plants => {
                plants::grass(&mut shapes, base + V::new(-1.4, 0., 0.), &mut rng, false);
                plants::wheat(&mut shapes, base + V::new(0.4, 0., 0.), &mut rng);
                for (i, m) in [PETAL_RED, PETAL_BLUE, PETAL_WHITE, PETAL_PINK]
                    .into_iter()
                    .enumerate()
                {
                    plants::flower(
                        &mut shapes,
                        base + V::new(-1.5 + i as f64 * 0.8, 0., -1.2),
                        &mut rng,
                        m,
                    );
                }
                plants::fern(&mut shapes, base + V::new(1.9, 0., -0.8), &mut rng);
            }
            Self::Ores => {
                for (i, m) in [COAL, IRON, COPPER, GOLD, REDSTONE, LAPIS, DIAMOND, EMERALD]
                    .into_iter()
                    .enumerate()
                {
                    let at =
                        base + V::new((i % 4) as f64 * 1.8 - 2.7, 0., (i / 4) as f64 * 2.0 - 0.8);
                    shapes.push(Primitive::cuboid(
                        at,
                        at + V::new(1.35, 1.3, 1.1),
                        if i < 4 { STONE } else { DEEPSLATE },
                        80,
                    ));
                    for _ in 0..22 {
                        let c = at
                            + V::new(
                                0.18 + rng.unit() * 0.99,
                                0.18 + rng.unit() * 0.94,
                                0.18 + rng.unit() * 0.74,
                            );
                        shapes.push(Primitive::ellipsoid(
                            c,
                            V::splat(0.09 + rng.unit() * 0.07),
                            m,
                            Replace::Any,
                            90,
                        ));
                    }
                    // Cut the front off to expose the inclusions within the host rock.
                    shapes.push(Primitive::cuboid(
                        at + V::new(-0.02, -0.02, 0.77),
                        at + V::new(1.37, 1.32, 1.12),
                        AIR,
                        100,
                    ));
                }
            }
            Self::Biome(_) => {
                for z in 0..12 {
                    for x in 0..12 {
                        let p = V::new(
                            9. + x as f64 * 1.8 + rng.signed() * 0.3,
                            17. / 16.,
                            8. + z as f64 * 1.8 + rng.signed() * 0.3,
                        );
                        if underwater {
                            aquatic::grow(&mut shapes, p, 3.8, kind, &mut rng);
                        } else {
                            if biome_name == "ice_spikes" {
                                shapes.push(Primitive::branch(
                                    p,
                                    p + V::Y * (1.8 + rng.unit() * 3.),
                                    0.3,
                                    0.055,
                                    PACKED_ICE,
                                    Replace::Air,
                                    30,
                                ));
                            } else if biome_name == "flower_forest" {
                                let m =
                                    [PETAL_RED, PETAL_WHITE, PETAL_BLUE, PETAL_PINK][rng.index(4)];
                                plants::flower(&mut shapes, p, &mut rng, m);
                            } else {
                                plants::detail(&mut shapes, p, kind, &mut rng);
                            }
                        }
                    }
                }
                if kind.trees() > 0. {
                    for d in [
                        V::new(-4., 0., -3.),
                        V::new(4., 0., -5.),
                        V::new(1., 0., 4.),
                    ] {
                        if kind == Habitat::Bamboo {
                            plants::bamboo(&mut shapes, base + d, &mut rng);
                        } else {
                            trees::tree(&mut shapes, base + d, kind, &mut rng);
                        }
                    }
                }
                if matches!(kind, Habitat::Lush | Habitat::Dripstone) {
                    for i in 0..9 {
                        let p = base + V::new(i as f64 * 0.6 - 2., 5. - base.y, -3.);
                        shapes.push(Primitive::branch(
                            p,
                            p - V::Y * (0.8 + rng.unit() * 1.8),
                            0.12,
                            0.04,
                            if kind == Habitat::Lush {
                                FERN
                            } else {
                                SANDSTONE
                            },
                            Replace::Air,
                            30,
                        ));
                    }
                }
            }
        }
        paint(&mut world, &shapes);
        world.spawn = Some(crate::world::World::meters(320, 20, 530));
        Ok(world)
    }
}
pub fn paint(world: &mut World, shapes: &[Primitive]) {
    let mut pages: HashMap<[i32; 3], (Vec<u16>, Vec<u8>)> = HashMap::new();
    for primitive in shapes {
        let (lo, hi) = primitive.bounds();
        let lo = (lo * 16.)
            .floor()
            .as_ivec3()
            .to_array()
            .map(|v| v.div_euclid(32));
        let hi = (hi * 16.)
            .ceil()
            .as_ivec3()
            .to_array()
            .map(|v| v.div_euclid(32));
        for y in lo[1].max(0)..=hi[1].min(world.chunks_y() as i32 - 1) {
            for z in lo[2].max(0)..=hi[2].min(19) {
                for x in lo[0].max(0)..=hi[0].min(19) {
                    let (paint, priority) = pages
                        .entry([x, y, z])
                        .or_insert_with(|| (vec![0; 32768], vec![0; 32768]));
                    primitive.raster([x * 32, y * 32, z * 32], paint, priority);
                }
            }
        }
    }
    for (key, (paint, _)) in pages {
        let mut data = world.page(key).cloned().unwrap_or(Chunk::Uniform(AIR));
        for (i, p) in paint.into_iter().enumerate() {
            if p != 0 {
                let m = geometry::apply(p, data.get(i));
                data.set(i, m);
            }
        }
        data.compact();
        world.insert_page(key, data);
    }
}
