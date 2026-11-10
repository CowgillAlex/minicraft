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
