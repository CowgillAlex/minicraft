use super::shadow_tests::{looking_at, pixels};
use super::*;
use crate::{
    view_settings::Change,
    world::{self, World},
};
use glam::Vec3;

fn difference(a: &[u8], b: &[u8]) -> f32 {
    let mut sum = 0.0;
    for y in 240..272 {
        for x in 240..272 {
            for channel in 0..3 {
                let index = (y * 512 + x) * 4 + channel;
                sum += (f32::from(a[index]) - f32::from(b[index])).abs();
            }
        }
    }
    sum / (32 * 32 * 3) as f32
}

#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn fog_and_distance_controls_change_pixels_and_stream_at_a_stationary_camera() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    let mut world = World::with_height(16 * 16);
    for x in 256..384 {
        for y in 0..96 {
            world.set(x, y, 32, world::STONE);
        }
    }
    renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.hud_enabled = false;
    renderer.night_vision = true;
    renderer.daylight.hours = 12.0;
    renderer.view.set_distance(48).unwrap();
    renderer.view.fog = false;
    let camera = looking_at(Vec3::new(0.0, 3.0, 18.0), Vec3::new(0.0, 3.0, -18.0));
    renderer.finish_meshes(&camera).unwrap();
    let clear = pixels(&mut renderer, &camera);
    renderer.view.change(Change::Fog);
    renderer.view.change(Change::Start(-127));
    let foggy = pixels(&mut renderer, &camera);
    renderer.view.change(Change::Density(-127));
    let light_fog = pixels(&mut renderer, &camera);
    assert!(
        difference(&clear, &foggy) > 15.0,
        "fog must visibly affect distant terrain"
    );
    assert!(difference(&clear, &light_fog) < difference(&clear, &foggy) * 0.7);
    renderer.view.fog = false;
    renderer.view.set_distance(12).unwrap();
    renderer.finish_meshes(&camera).unwrap();
    let clipped = pixels(&mut renderer, &camera);
    assert!(
        difference(&clear, &clipped) > 15.0,
        "shortening distance must remove the far wall"
    );
    renderer.view.set_distance(48).unwrap();
    renderer.finish_meshes(&camera).unwrap();
    let restored = pixels(&mut renderer, &camera);
    assert!(
        difference(&clear, &restored) < 1.0,
        "expanding distance must reload terrain without camera motion"
    );
}
