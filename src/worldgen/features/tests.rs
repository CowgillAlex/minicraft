use super::*;
#[test]
fn every_pinned_biome_has_a_feature_profile() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/worldgen/biomes.json")).unwrap();
    for row in data.as_array().unwrap() {
        ecology::habitat(row[6].as_str().unwrap());
    }
}
#[test]
fn recipes_have_real_stems_grains_and_branching_wood() {
    let mut rng = Rng::new(1, "test", [0; 3]);
    let mut shapes = Vec::new();
    plants::wheat(&mut shapes, V::ZERO, &mut rng);
    assert!(shapes.iter().any(|p| p.material == GRAIN));
    assert!(shapes.iter().any(|p| p.material == WHEAT_STEM));
    shapes.clear();
    trees::tree(&mut shapes, V::ZERO, Habitat::Forest, &mut rng);
    assert!(shapes.iter().filter(|p| p.material == BARK).count() > 30);
    assert!(shapes.iter().any(|p| p.material == HEARTWOOD));
    assert!(shapes.iter().any(|p| p.material == LEAF));
}
#[test]
fn feature_pages_and_optics_are_independent_of_loading_order() {
    let generator = crate::worldgen::Generator::new(1794358123416304200).unwrap();
    let mut a = generator.sampler();
    let mut b = generator.sampler();
    let ground = a.feature_ground(272., -140.);
    let cell = (ground * 16.).as_ivec3().to_array();
    let key = cell.map(|v| v.div_euclid(32));
    let keys = [
        key,
        [key[0] + 1, key[1], key[2]],
        [key[0], key[1] + 1, key[2]],
        [key[0] - 1, key[1], key[2]],
        [key[0], key[1] + 2, key[2]],
    ];
    let preview = crate::worldgen::preview::Preview {
        seed: 1794358123416304200,
        origin: [0; 3],
        surfaces: true,
        features: true,
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let expected: Vec<_> = keys
        .into_iter()
        .map(|key| crate::worldgen::page::generate(preview, &mut a, key.map(|v| v * 32)).data)
        .collect();
    let parts = expected
        .iter()
        .flat_map(|c| c.to_vec())
        .filter(|m| *m >= STEM)
        .count();
    assert!(
        parts > 0,
        "The real terrain pages must contain generated plant or mineral parts"
    );
    for i in (0..keys.len()).rev() {
        let actual = crate::worldgen::page::generate(preview, &mut b, keys[i].map(|v| v * 32)).data;
        assert_eq!(actual.to_vec(), expected[i].to_vec());
        for n in (0..32768).step_by(181) {
            let p = [
                keys[i][0] * 32 + (n % 32) as i32,
                keys[i][1] * 32 + (n / 1024) as i32,
                keys[i][2] * 32 + ((n / 32) % 32) as i32,
            ];
            assert_eq!(
                crate::light_field::loss(actual.get(n)),
                crate::light_field::loss(b.world_material(preview.point(p), true, true)),
                "{p:?}"
            );
        }
    }
    println!("FEATURE three-page parity and optics {:?}", start.elapsed());
}
#[test]
fn source_height_distributions_keep_their_ranges_and_concentration() {
    let rules = ores::rules();
    for rule in rules {
        let range = &rule["placement"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["type"] == "minecraft:height_range")
            .unwrap()["height"];
        let mut rng = Rng::new(1, rule["name"].as_str().unwrap(), [0; 3]);
        let values: Vec<_> = (0..10000).map(|_| ores::height(&mut rng, range)).collect();
        if range["type"] == "minecraft:trapezoid" {
            let min = *values.iter().min().unwrap();
            let max = *values.iter().max().unwrap();
            let mid = (min + max) / 2;
            let quarter = (max - min) / 4;
            let centre = values
                .iter()
                .filter(|&&y| (y - mid).abs() < quarter)
                .count();
            assert!(
                centre > 6500,
                "{} must concentrate near its peak",
                rule["name"]
            );
        }
    }
    assert_eq!(geometry::apply((3 << 8) | u16::from(DIAMOND), WATER), WATER);
    assert_eq!(geometry::apply((3 << 8) | u16::from(DIAMOND), DIRT), DIRT);
    assert_eq!(
        geometry::apply((3 << 8) | u16::from(DIAMOND), DEEPSLATE),
        DIAMOND
    );
}
#[test]
fn generated_cabin_has_lamps_and_a_working_editable_door() {
    let mut world = showcase::Showcase::Cabin.build().unwrap();
    assert!(!world.emitters.is_empty());
    let hit = [
        (19.5 * 16.) as i32,
        (1.3125 * 16.) as i32,
        (21.75 * 16.) as i32,
    ];
    assert!(world.recognize_generated_door(hit).is_some());
    assert_eq!(world.doors.len(), 1);
    assert_eq!(world.doors[0].status(), crate::doors::Status::Ready);
    let far = crate::physics::Aabb {
        min: glam::Vec3::splat(100.),
        max: glam::Vec3::splat(101.),
    };
    assert_eq!(
        world.toggle_door(0, far, far).0,
        crate::world::DoorAction::Changed
    );
    assert_eq!(
        world.toggle_door(0, far, far).0,
        crate::world::DoorAction::Changed
    );
}
#[test]
fn soft_parts_filter_light_and_stay_walkable_while_wood_remains_solid() {
    for m in [
        STEM,
        GRASS_BLADE,
        SEED,
        GRAIN,
        LEAF,
        NEEDLES,
        FERN,
        SEAGRASS,
        KELP,
    ] {
        assert!(!crate::materials::collides(m));
        assert!(crate::materials::light_loss(m) < 255);
    }
    for m in [BARK, HEARTWOOD, BAMBOO, STONE, DIAMOND] {
        assert!(crate::materials::collides(m));
    }
}
#[test]
#[ignore = "manual feature streaming CPU benchmark"]
fn features_streaming_benchmark() {
    let seed = 1794358123416304200;
    let generator = crate::worldgen::Generator::new(seed).unwrap();
    let mut sampler = generator.sampler();
    let p = sampler.feature_ground(272., -140.);
    let cell = (p * 16.).as_ivec3().to_array();
    let mut world = crate::world::World::with_height(384 * 16);
    world.infinite = true;
    world.noise_preview = Some(crate::worldgen::preview::Preview {
        seed,
        origin: [0, -64, 0],
        surfaces: true,
        features: true,
        ..Default::default()
    });
    let focus = crate::world::World::meters(cell[0], cell[1] + 1024, cell[2]);
    let key = crate::terrain_region::key(focus);
    let mut builder = crate::terrain_region::Builder::new(world, &generator, Default::default());
    let start = std::time::Instant::now();
    let region = builder.build(key, 0, focus, || false).unwrap();
    println!(
        "FEATURE STREAM {:?}, {} meshes, {} GPU bytes",
        start.elapsed(),
        region.meshes.len(),
        region.bytes
    );
}
#[test]
fn unloaded_procedural_lamps_are_found_and_player_removals_mask_them() {
    use crate::{light_field::LightSource, world::World};
    let generator = crate::worldgen::Generator::new(1).unwrap();
    let mut sampler = generator.sampler();
    let lo = V::new(20., 68., 28.);
    sampler.features.areas.insert(
        [2, 3],
        Arc::new(vec![Primitive::cuboid(
            lo,
            lo + V::splat(0.125),
            LIGHT,
            101,
        )]),
    );
    let mut world = World::with_height(384 * 16);
    world.infinite = true;
    world.noise_preview = Some(crate::worldgen::preview::Preview {
        features: true,
        ..Default::default()
    });
    // Translate reference coordinates to the scene's voxel coordinates.
    let preview = world.noise_preview.unwrap();
    let cell = std::array::from_fn(|i| (lo[i] * 16.) as i32 - preview.origin[i] * 16);
    let origin = cell.map(|v| v - 8);
    let mut sky = Default::default();
    let mut tiles = Default::default();
    let expected = {
        let mut source =
            crate::light_tiles::Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false);
        source.emitters(origin, [8; 3])
    };
    assert_eq!(expected.len(), 8);
    assert!(expected.contains(&cell));
    for &p in &expected {
        world.set(p[0], p[1], p[2], AIR);
    }
    let mut source =
        crate::light_tiles::Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false);
    assert!(source.emitters(origin, [8; 3]).is_empty());
}
#[test]
#[ignore = "manual landmark search for procedural screenshots"]
fn feature_landmarks() {
    let generator = crate::worldgen::Generator::new(1794358123416304200).unwrap();
    let mut sampler = generator.sampler();
    let mut found = std::collections::HashSet::new();
    for z in (-2048..=2048).step_by(256) {
        for x in (-2048..=2048).step_by(256) {
            let p = sampler.feature_ground(x as f64, z as f64);
            let (h, name) = sampler.feature_habitat(p.to_array());
            let water = sampler.material_fast((p + V::Y * 0.1).to_array()) == WATER;
            if found.insert((h as u8, water)) {
                println!("LANDMARK {name} {x} {:.3} {z} wet={water}", p.y);
            }
        }
    }
}
#[test]
fn procedural_sky_filters_leaves_and_glass_but_lamps_still_occlude_it() {
    let generator = crate::worldgen::Generator::new(1).unwrap();
    let mut sampler = generator.sampler();
    let mut world = crate::world::World::with_height(192);
    world.infinite = true;
    world.noise_preview = Some(crate::worldgen::preview::Preview {
        origin: [120, 58, -20],
        height_m: 12,
        ..Default::default()
    });
    for y in 0..192 {
        world.set(2, y, 2, AIR);
    }
    world.set(2, 100, 2, GLASS);
    world.set(2, 90, 2, LEAF);
    let mut sky = crate::worldgen::sky::Sky::default();
    let values = sky.values(&world, &mut sampler, [2, 2], 0, 48);
    assert_eq!(values[25], 255);
    assert_eq!(values[23], 252);
    assert_eq!(values[21], 244);
    world.set(2, 80, 2, LIGHT);
    sky.invalidate(&[[2, 80, 2]]);
    let values = sky.values(&world, &mut sampler, [2, 2], 0, 48);
    assert_eq!(values[20], 244);
    assert_eq!(values[19], 0);
}
