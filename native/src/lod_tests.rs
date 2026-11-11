use super::{
    shadow_tests::{looking_at, pixels},
    *,
};
use crate::{
    terrain_lod::Level,
    world::{self, World},
};
use glam::Vec3;
#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn distant_lod_keeps_the_rock_arch_open_and_materials_visible() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        crate::worldgen::preview::Scene::Village,
    ))
    .unwrap();
    let mut world = World::empty();
    for x in 192_i32..448 {
        for z in 192_i32..448 {
            let height = 16 + (x - 320).abs() / 3 + (z - 320).abs() / 4;
            for y in 0..height {
                world.set(
                    x,
                    y,
                    z,
                    if y >= height - 16 {
                        world::GRASS
                    } else {
                        world::STONE
                    },
                );
            }
        }
    }
    for z in 268..352 {
        for x in 268..372 {
            for y in 32..108 {
                if !(280..360).contains(&x) || y >= 100 {
                    world.set(x, y, z, world::STONE);
                }
            }
        }
    }
    renderer.lighting = LightVolume::new(&renderer.device, &renderer.queue, &world);
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.hud_enabled = false;
    renderer.night_vision = true;
    renderer.view.fog = false;
    renderer.view.set_distance(64).unwrap();
    renderer.daylight.hours = 12.0;
    let camera = looking_at(Vec3::new(0.0, 7.0, 32.0), Vec3::new(0.0, 3.5, 0.0));
    let mut baseline = Vec::new();
    let mut original = 0;
    for level in [Level::Full, Level::Medium, Level::Far] {
        renderer.scene.test_lod(&renderer.device, level);
        renderer.shadow_ready = false;
        let data = pixels(&mut renderer, &camera);
        let path = std::env::temp_dir().join(format!("minicraft-lod-{level:?}.png"));
        let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&data)
            .unwrap();
        if level == Level::Full {
            baseline = data.clone();
            original = renderer.visible_triangles;
        } else {
            assert!(
                renderer.visible_triangles < original * 9 / 10,
                "distant detail must reduce the submitted geometry"
            );
            let difference = data
                .as_chunks::<4>()
                .0
                .iter()
                .zip(baseline.as_chunks::<4>().0.iter())
                .map(|(a, b)| {
                    (0..3)
                        .map(|c| (a[c] as f32 - b[c] as f32).abs())
                        .sum::<f32>()
                })
                .sum::<f32>()
                / (512.0 * 512.0 * 3.0);
            println!(
                "LOD {level:?}: {} triangles, image mean difference {difference:.3}",
                renderer.visible_triangles
            );
            assert!(difference < 12.0, "LOD must preserve the overall image");
        }
        let mut blue = 0_i32;
        for y in 228..240 {
            for x in 250..262 {
                let p = &data[(x + y * 512) * 4..];
                blue += p[2] as i32 - p[0] as i32;
            }
        }
        assert!(
            blue > 144 * 15,
            "the sky must remain visible through the rock arch"
        );
        let greens = (230..280)
            .flat_map(|y| (120..390).map(move |x| (x + y * 512) * 4))
            .filter(|&i| {
                data[i + 1] as i32 - data[i] as i32 > 10
                    && data[i + 1] as i32 - data[i + 2] as i32 > 10
            })
            .count();
        assert!(
            greens > 50,
            "grass color must remain visible in distant terrain"
        );
    }
}
