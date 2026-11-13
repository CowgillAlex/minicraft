use super::shadow_tests::{looking_at, pixels};
use super::*;
use crate::{
    chunk::Chunk,
    world::{self, World},
};
use glam::Vec3;

#[test]
#[ignore = "requires a graphics adapter"]
fn graphics_quality_changes_and_current_view_occlusion_are_safe() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    renderer.hud_enabled = false;
    renderer.transparency_enabled = false;
    let mut world = World::empty();
    for x in 8..12 {
        for y in 0..4 {
            world.insert_page([x, y, 7], Chunk::Uniform(world::STONE));
            world.insert_page([x, y, 3], Chunk::Uniform(world::WOOD));
        }
    }
    renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.shadow_ready = false;
    let mut reduced = false;
    for eye in [
        Vec3::new(0.0, 3.0, 2.0),
        Vec3::new(5.0, 3.0, -3.0),
        Vec3::new(0.0, 3.0, -6.0),
    ] {
        let camera = looking_at(eye, Vec3::new(0.0, 3.0, -12.0));
        renderer.graphics.occlusion = false;
        let reference = pixels(&mut renderer, &camera);
        let triangles = renderer.visible_triangles;
        renderer.graphics.occlusion = true;
        let culled = pixels(&mut renderer, &camera);
        let differences = reference
            .iter()
            .zip(&culled)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differences == 0,
            "occlusion changed {differences} bytes at camera {:?}",
            eye
        );
        reduced |= renderer.visible_triangles < triangles;
    }
    assert!(reduced, "a solid wall must eliminate geometry behind it");
    let camera = looking_at(Vec3::new(0.0, 3.0, 2.0), Vec3::new(0.0, 3.0, -12.0));
    for (scale, ao, shadows, bloom) in [
        (75, 1, 2048, true),
        (50, 0, 1024, false),
        (100, 2, 4096, true),
    ] {
        renderer.graphics.scale = scale;
        renderer.graphics.ao = ao;
        renderer.graphics.shadows = shadows;
        renderer.graphics.bloom = bloom;
        let data = pixels(&mut renderer, &camera);
        assert!(data
            .chunks_exact(4)
            .any(|p| p[0] > 10 || p[1] > 10 || p[2] > 10));
    }
    renderer.timings.profile = true;
    for _ in 0..4 {
        let _ = pixels(&mut renderer, &camera);
    }
    if renderer.timings.supported() {
        let times = renderer
            .timings
            .ms
            .expect("asynchronous GPU readback completes on subsequent frames");
        assert!(times.iter().all(|ms| ms.is_finite() && *ms >= 0.0));
        assert!(times.iter().sum::<f32>() > 0.0);
    }
}
