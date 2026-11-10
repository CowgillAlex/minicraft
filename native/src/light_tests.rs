use super::*;
#[test]
#[ignore = "manual lighting benchmark"]
fn benchmark_lighting_edits() {
    let mut world = World::generate();
    let start = std::time::Instant::now();
    let mut field = LightField::build(&world);
    println!("Initial bake: {:?}", start.elapsed());
    let mut displayed = field.pixels.clone();
    for (cell, materials) in [
        ([320, 60, 320], vec![world::LIGHT, world::AIR]),
        (
            [322, 62, 322],
            vec![
                world::STONE,
                world::GLASS,
                world::AIR,
                world::LIGHT,
                world::AIR,
            ],
        ),
    ] {
        for material in materials {
            world.set(cell[0], cell[1], cell[2], material);
            let start = std::time::Instant::now();
            let patch = field.update(&world, &[cell]);
            let elapsed = start.elapsed();
            let bytes = patch.as_ref().map_or(0, |p| p.pixels.len());
            if let Some(patch) = patch {
                apply_patch(&mut displayed, &patch, field.grid);
            }
            println!("Edit {cell:?} material {material}: {elapsed:?}, {bytes} upload bytes");
        }
    }
    // Validate the complete generated-world result after the timed sequence.
    check_update(&world, &mut field, &mut displayed, &[]);
}
fn room() -> World {
    let mut world = World::empty();
    for x in 300..=340 {
        for y in 16..=56 {
            for z in 300..=340 {
                if x == 300 || x == 340 || y == 16 || y == 56 || z == 300 || z == 340 {
                    world.set(x, y, z, world::STONE);
                }
            }
        }
    }
    world.set(294, 34, 322, world::LIGHT);
    world
}
fn window(world: &mut World, material: u8) {
    for y in 30..=38 {
        for z in 318..=326 {
            world.set(300, y, z, material);
        }
    }
}
#[test]
fn walls_block_light_and_glass_admits_both_warm_light_and_skylight() {
    let mut world = room();
    assert_eq!(LightField::build(&world).at([310, 34, 322]), [0, 0]);
    window(&mut world, world::GLASS);
    let glass = LightField::build(&world).at([310, 34, 322]);
    assert!(
        glass[0] > 30,
        "lamp should illuminate through glass: {glass:?}"
    );
    assert!(glass[1] > 0, "skylight should enter the window");
    window(&mut world, world::AIR);
    let air = LightField::build(&world).at([310, 34, 322]);
    assert!(air[0] >= glass[0]);
    assert!(air[1] >= glass[1]);
    window(&mut world, world::STONE);
    assert_eq!(LightField::build(&world).at([310, 34, 322]), [0, 0]);
}
#[test]
fn warm_light_falls_off_and_removing_the_emitter_removes_illumination() {
    let mut world = World::empty();
    world.set(320, 40, 320, world::LIGHT);
    let field = LightField::build(&world);
    assert!(field.at([322, 42, 322])[0] > field.at([350, 42, 322])[0]);
    assert_eq!(field.at([480, 42, 322])[0], 0);
    world.set(320, 40, 320, world::AIR);
    assert_eq!(LightField::build(&world).at([322, 42, 322])[0], 0);
}
#[test]
fn snapshots_are_independent_and_thin_walls_are_not_skipped() {
    let mut world = World::empty();
    world.set(321, 40, 320, world::STONE);
    let from = World::meters(318, 40, 320) + Vec3::splat(VOXEL * 0.5);
    let to = World::meters(326, 40, 320) + Vec3::splat(VOXEL * 0.5);
    assert_eq!(segment_loss(&world, from, to), BLOCKED);
    let snapshot = world.clone();
    world.set(321, 40, 320, world::GLASS);
    assert_eq!(snapshot.get(321, 40, 320), world::STONE);
    assert!(segment_loss(&world, from, to) < BLOCKED);
    world.set(321, 40, 320, world::LIGHT);
    assert!(world.emitters.contains(&[321, 40, 320]));
    world.set(321, 40, 320, world::AIR);
    assert!(!world.emitters.contains(&[321, 40, 320]));
}

fn apply_patch(pixels: &mut [u8], patch: &LightPatch, grid: Grid) {
    let mut source = 0;
    for z in patch.origin[2]..patch.origin[2] + patch.size[2] {
        for y in patch.origin[1]..patch.origin[1] + patch.size[1] {
            let start = grid.index(patch.origin[0], y, z) * 2;
            let length = patch.size[0] * 2;
            pixels[start..start + length].copy_from_slice(&patch.pixels[source..source + length]);
            source += length;
        }
    }
    assert_eq!(source, patch.pixels.len());
}
fn check_update(world: &World, field: &mut LightField, displayed: &mut [u8], edits: &[[i32; 3]]) {
    if let Some(patch) = field.update(world, edits) {
        apply_patch(displayed, &patch, field.grid);
    }
    assert_eq!(
        displayed, field.pixels,
        "partial uploads must cover all changed samples"
    );
    let rebuilt = LightField::build(world);
    assert!(
        field.edges == rebuilt.edges,
        "cached connections differ from a fresh bake"
    );
    assert!(
        field.seeds == rebuilt.seeds,
        "cached sources differ from a fresh bake"
    );
    if let Some(i) = field
        .pixels
        .iter()
        .zip(&rebuilt.pixels)
        .position(|(a, b)| a != b)
    {
        panic!(
            "incremental lighting differs at {:?} channel {}: {} vs {}",
            field.grid.position(i / 2),
            i % 2,
            field.pixels[i],
            rebuilt.pixels[i]
        );
    }
}
#[test]
fn incremental_windows_lamps_and_roofs_match_full_bakes() {
    let mut world = room();
    let mut field = LightField::build(&world);
    let mut displayed = field.pixels.clone();
    let opening: Vec<_> = (30..=38)
        .flat_map(|y| (318..=326).map(move |z| [300, y, z]))
        .collect();
    for material in [world::GLASS, world::AIR, world::STONE] {
        window(&mut world, material);
        check_update(&world, &mut field, &mut displayed, &opening);
    }
    for material in [world::LIGHT, world::AIR, world::GLASS, world::STONE] {
        world.set(322, 42, 322, material);
        check_update(&world, &mut field, &mut displayed, &[[322, 42, 322]]);
    }
    // A one-voxel roof opening changes sky sources far below the edited cell.
    for material in [world::AIR, world::GLASS, world::STONE] {
        world.set(322, 56, 322, material);
        check_update(&world, &mut field, &mut displayed, &[[322, 56, 322]]);
    }
}
#[test]
fn coalesced_edits_and_overlapping_sources_match_full_bakes() {
    let mut world = room();
    window(&mut world, world::GLASS);
    world.set(318, 34, 322, world::LIGHT);
    world.set(330, 34, 322, world::LIGHT);
    let mut field = LightField::build(&world);
    let mut displayed = field.pixels.clone();
    let mut edits = Vec::new();
    for cell in [[318, 34, 322], [330, 34, 322], [294, 34, 322]] {
        world.set(cell[0], cell[1], cell[2], world::AIR);
        edits.push(cell);
    }
    for (cell, material) in [
        ([326, 38, 322], world::LIGHT),
        ([0, 190, 0], world::LIGHT),
        ([639, 191, 639], world::GLASS),
        ([302, 34, 322], world::STONE),
        ([300, 34, 322], world::STONE),
    ] {
        world.set(cell[0], cell[1], cell[2], material);
        edits.push(cell);
    }
    check_update(&world, &mut field, &mut displayed, &edits);
    // Removing the final source must clear its complete propagated footprint.
    world.set(326, 38, 322, world::AIR);
    world.set(0, 190, 0, world::AIR);
    check_update(
        &world,
        &mut field,
        &mut displayed,
        &[[326, 38, 322], [0, 190, 0]],
    );
    assert!(field.pixels.iter().step_by(2).all(|&v| v == 0));
}
#[test]
fn axis_connections_preserve_fine_voxel_transmission() {
    let mut world = World::empty();
    for x in 318..=326 {
        world.set(
            x,
            42,
            322,
            if x % 3 == 0 { world::GLASS } else { world::AIR },
        );
    }
    for axis in 0..3 {
        let p = [79, 10, 80];
        let mut next = p;
        next[axis] += 1;
        assert_eq!(
            edge_loss(&world, p, axis),
            segment_loss(
                &world,
                center(p[0], p[1], p[2]),
                center(next[0], next[1], next[2])
            )
        );
    }
}

#[test]
fn door_motion_updates_light_through_the_opening_and_matches_full_bakes() {
    use crate::{doors::Door, physics::Aabb};
    let mut world = World::empty();
    for x in 312..=344 {
        for y in 17..=52 {
            for z in 288..=320 {
                if x == 312 || x == 344 || y == 17 || y == 52 || z == 288 || z == 320 {
                    world.set(x, y, z, world::STONE);
                }
            }
        }
    }
    let mut door = Door::prefab([320, 18, 318]);
    for x in 1..15 {
        for y in 1..31 {
            for z in 1..=2 {
                door.set([x, y, z], world::WOOD);
            }
        }
    }
    world.install_door(door);
    world.set(330, 34, 326, world::LIGHT);
    let mut field = LightField::build(&world);
    let mut displayed = field.pixels.clone();
    let closed = field.at([330, 34, 310]);
    assert_eq!(closed, [0, 0]);
    let far = Aabb {
        min: Vec3::splat(100.0),
        max: Vec3::splat(101.0),
    };
    let (_, edits) = world.toggle_door(0, far, far);
    check_update(&world, &mut field, &mut displayed, &edits);
    let open = field.at([330, 34, 310]);
    assert!(
        open[0] > 0 && open[1] > 0,
        "open door should admit lamp and sky light: {open:?}"
    );
    let (_, edits) = world.toggle_door(0, far, far);
    check_update(&world, &mut field, &mut displayed, &edits);
    assert_eq!(field.at([330, 34, 310]), closed);
}

#[test]
fn lighting_and_incremental_uploads_follow_the_actual_world_height() {
    let mut world = World::with_height(16 * 16);
    world.set(320, 230, 320, world::LIGHT);
    let mut field = LightField::build(&world);
    assert_eq!(field.size(), [160, 64, 160]);
    assert!(field.at([320, 234, 320])[0] > 0);
    let mut displayed = field.pixels.clone();
    world.set(320, 230, 320, world::AIR);
    check_update(&world, &mut field, &mut displayed, &[[320, 230, 320]]);
    assert_eq!(field.at([320, 234, 320])[0], 0);
}
