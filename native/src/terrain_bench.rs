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
