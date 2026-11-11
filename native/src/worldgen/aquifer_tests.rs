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
