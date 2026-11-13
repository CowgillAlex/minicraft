use super::*;
pub(super) fn cabin_candidate(key: [i32; 2]) -> bool {
    key[0].rem_euclid(8) == 2 && key[1].rem_euclid(8) == 3
}

pub(super) fn surface_area(sampler: &mut TerrainSampler<'_>, key: [i32; 2]) -> Vec<Primitive> {
    let mut rng = Rng::new(sampler.seed, "surface", [key[0], 0, key[1]]);
    let mut out = Vec::new();
    let centre = V::new(key[0] as f64 * 8. + 4., 0., key[1] as f64 * 8. + 4.);
    let ground = sampler.feature_ground(centre.x, centre.z);
    let (kind, _biome_name) = sampler.feature_habitat(ground.to_array());
    let wet = sampler.material_fast((ground + V::Y * 0.08).to_array()) == WATER;
    let house =
        !wet && matches!(
            kind,
            Habitat::Meadow
                | Habitat::Forest
                | Habitat::Birch
                | Habitat::Pine
                | Habitat::Snow
                | Habitat::Savanna
        ) && cabin_candidate(key)
            && rng.unit() < 0.4;
    let mut suitable = house;
    if house {
        for d in [V::X * 2.5, -V::X * 2.5, V::Z * 2.1, -V::Z * 2.1] {
            let p = sampler.feature_ground(centre.x + d.x, centre.z + d.z);
            suitable &= (p.y - ground.y).abs() < 0.65
                && sampler.material_fast((p + V::Y * 0.08).to_array()) == AIR;
        }
    }
    if suitable {
        structures::cabin(&mut out, ground);
        for z in 0..3 {
            for x in 0..4 {
                plants::wheat(
                    &mut out,
                    sampler.feature_ground(
                        ground.x + 3. + x as f64 * 0.45,
                        ground.z - 0.5 + z as f64 * 0.45,
                    ),
                    &mut rng,
                );
            }
        }
    } else if (!wet || (kind == Habitat::Wetland && ground.y > 61.))
        && rng.unit() < kind.trees()
        && (0.0..260.).contains(&ground.y)
    {
        if kind == Habitat::Bamboo {
            for dx in [-0.6, 0., 0.6] {
                plants::bamboo(&mut out, ground + V::X * dx, &mut rng);
            }
        } else {
            trees::tree(&mut out, ground, kind, &mut rng);
        }
    }
    for z in 0..6 {
        for x in 0..6 {
            let px = key[0] as f64 * 8. + 0.5 + x as f64 * 1.3 + rng.signed() * 0.22;
            let pz = key[1] as f64 * 8. + 0.5 + z as f64 * 1.3 + rng.signed() * 0.22;
            if suitable && (px - ground.x).abs() < 2.8 && (pz - ground.z).abs() < 2.5 {
                continue;
            }
            if rng.unit()
                > if matches!(
                    kind,
                    Habitat::Alpine | Habitat::Desert | Habitat::Badlands | Habitat::Snow
                ) {
                    0.2
                } else {
                    0.72
                }
            {
                continue;
            }
            let p = sampler.feature_ground(px, pz);
            let (h, local_name) = sampler.feature_habitat(p.to_array());
            let above = sampler.material_fast((p + V::Y * 0.08).to_array());
            if above == WATER {
                let depth = (63. - p.y).max(0.2);
                aquatic::grow(
                    &mut out,
                    p,
                    depth,
                    if h.aquatic() { h } else { Habitat::River },
                    &mut rng,
                );
            } else if above == AIR && p.y > -63. && !h.cave() {
                if local_name == "ice_spikes" {
                    out.push(Primitive::branch(
                        p,
                        p + V::Y * (1.8 + rng.unit() * 3.0),
                        0.3,
                        0.055,
                        PACKED_ICE,
                        Replace::Air,
                        30,
                    ));
                } else if local_name == "flower_forest" {
                    let color = [PETAL_WHITE, PETAL_BLUE, PETAL_PINK, PETAL_RED][rng.index(4)];
                    plants::flower(&mut out, p, &mut rng, color);
                } else {
                    plants::detail(&mut out, p, h, &mut rng);
                }
            }
        }
    }
    out
}
pub(super) fn cave_area(sampler: &mut TerrainSampler<'_>, key: [i32; 3]) -> Vec<Primitive> {
    let mut out = Vec::new();
    let mut rng = Rng::new(sampler.seed, "cave", key);
    for z in 0..3 {
        for x in 0..3 {
            let px = key[0] as f64 * 8. + 1. + x as f64 * 2.5;
            let pz = key[2] as f64 * 8. + 1. + z as f64 * 2.5;
            for y in 0..8 {
                let p = V::new(px, key[1] as f64 * 8. + y as f64, pz);
                if sampler.material_fast(p.to_array()) == STONE
                    && sampler.material_fast((p - V::Y).to_array()) == AIR
                {
                    let (kind, _) = sampler.feature_habitat((p - V::Y).to_array());
                    if matches!(kind, Habitat::Lush | Habitat::Dripstone) {
                        let mut base = p;
                        for _ in 0..16 {
                            if sampler.material_fast((base - V::Y * 0.0625).to_array()) == STONE {
                                base.y -= 0.0625;
                            } else {
                                break;
                            }
                        }
                        let length = 0.5 + rng.unit() * 2.;
                        let end = base - V::Y * length;
                        out.push(Primitive::branch(
                            base,
                            end,
                            if kind == Habitat::Lush { 0.047 } else { 0.16 },
                            0.035,
                            if kind == Habitat::Lush {
                                FERN
                            } else {
                                SANDSTONE
                            },
                            Replace::Air,
                            24,
                        ));
                        if kind == Habitat::Lush {
                            for j in 1..5 {
                                out.push(Primitive::ellipsoid(
                                    base.lerp(end, j as f64 / 5.),
                                    V::new(0.13, 0.065, 0.1),
                                    LEAF,
                                    Replace::Air,
                                    25,
                                ));
                            }
                        }
                    }
                }
                if sampler.material_fast(p.to_array()) != STONE
                    || sampler.material_fast((p + V::Y).to_array()) != AIR
                {
                    continue;
                }
                let (kind, _) = sampler.feature_habitat((p + V::Y).to_array());
                if !kind.cave() {
                    continue;
                }
                let mut base = p;
                for _ in 0..16 {
                    if sampler.material_fast((base + V::Y * 0.0625).to_array()) == STONE {
                        base.y += 0.0625;
                    } else {
                        break;
                    }
                }
                base.y += 0.0625;
                plants::detail(&mut out, base, kind, &mut rng);
                if kind == Habitat::Lush {
                    out.push(Primitive::ellipsoid(
                        base + V::Y * 0.025,
                        V::new(0.7, 0.04, 0.6),
                        MOSS,
                        Replace::Air,
                        20,
                    ));
                }
            }
        }
    }
    out
}
