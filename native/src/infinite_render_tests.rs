use super::shadow_tests::{looking_at, pixels};
use super::*;
use crate::{
    terrain_region,
    world::{self, World},
    worldgen::{
        preview::{Preview, Scene},
        Generator,
    },
};
use glam::Vec3;
#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn infinite_terrain_renders_past_both_old_edges_and_restores_session_edits() {
    let instance = wgpu::Instance::default();
    let preview = Preview {
        origin: [120, 20, -20],
        height_m: 64,
        ..Default::default()
    };
    let generator = Generator::new(preview.seed).unwrap();
    let mut sampler = generator.sampler();
    let mut renderer =
        pollster::block_on(Renderer::new(&instance, None, Scene::Noise(preview))).unwrap();
    renderer.view.set_distance(12).unwrap();
    renderer.view.fog = false;
    renderer.night_vision = true;
    renderer.hud_enabled = false;
    let mut lamp = None;
    for x in [740, -540, 740] {
        let height = crate::worldgen::sky::surface(preview, &mut sampler, x, 347);
        let position = World::meters(x, height, 347) + Vec3::Y * 2.5;
        let camera = looking_at(position, position + Vec3::new(1.0, -0.4, -0.7));
        let start = Instant::now();
        renderer.finish_meshes(&camera).unwrap();
        assert!(renderer.world().is_loaded([x, height - 1, 347]));
        assert_eq!(
            renderer.world().get_static(x, height - 1, 347),
            sampler.material(preview.point([x, height - 1, 347]))
        );
        let _ = pixels(&mut renderer, &camera);
        assert!(
            renderer.visible_triangles > 0,
            "terrain must render beyond the old boundary"
        );
        println!(
            "Streamed x={x} in {:.2}s, {:.1} MiB resident voxels, {} triangles",
            start.elapsed().as_secs_f64(),
            renderer.world().voxel_storage_bytes() as f64 / 1048576.0,
            renderer.visible_triangles
        );
        if lamp.is_none() {
            let cell = [x, height + 16, 347];
            let changes = renderer
                .scene
                .edit(&renderer.device, cell, world::LIGHT)
                .unwrap();
            renderer.scene.stream_edits(&changes);
            renderer.finish_meshes(&camera).unwrap();
            assert!(renderer
                .scene
                .light_at(World::meters(cell[0], cell[1], cell[2]))
                .is_some());
            lamp = Some(cell);
        }
        if x == 740 {
            let p = lamp.unwrap();
            assert_eq!(renderer.world().get_static(p[0], p[1], p[2]), world::LIGHT);
        }
        assert!(terrain_region::key(position)[0] < 0 || position.x > 20.0);
    }
}
