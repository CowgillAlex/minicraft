use super::*;
use crate::worldgen::density::{Density, Node};

fn field(surface_low: bool, floodedness: f64) -> Density {
    Density {
        nodes: vec![
            Node::Constant(-0.1),
            if surface_low {
                Node::Gradient {
                    from_y: 0.0,
                    to_y: 8.0,
                    from: 1.0,
                    to: 0.0,
                }
            } else {
                Node::Constant(1.0)
            },
            Node::Constant(0.0),
            Node::Constant(floodedness),
            Node::Constant(0.0),
        ],
        noises: Vec::new(),
        blended: Vec::new(),
        roots: [0, 1, 2, 3, 4],
        climate: [0; 6],
        min_y: -64,
        height: 384,
        sea_level: 63,
    }
}

#[test]
fn fluid_cells_use_all_axes_and_are_independent_of_cache_order() {
    let mut a = Aquifer::new(12345);
    let mut b = Aquifer::new(12345);
    let keys = [[-2, -3, -4], [0, 0, 0], [0, 0, 1], [12, 3, -7]];
    for key in keys.into_iter().rev() {
        b.location(key);
    }
    for key in keys {
        let location = a.location(key);
        assert_eq!(location, b.location(key));
        for (axis, spacing) in [16, 12, 16].into_iter().enumerate() {
            let offset = location[axis] - key[axis] * spacing;
            assert!((0..if axis == 1 { 9 } else { 10 }).contains(&offset));
        }
    }
    assert_ne!(a.location([0, 0, 0])[..2], a.location([0, 0, 1])[..2]);
}

#[test]
fn crossing_z_cell_boundaries_matches_fresh_aquifer_samples() {
    let graph = Density::new(1).unwrap();
    let points =
        [-32.0625, -16.0625, -0.0625, 5.0625, 16.0625, 32.0625].map(|z| [120.0625, 24.0625, z]);
    let expected =
        points.map(|point| Aquifer::new(1).substance(&mut Sampler::new(&graph), point, -0.01));
    let mut cached = Aquifer::new(1);
    let mut sampler = Sampler::new(&graph);
    for index in (0..points.len()).chain((0..points.len()).rev()) {
        assert_eq!(
            cached.substance(&mut sampler, points[index], -0.01),
            expected[index]
        );
    }
}

#[test]
fn ocean_fills_to_sea_level_while_underground_regions_can_stay_dry() {
    let sea = field(true, -1.0);
    let dry = field(false, -1.0);
    let wet = field(false, 1.0);
    for (graph, expected) in [(&sea, WATER), (&dry, AIR), (&wet, WATER)] {
        let mut sampler = Sampler::new(graph);
        let mut aquifer = Aquifer::new(1);
        for x in [-16.0625, -0.0625, 0.0, 15.9375, 16.0625] {
            for z in [-16.0625, -0.0625, 0.0, 15.9375, 16.0625] {
                assert_eq!(
                    aquifer.substance(&mut sampler, [x, 40.0, z], -0.1),
                    expected
                );
                assert_eq!(aquifer.substance(&mut sampler, [x, 63.0, z], -0.1), AIR);
                assert_eq!(aquifer.substance(&mut sampler, [x, 40.0, z], 0.1), STONE);
            }
        }
    }
    let mut sampler = Sampler::new(&wet);
    assert_eq!(
        Aquifer::new(1).substance(&mut sampler, [0.0, -60.0, 0.0], -0.1),
        AIR
    );
}

#[test]
fn incompatible_fluid_levels_form_stone_barriers_instead_of_exposed_water_walls() {
    let graph = field(false, 0.0);
    let mut sampler = Sampler::new(&graph);
    let mut aquifer = Aquifer::new(1);
    let p = [0.0, 30.0, 0.0];
    aquifer.prepare_neighbors([-1, 2, -1]);
    for (i, candidate) in aquifer.neighbors.iter_mut().enumerate() {
        candidate.location = if i == 0 {
            [0, 30, 0]
        } else if i == 1 {
            [0, 30, 1]
        } else {
            [100 + i as i32, 100, 100]
        };
        candidate.fluid = Some(Fluid {
            level: if i == 1 { 20 } else { 40 },
            material: WATER,
        });
    }
    assert_eq!(aquifer.substance(&mut sampler, p, -0.1), STONE);
    aquifer.neighbors[1].fluid = Some(Fluid {
        level: 40,
        material: WATER,
    });
    assert_eq!(aquifer.substance(&mut sampler, p, -0.1), WATER);
}

#[test]
fn negative_pressure_cannot_puncture_a_one_voxel_wet_dry_seal() {
    let mut graph = field(false, 0.0);
    graph.nodes[2] = Node::Constant(-1.0);
    let mut sampler = Sampler::new(&graph);
    for axis in 0..3 {
        let p = [0.0, 62.9375, 0.0];
        let anchor = [-1, 5, -1];
        let mut aquifer = Aquifer::new(1);
        aquifer.prepare_neighbors(anchor);
        for (i, c) in aquifer.neighbors.iter_mut().enumerate() {
            c.location = if i < 2 {
                [0, 63, 0]
            } else {
                [100 + i as i32, 100, 100]
            };
            if i < 2 {
                c.location[axis] += if i == 0 { -1 } else { 1 };
            }
            c.fluid = Some(Fluid {
                level: if i == 0 { 63 } else { -128 },
                material: if i == 0 { WATER } else { AIR },
            });
        }
        // Place the query on the bisector even for the vertical case.
        let mut p = p;
        if axis == 1 {
            p[1] = 63.0;
        }
        // Both sides must differ at the query height; the real free surface remains air.
        aquifer.neighbors[0].fluid.as_mut().unwrap().level = 64;
        for (offset, expected) in [(-0.0625, WATER), (0.0, STONE), (0.0625, AIR)] {
            let mut q = p;
            q[axis] += offset;
            assert_eq!(
                aquifer.substance(&mut sampler, q, -0.45),
                expected,
                "axis {axis} at {q:?}"
            );
        }
        for c in &mut aquifer.neighbors[..2] {
            c.fluid = Some(Fluid {
                level: 64,
                material: WATER,
            });
        }
        assert_eq!(aquifer.substance(&mut sampler, p, -0.45), WATER);
    }
}

#[test]
fn reported_cave_boundaries_are_sealed_and_spurious_surface_water_is_dry() {
    let generator = crate::worldgen::Generator::new(1794358123416304200).unwrap();
    let mut sampler = generator.sampler();
    for (p, expected) in [
        ([268.0, 22.0, -144.0], STONE),
        ([268.0625, 22.0, -144.0], STONE),
        ([272.0, 25.0, -140.0], STONE),
        ([271.9375, 25.0, -140.0], STONE),
        ([314.0, 62.0, 25.875], AIR),
        ([314.0, 62.0, 25.9375], AIR),
        ([314.6875, 62.875, 25.0625], AIR),
    ] {
        assert_eq!(sampler.material(p), expected, "{p:?}");
    }
}

#[test]
fn preliminary_heights_match_the_literal_source_scan() {
    for seed in [1, 12345, 1794358123416304200] {
        let graph = Density::new(seed).unwrap();
        let mut sampler = Sampler::new(&graph);
        let mut aquifer = Aquifer::new(seed);
        for [x, z] in [[-17, -5], [0, 0], [3, 1], [304, 19], [272, 3]] {
            let expected = (graph.min_y..graph.min_y + graph.height)
                .rev()
                .find(|&y| sampler.root(1, [f64::from(x), f64::from(y), f64::from(z)]) > 0.0)
                .unwrap_or(graph.min_y);
            assert_eq!(
                aquifer.surface(&mut sampler, x, z),
                expected,
                "seed {seed}, [{x}, {z}]"
            );
        }
    }
}

#[test]
fn preliminary_scan_keeps_thin_upper_layers_and_excludes_the_world_top() {
    let mut graph = field(false, 0.0);
    graph.min_y = 0;
    graph.height = 32;
    graph
        .nodes
        .extend([Node::Y, Node::Constant(1.0), Node::Constant(-1.0)]);
    graph.nodes.push(Node::Range {
        input: 5,
        min: 7.0,
        max: 8.0,
        inside: 6,
        outside: 7,
    });
    graph.nodes[1] = Node::Range {
        input: 5,
        min: 25.0,
        max: 26.0,
        inside: 6,
        outside: 8,
    };
    assert_eq!(
        Aquifer::new(1).surface(&mut Sampler::new(&graph), 3, -5),
        25
    );
    graph.nodes[1] = Node::Range {
        input: 5,
        min: 32.0,
        max: 33.0,
        inside: 6,
        outside: 8,
    };
    assert_eq!(Aquifer::new(1).surface(&mut Sampler::new(&graph), 3, -5), 7);
    graph.nodes[1] = Node::Constant(0.0);
    assert_eq!(Aquifer::new(1).surface(&mut Sampler::new(&graph), 3, -5), 0);
    // Interpolation is deliberately unsupported by the column preparation.
    // Its literal scan must still find the highest positive lattice sample.
    graph.nodes[1] = Node::Interpolated(8);
    let mut sampler = Sampler::new(&graph);
    let expected = (0..32)
        .rev()
        .find(|&y| sampler.root(1, [3.0, f64::from(y), -5.0]) > 0.0)
        .unwrap_or(0);
    assert_eq!(Aquifer::new(1).surface(&mut sampler, 3, -5), expected);
}
