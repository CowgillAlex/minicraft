use crate::{
    terrain_region::{center, Builder},
    world::World,
    worldgen::{preview::Preview, sky::Sky, Generator},
};
use std::time::Instant;

#[test]
#[ignore = "manual streaming CPU benchmark"]
fn flight_streaming_benchmark() {
    for (name, origin, y) in [
        ("surface", [120, -64, -20], 15),
        ("cave", [134, 20, -37], 3),
    ] {
        let generator = Generator::new(1).unwrap();
        let mut world = World::with_height(384 * 16);
        world.infinite = true;
        world.noise_preview = Some(Preview {
            origin,
            height_m: if name == "cave" { 48 } else { 384 },
            ..Default::default()
        });
        let mut builder = Builder::new(world, &generator, Sky::default());
        for x in [0, 1, 2, 3, 0] {
            let key = [x, y, 2];
            let start = Instant::now();
            let region = builder.build(key, 0, center(key), || false).unwrap();
            println!(
                "BENCH {name} {x} {:.6} {} {}",
                start.elapsed().as_secs_f64(),
                region.meshes.len(),
                region.bytes
            );
        }
    }
}

#[test]
#[ignore = "manual surfaced streaming CPU benchmark"]
fn surface_streaming_benchmark() {
    let generator = Generator::new(1).unwrap();
    let mut world = World::with_height(384 * 16);
    world.infinite = true;
    world.noise_preview = Some(Preview {
        origin: [0, -64, 0],
        surfaces: true,
        ..Default::default()
    });
    let mut builder = Builder::new(world, &generator, Sky::default());
    for x in [0, 1, 2, 0] {
        let key = [x, 16, 0];
        let start = Instant::now();
        let region = builder.build(key, 0, center(key), || false).unwrap();
        println!(
            "SURFACE REGION {x} {:.3}s {} meshes {} bytes",
            start.elapsed().as_secs_f64(),
            region.meshes.len(),
            region.bytes
        );
    }
}

#[test]
#[ignore = "manual terrain LOD CPU and memory survey"]
fn lod_region_benchmark() {
    use crate::terrain_lod::Level;
    for (name, seed, origin, height, key) in [
        ("surface", 1, [0, -64, 0], 384, [0, 16, 0]),
        ("watery cave", 1, [134, 20, -37], 48, [0, 3, 2]),
        ("seed -67 cave", -67, [120, -64, -20], 384, [-21, 2, 6]),
    ] {
        let generator = Generator::new(seed).unwrap();
        let mut world = World::with_height(height * 16);
        world.infinite = true;
        world.noise_preview = Some(Preview {
            seed,
            origin,
            height_m: height,
            surfaces: true,
            ..Default::default()
        });
        let mut builder = Builder::new(world, &generator, Sky::default());
        for level in [Level::Full, Level::Medium, Level::Far] {
            let start = Instant::now();
            let region = builder
                .build_lod(key, 0, center(key), level, || false)
                .unwrap();
            let triangles: usize = region
                .meshes
                .iter()
                .map(|(_, m)| m.indices.len() / 3 + m.transparent_indices.len() / 3)
                .sum();
            println!(
                "LOD REGION {name} {level:?}: {triangles} triangles, {} bytes, {:.3}s",
                region.bytes,
                start.elapsed().as_secs_f64()
            );
        }
    }
}

/// Production feature streaming at fixed source positions, including cold,
/// adjacent and cached-return builds. Run separately from other CPU benchmarks.
#[test]
#[ignore = "manual decorated flight benchmark"]
fn decorated_flight_benchmark() {
    for (name, seed, x, z) in [
        ("forest", 1794358123416304200i64, -1800., -2056.),
        ("savanna", 8349847389, 84., 28.),
        ("aquifer surface", 1794358123416304200, 272., -140.),
    ] {
        let generator = Generator::new(seed).unwrap();
        let ground = generator.sampler().feature_ground(x, z);
        let eye = World::meters(
            (x * 16.) as i32,
            ((ground.y + 65.) * 16.) as i32,
            (z * 16.) as i32,
        );
        let first = crate::terrain_region::key(eye);
        let mut world = World::with_height(384 * 16);
        world.infinite = true;
        world.noise_preview = Some(Preview {
            seed,
            origin: [0, -64, 0],
            surfaces: true,
            features: true,
            ..Default::default()
        });
        let mut builder = Builder::new(world, &generator, Sky::default());
        for step in [0, 1, 2, 0] {
            let key = [first[0] + step, first[1], first[2]];
            let start = Instant::now();
            let result = builder.build(key, 0, center(key), || false).unwrap();
            println!(
                "DECORATED {name} step={step} elapsed={:.5} bytes={}",
                start.elapsed().as_secs_f64(),
                result.bytes
            );
        }
    }
}
