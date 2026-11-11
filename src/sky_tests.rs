use super::shadow_tests::{looking_at, pixels};
use super::*;
use crate::{
    view_settings::Change,
    world::{self, World},
};
use glam::Vec3;
fn center(data: &[u8]) -> f32 {
    let mut sum = 0.0;
    for y in 254..258 {
        for x in 254..258 {
            for c in 0..3 {
                sum += f32::from(data[(y * 512 + x) * 4 + c]);
            }
        }
    }
    sum / 48.0
}
#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn fog_never_draws_the_moon_disc_in_front_of_opaque_terrain() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    renderer.hud_enabled = false;
    renderer.ao_enabled = false;
    renderer.daylight.hours = 18.0;
    renderer.view.set_distance(48).unwrap();
    renderer.view.change(Change::Start(-127));
    renderer.view.change(Change::Density(127));
    let direction = renderer.daylight.parameters().0;
    let point = Vec3::new(0.0, 3.0, 0.0);
    let camera = looking_at(point - direction * 28.0, point);
    let mut world = World::empty();
    for z in 256..384 {
        for y in 0..96 {
            for x in 256..384 {
                world.set(x, y, z, world::STONE);
            }
        }
    }
    renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.shadow_ready = false;
    let covered = center(&pixels(&mut renderer, &camera));
    let world = World::empty();
    renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.shadow_ready = false;
    let visible = center(&pixels(&mut renderer, &camera));
    println!("Moon disc: open {visible:.2}, behind fogged terrain {covered:.2}");
    assert!(
        visible > covered + 30.0,
        "celestial discs must be occluded even when terrain is fogged"
    );
}

#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn cave_backdrop_and_distant_fog_are_black_with_night_vision() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    renderer.hud_enabled = false;
    renderer.ao_enabled = false;
    renderer.night_vision = true;
    renderer.daylight.hours = 12.0;
    renderer.view.set_distance(24).unwrap();
    renderer.view.change(Change::Start(-127));
    renderer.view.change(Change::Density(127));
    let camera = looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 2.0, -1.0));
    let mut readings = Vec::new();
    for (roof, wall) in [(false, false), (true, false), (true, true)] {
        let mut world = World::empty();
        if roof {
            for x in 32..608 {
                for z in 32..608 {
                    world.set(x, 64, z, world::STONE);
                }
            }
        }
        if wall {
            for x in 200..440 {
                for y in 0..64 {
                    world.set(x, y, 128, world::STONE);
                }
            }
        }
        renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
        renderer.scene = SceneMesh::new(&renderer.device, world);
        renderer.shadow_ready = false;
        readings.push(center(&pixels(&mut renderer, &camera)));
    }
    println!(
        "Cave fog pixels: open sky {}, cave backdrop {}, cave wall {}",
        readings[0], readings[1], readings[2]
    );
    assert!(readings[0] > 40.0, "outdoor sky must remain visible");
    assert!(
        readings[1] < 2.0,
        "unloaded cave backdrop must be black even with night vision"
    );
    assert!(
        readings[2] < readings[0] * 0.45,
        "distant cave geometry must fade to black"
    );
}

#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn empty_air_regions_keep_the_outdoor_sky_visible() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Noise(Default::default()),
    ))
    .unwrap();
    renderer.hud_enabled = false;
    renderer.daylight.hours = 12.0;
    let position = renderer.world().spawn.unwrap() + Vec3::Y * 14.0;
    let camera = looking_at(position, position + Vec3::Y);
    assert!(
        renderer.scene.light_at(position).is_none(),
        "an empty air region has no mesh light brick"
    );
    assert!(
        renderer.scene.sky_light_at(position).is_some(),
        "the nearest loaded terrain supplies sky exposure"
    );
    assert!(
        center(&pixels(&mut renderer, &camera)) > 40.0,
        "outdoor air must retain the daytime sky"
    );
}
