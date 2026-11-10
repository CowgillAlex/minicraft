use super::shadow_tests::{looking_at, pixels};
use super::*;
use crate::{
    light_volume::LightVolume,
    scene_mesh::SceneMesh,
    world::{self, World},
};
use glam::Vec3;
fn pond(covered: bool) -> World {
    let mut world = World::empty();
    for x in 250..390 {
        for z in 250..390 {
            world.set(x, 16, z, world::WATER);
        }
    }
    if covered {
        for x in 260..380 {
            for z in 260..380 {
                world.set(x, 44, z, world::STONE);
            }
        }
    }
    world
}
fn brightness(data: &[u8]) -> f32 {
    let mut sum = 0.0;
    for y in 232..280 {
        for x in 232..280 {
            let pixel = &data[(y * 512 + x) * 4..][..3];
            sum += pixel.iter().map(|&v| v as f32).sum::<f32>() / 3.0;
        }
    }
    sum / (48 * 48) as f32
}
#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn water_receives_shadows_in_daylight_and_at_night() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    renderer.hud_enabled = false;
    let camera = looking_at(Vec3::new(0.0, 1.8, 2.0), Vec3::new(0.0, 17.0 / 16.0, 0.0));
    for hour in [12.0, 3.0] {
        renderer.daylight.hours = hour;
        let mut readings = Vec::new();
        for covered in [false, true] {
            let world = pond(covered);
            renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
            renderer.scene = SceneMesh::new(&renderer.device, world);
            renderer.shadow_ready = false;
            readings.push(brightness(&pixels(&mut renderer, &camera)));
        }
        println!(
            "Water at {hour}: open {:.2}, covered {:.2}",
            readings[0], readings[1]
        );
        assert!(readings[0] > 4.0, "open water must remain visible");
        assert!(
            readings[1] < readings[0] * 0.6,
            "a roof must shade water, including reflected/ambient light"
        );
    }
}

#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn shallow_water_reveals_the_floor_and_depth_reduces_its_visibility() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    renderer.hud_enabled = false;
    renderer.daylight.hours = 12.0;
    let camera = looking_at(Vec3::new(0.0, 6.0, 0.01), Vec3::new(0.0, 49.0 / 16.0, 0.0));
    let mut contrasts = Vec::new();
    for bottom in [40, 8] {
        let mut colors = Vec::new();
        for material in [world::STONE, world::WOOD] {
            let mut world = World::empty();
            for x in 250..390 {
                for z in 250..390 {
                    world.set(x, bottom, z, material);
                    for y in bottom + 1..=48 {
                        world.set(x, y, z, world::WATER);
                    }
                }
            }
            renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
            renderer.scene = SceneMesh::new(&renderer.device, world);
            renderer.shadow_ready = false;
            colors.push(brightness(&pixels(&mut renderer, &camera)));
        }
        contrasts.push((colors[0] - colors[1]).abs());
    }
    println!(
        "Floor contrast through water: shallow {:.2}, deep {:.2}",
        contrasts[0], contrasts[1]
    );
    assert!(
        contrasts[0] > 15.0,
        "shallow water must show the bottom material"
    );
    assert!(
        contrasts[1] < contrasts[0] * 0.8,
        "deeper water must attenuate the bottom"
    );
}
