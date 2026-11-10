use super::*;
use crate::world::{self, World, VOXEL};
use glam::Vec3;

fn floor() -> World {
    let mut world = World::empty();
    for x in 240..400 {
        for z in 240..400 {
            world.set(x, 16, z, world::STONE);
        }
    }
    world
}
fn feet() -> Vec3 {
    World::meters(320, 17, 320) + Vec3::Y * 0.00002
}
fn input(direction: Vec3, speed: f32) -> Input {
    Input {
        direction,
        speed,
        ..Default::default()
    }
}
#[test]
fn clipping_stops_both_directions_and_allows_tangent_contact() {
    let obstacle = Aabb {
        min: Vec3::splat(2.0),
        max: Vec3::splat(3.0),
    };
    for axis in 0..3 {
        let mut body = obstacle;
        body.min[axis] = 0.0;
        body.max[axis] = 1.0;
        assert!((obstacle.clip(body, axis, 10.0) - (1.0 - aabb::EPSILON)).abs() < 0.00001);
        body.min[axis] = 4.0;
        body.max[axis] = 5.0;
        assert!((obstacle.clip(body, axis, -10.0) - (-1.0 + aabb::EPSILON)).abs() < 0.00001);
        let other = (axis + 1) % 3;
        body.max[other] = 2.0;
        body.min[other] = 1.0;
        assert_eq!(obstacle.clip(body, axis, -10.0), -10.0);
    }
}
#[test]
fn falls_lands_and_stays_stable_without_floor_penetration() {
    let world = floor();
    let mut player = Player::new(feet() + Vec3::Y * 7.0);
    for _ in 0..60 {
        player.tick(&world, Input::default());
    }
    assert!(player.grounded);
    assert!((player.position.y - 17.0 * VOXEL).abs() < 0.00003);
    assert!(Player::can_occupy(&world, player.position));
    let landed = player.position;
    for _ in 0..200 {
        player.tick(&world, Input::default());
    }
    assert!(player.position.abs_diff_eq(landed, 0.000001));
    assert_eq!(player.velocity.y, 0.0);
}
#[test]
fn fast_motion_cannot_tunnel_through_one_voxel_walls_and_slides() {
    let mut world = floor();
    for y in 17..80 {
        for z in 280..380 {
            world.set(336, y, z, world::GLASS);
        }
    }
    let mut player = Player::new(feet());
    player.mode = Mode::Fly;
    player.tick(
        &world,
        Input {
            sprint: true,
            ..input(Vec3::X, 40.0)
        },
    );
    assert!(player.position.x < 0.701 && player.position.x > 0.69);
    assert_eq!(player.velocity.x, 0.0);
    let before = player.position;
    player.tick(&world, input(Vec3::new(1.0, 0.0, 1.0).normalize(), 5.0));
    assert!((player.position.x - before.x).abs() < 0.00001);
    assert!(player.position.z > before.z + 0.1);
    assert!(Player::can_occupy(&world, player.position));
    // Cross the same thin plane from the other side.
    player.position.x = 3.0;
    player.tick(&world, input(-Vec3::X, 40.0));
    assert!(player.position.x > 1.36 && player.position.x < 1.37);
}
#[test]
fn stepping_climbs_small_voxels_but_not_tall_walls_or_low_ceilings() {
    let mut world = floor();
    for x in 327..345 {
        for z in 310..330 {
            for y in 17..21 {
                world.set(x, y, z, world::STONE);
            }
        }
    }
    let mut player = Player::new(feet());
    player.tick(&world, Input::default());
    for _ in 0..5 {
        player.tick(&world, input(Vec3::X, 5.0));
    }
    assert!(player.position.x > 0.8);
    assert!((player.position.y - 21.0 * VOXEL).abs() < 0.00003);
    assert!(player.grounded && Player::can_occupy(&world, player.position));
    // The same step cannot push the player's head through a low roof.
    for x in 310..345 {
        for z in 310..330 {
            world.set(x, 47, z, world::STONE);
        }
    }
    let mut player = Player::new(feet());
    player.tick(&world, Input::default());
    for _ in 0..10 {
        player.tick(&world, input(Vec3::X, 5.0));
    }
    assert!(player.position.x < 0.15);
    assert!(Player::can_occupy(&world, player.position));
}
#[test]
fn jumping_hits_ceilings_and_water_is_not_solid() {
    let mut world = floor();
    for x in 310..330 {
        for z in 310..330 {
            world.set(x, 49, z, world::STONE);
        }
    }
    let mut player = Player::new(feet());
    player.tick(&world, Input::default());
    player.tick(
        &world,
        Input {
            jump: true,
            ..Default::default()
        },
    );
    assert!(player.velocity.y == 0.0);
    assert!(player.bounds().max.y <= 49.0 * VOXEL);
    for x in 327..345 {
        for y in 17..45 {
            for z in 310..330 {
                world.set(x, y, z, world::WATER);
            }
        }
    }
    player.mode = Mode::Fly;
    for _ in 0..8 {
        player.tick(&world, input(Vec3::X, 5.0));
    }
    assert!(player.position.x > 1.9);
}
#[test]
fn world_edges_stop_motion_and_spawn_is_clear() {
    let world = World::generate();
    let mut player = Player::spawn();
    assert!(Player::can_occupy(&world, player.position));
    player.tick(&world, Input::default());
    assert!(player.grounded);
    player.mode = Mode::Fly;
    player.position = Vec3::new(19.5, 6.0, 0.0);
    player.tick(&world, input(Vec3::X, 40.0));
    assert!(player.position.x <= 19.7 && player.position.x > 19.69);
}

#[test]
fn flight_keeps_shift_down_and_control_boost_bindings() {
    use std::collections::HashSet;
    use winit::keyboard::KeyCode;
    let world = World::empty();
    for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
        let mut player = Player::new(Vec3::new(0.0, 5.0, 0.0));
        player.mode = Mode::Fly;
        player.tick(&world, Input::from_keys(&HashSet::from([shift]), 0.0, 5.0));
        assert!((player.position.y - 4.75).abs() < 0.00001);
    }
    for control in [KeyCode::ControlLeft, KeyCode::ControlRight] {
        let mut player = Player::new(Vec3::new(0.0, 5.0, 0.0));
        player.mode = Mode::Fly;
        player.tick(
            &world,
            Input::from_keys(&HashSet::from([KeyCode::KeyW, control]), 0.0, 5.0),
        );
        assert!((player.position.x - 0.75).abs() < 0.00001);
    }
}
#[test]
fn jump_clears_one_metre_and_returns_to_ground() {
    let world = floor();
    let mut player = Player::new(feet());
    player.tick(&world, Input::default());
    let start = player.position.y;
    player.tick(
        &world,
        Input {
            jump: true,
            ..Default::default()
        },
    );
    let mut peak = player.position.y;
    for _ in 0..25 {
        player.tick(&world, Input::default());
        peak = peak.max(player.position.y);
    }
    assert!(peak - start > 1.2 && peak - start < 1.3);
    assert!(player.grounded);
    assert!((player.position.y - start).abs() < 0.00001);
}

#[test]
fn infinite_walking_crosses_the_old_edge_and_waits_for_unloaded_collision_data() {
    use crate::{
        chunk::Chunk,
        physics::{collision, Aabb, Input, Player},
        world::{World, AIR, STONE},
    };
    use glam::Vec3;
    let mut world = World::with_height(192);
    world.infinite = true;
    for x in 20..=26 {
        for z in 9..=11 {
            for y in 0..=3 {
                world.insert_page([x, y, z], Chunk::Uniform(if y == 0 { STONE } else { AIR }));
            }
        }
    }
    let mut player = Player::new(World::meters(664, 32, 320));
    for _ in 0..10 {
        player.tick(
            &world,
            Input {
                direction: Vec3::X,
                speed: 5.0,
                ..Default::default()
            },
        );
    }
    assert!(player.position.x > 23.0);
    let body = Player::bounds_at(World::meters(856, 32, 320));
    let motion = collision::move_body(&world, body, Vec3::new(4.0, 0.0, 0.0), true);
    assert!(motion.delta.x >= 0.0 && motion.delta.x <= 0.201);
    assert!(!Player::can_occupy(&world, World::meters(-100, 64, -100)));
    let mut waiting = Player::new(World::meters(-100, 64, -100));
    let before = waiting.position;
    waiting.tick(
        &world,
        Input {
            direction: Vec3::X,
            speed: 5.0,
            ..Default::default()
        },
    );
    assert_eq!(waiting.position, before);
    assert!(Aabb::voxel([672, 32, 320]).max.x > 20.0);
    let body = Player::bounds_at(Vec3::new(12000.0, 0.5, -12000.0));
    let motion = collision::move_body(&world, body, -Vec3::Y, false);
    assert!((motion.delta.y + 0.5).abs() < 0.00003);
    assert!(motion.grounded);
}
