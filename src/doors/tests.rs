use super::*;
#[test]
fn prefab_frame_and_handle_control_function_without_restricting_the_infill() {
    let mut door = Door::prefab([320, 32, 320]);
    assert_eq!(door.status(), Status::Ready);
    door.set([8, 10, 2], world::AIR);
    assert_eq!(door.status(), Status::Ready);
    door.set([0, 10, 2], world::AIR);
    assert_eq!(door.status(), Status::FrameBroken);
    door.set([0, 10, 2], world::WOOD);
    assert_eq!(door.status(), Status::Ready);
    door.set([14, 15, 3], world::AIR);
    assert_eq!(door.status(), Status::HandleMissing);
    door.set([14, 15, 3], world::HANDLE);
    assert_eq!(door.status(), Status::Ready);
}

fn far() -> Aabb {
    Aabb {
        min: Vec3::splat(100.0),
        max: Vec3::splat(101.0),
    }
}
#[test]
fn cell_transforms_round_trip_for_both_hinges_all_facings_and_open_states() {
    for facing in 0..4 {
        for hinge_right in [false, true] {
            for open in [false, true] {
                let mut door = Door::prefab([300, 32, 300]);
                door.facing = facing;
                door.hinge_right = hinge_right;
                door.open = open;
                for z in 0..4 {
                    for y in 0..32 {
                        for x in 0..16 {
                            let p = [x, y, z];
                            assert_eq!(door.local_cell(door.world_cell(p)), Some(p));
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn attaching_a_handle_recognizes_custom_infill_and_all_four_wall_facings() {
    for facing in 0..4 {
        for hinge_right in [false, true] {
            let mut world = World::empty();
            let mut design = Door::prefab([320, 32, 320]);
            design.facing = facing;
            design.hinge_right = hinge_right;
            design.set([14, 15, 3], world::AIR);
            let handle = [if hinge_right { 1 } else { 14 }, 15, 3];
            design.set([8, 10, 1], world::STONE);
            design.set([8, 10, 2], world::AIR);
            for (p, material) in design.occupied() {
                world.set(p[0], p[1], p[2], material);
            }
            let cell = design.closed_cell(handle);
            world.edit(cell, world::HANDLE).unwrap();
            assert_eq!(world.doors.len(), 1, "facing {facing}, right {hinge_right}");
            let door = &world.doors[0];
            assert_eq!(door.status(), Status::Ready);
            assert_eq!(door.hinge_right, hinge_right);
            assert_eq!(door.get([8, 10, 1]), world::STONE);
            assert_eq!(door.get([8, 10, 2]), world::AIR);
            for (p, material) in door.occupied() {
                assert_eq!(world.get(p[0], p[1], p[2]), material);
                assert_eq!(world.get_static(p[0], p[1], p[2]), world::AIR);
            }
        }
    }
}
#[test]
fn breaking_and_repairing_an_open_door_keeps_its_shape_pose_and_snapshots() {
    let mut world = World::empty();
    world.install_door(Door::prefab([320, 32, 320]));
    let before = world.clone();
    let count = world.voxels;
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::Changed
    );
    assert_eq!(world.voxels, count);
    let p = world.doors[0].world_cell([0, 10, 2]);
    world.edit(p, world::AIR).unwrap();
    assert!(world.doors[0].open);
    assert_eq!(world.doors[0].status(), Status::FrameBroken);
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::FrameBroken
    );
    assert_eq!(before.doors[0].status(), Status::Ready);
    assert!(!before.doors[0].open);
    world.edit(p, world::WOOD).unwrap();
    let handle = world.doors[0].world_cell([14, 15, 3]);
    world.edit(handle, world::AIR).unwrap();
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::HandleMissing
    );
    world.edit(handle, world::HANDLE).unwrap();
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::Changed
    );
    assert!(!world.doors[0].open);
    assert_eq!(world.voxels, count);
}
#[test]
fn doors_cannot_close_through_the_player_or_world_geometry() {
    let mut world = World::empty();
    world.install_door(Door::prefab([320, 32, 320]));
    let cell = world.doors[0].world_cell([8, 10, 2]);
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::Changed
    );
    let body = Aabb::voxel(cell);
    assert_eq!(
        world.toggle_door(0, body, far()).0,
        world::DoorAction::Blocked
    );
    assert_eq!(
        world.toggle_door(0, far(), body).0,
        world::DoorAction::Blocked
    );
    world.set(cell[0], cell[1], cell[2], world::STONE);
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::Blocked
    );
    world.set(cell[0], cell[1], cell[2], world::AIR);
    assert_eq!(
        world.toggle_door(0, far(), far()).0,
        world::DoorAction::Changed
    );
}
#[test]
fn generated_entrances_are_walkable_when_open_and_closed_doors_block_them() {
    use crate::physics::{Input, Player};
    let mut world = World::generate();
    assert_eq!(world.doors.len(), 3);
    let door = world.doors[0].clone();
    let origin = World::meters(door.origin[0], door.origin[1], door.origin[2]);
    let mut player = Player::new(origin + Vec3::new(0.5, 0.00002, 13.0 * VOXEL));
    for _ in 0..8 {
        player.tick(
            &world,
            Input {
                direction: -Vec3::Z,
                speed: 5.0,
                ..Default::default()
            },
        );
    }
    assert!(player.position.z > origin.z + 0.35);
    assert_eq!(
        world.toggle_door(0, player.bounds(), player.bounds()).0,
        world::DoorAction::Changed
    );
    for _ in 0..16 {
        player.tick(
            &world,
            Input {
                direction: -Vec3::Z,
                speed: 5.0,
                ..Default::default()
            },
        );
    }
    assert!(player.position.z < origin.z - 0.5);
    assert!(Player::can_occupy(&world, player.position));
}

#[test]
fn repairing_a_frame_after_placing_its_handle_recognizes_it() {
    let mut world = World::empty();
    let mut design = Door::prefab([320, 32, 320]);
    design.set([14, 15, 3], world::AIR);
    design.set([0, 10, 2], world::AIR);
    for (p, material) in design.occupied() {
        world.set(p[0], p[1], p[2], material);
    }
    let handle = design.closed_cell([14, 15, 3]);
    world.edit(handle, world::HANDLE).unwrap();
    assert!(world.doors.is_empty());
    let missing = design.closed_cell([0, 10, 2]);
    world.edit(missing, world::WOOD).unwrap();
    assert_eq!(world.doors.len(), 1);
    assert_eq!(world.doors[0].status(), Status::Ready);
}
#[test]
fn lights_inside_an_object_move_with_it_and_do_not_leave_stale_sources() {
    let mut world = World::empty();
    world.install_door(Door::prefab([320, 32, 320]));
    let source = world.doors[0].world_cell([8, 10, 3]);
    world.edit(source, world::LIGHT).unwrap();
    assert!(world.emitters.contains(&source));
    world.toggle_door(0, far(), far());
    let moved = world.doors[0].world_cell([8, 10, 3]);
    assert_ne!(moved, source);
    assert!(!world.emitters.contains(&source));
    assert!(world.emitters.contains(&moved));
    world.edit(moved, world::AIR).unwrap();
    assert!(!world.emitters.contains(&moved));
}
