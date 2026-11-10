use super::*;
use crate::{
    camera::Camera,
    scene_mesh::SceneMesh,
    world::{self, World},
};
use glam::Vec3;

fn looking_at(position: Vec3, target: Vec3) -> Camera {
    let direction = (target - position).normalize();
    Camera {
        position,
        yaw: direction.z.atan2(direction.x),
        pitch: direction.y.asin(),
        ..Default::default()
    }
}
fn pixels(renderer: &mut Renderer, camera: &Camera) -> Vec<u8> {
    let size = 512;
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow regression readback"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: renderer.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.render(
        &target.create_view(&Default::default()),
        camera,
        FrameInfo {
            size: (size, size),
            fps: 0.0,
            captured: false,
            vsync: false,
        },
    );
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shadow pixels"),
        size: (size * size * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    renderer.device.poll(wgpu::Maintain::Wait);
    rx.recv().unwrap().unwrap();
    let data = buffer.slice(..).get_mapped_range().to_vec();
    data
}
fn sample(data: &[u8], camera: &Camera, point: Vec3) -> Option<u8> {
    let clip = camera.matrix(1.0) * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * 512.0).floor() as i32;
    let y = ((-ndc.y * 0.5 + 0.5) * 512.0).floor() as i32;
    if !(4..508).contains(&x) || !(4..508).contains(&y) {
        return None;
    }
    Some(data[(y as usize * 512 + x as usize) * 4])
}
fn scene(renderer: &mut Renderer, world: World) {
    renderer.scene = SceneMesh::new(&renderer.device, world);
    renderer.shadow_ready = false;
}

/// Validate actual shadow comparisons, without albedo, sunlight or AO hiding acne.
#[test]
#[ignore = "requires a Vulkan/DirectX-capable GPU or software Vulkan adapter"]
fn shadow_receivers_are_lit_at_multiple_angles_and_real_occluders_still_cast() {
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(&instance, None)).unwrap();
    renderer.hud_enabled = false;
    renderer.ao_enabled = false;
    renderer.shadow_debug = true;
    let camera = crate::capture_views::shadow_view("house").unwrap();
    let data = pixels(&mut renderer, &camera);
    let base = world::terrain_height(348, 445) + 2;
    let sun = Vec3::new(-0.55, 0.78, 0.30).normalize();
    let mut checked = 0;
    let mut mismatch = 0;
    for x in (320..377).step_by(2) {
        for y in (base + 12..base + 68).step_by(2) {
            let z = if y >= base + 43 { 472 } else { 473 };
            if renderer.scene.world.get(x, y, z) == world::AIR
                || renderer.scene.world.get(x, y, z + 1) != world::AIR
            {
                continue;
            }
            let point = World::meters(x, y, z + 1) + Vec3::new(0.03125, 0.03125, 0.0);
            let ray = (point - camera.position).normalize();
            let Some(hit) = crate::editing::raycast(
                &renderer.scene.world,
                camera.position,
                ray,
                camera.position.distance(point) + 0.02,
            ) else {
                continue;
            };
            if hit.cell != [x, y, z] {
                continue;
            }
            let mut reference = None;
            let mut boundary = false;
            for dx in [-0.25, 0.0, 0.25] {
                for dy in [-0.25, 0.0, 0.25] {
                    let origin = point + Vec3::new(dx, dy, 0.004);
                    let blocked =
                        crate::editing::raycast(&renderer.scene.world, origin, sun, 80.0).is_some();
                    if reference.is_some_and(|value| value != blocked) {
                        boundary = true;
                    }
                    reference = Some(blocked);
                }
            }
            if boundary {
                continue;
            }
            for dx in [-0.02, 0.0, 0.02] {
                for dy in [-0.02, 0.0, 0.02] {
                    let Some(value) = sample(&data, &camera, point + Vec3::new(dx, dy, 0.0)) else {
                        continue;
                    };
                    checked += 1;
                    if (reference == Some(true) && value > 30)
                        || (reference == Some(false) && value < 245)
                    {
                        if mismatch < 10 {
                            println!(
                                "house mismatch {x},{y},{z}: CPU {:?}, GPU {value}",
                                reference
                            );
                        }
                        mismatch += 1;
                    }
                }
            }
        }
    }
    println!("House shadow CPU reference: {mismatch}/{checked} mismatches");
    assert!(checked > 100);
    assert_eq!(
        mismatch, 0,
        "shadow map disagrees with stable CPU ray-cast samples"
    );
    for axis in 0..3 {
        let mut world = World::empty();
        let (normal, center, u, v) = match axis {
            0 => {
                for y in 20..60 {
                    for z in 280..360 {
                        world.set(310, y, z, world::STONE);
                    }
                }
                (-Vec3::X, World::meters(310, 40, 320), Vec3::Y, Vec3::Z)
            }
            1 => {
                for x in 280..360 {
                    for z in 280..360 {
                        world.set(x, 24, z, world::STONE);
                    }
                }
                (Vec3::Y, World::meters(320, 25, 320), Vec3::X, Vec3::Z)
            }
            _ => {
                for x in 280..360 {
                    for y in 20..60 {
                        world.set(x, y, 330, world::STONE);
                    }
                }
                (Vec3::Z, World::meters(320, 40, 331), Vec3::X, Vec3::Y)
            }
        };
        scene(&mut renderer, world);
        for distance in [0.35, 4.0] {
            let camera = looking_at(
                center + normal * distance + Vec3::new(0.13, 0.17, 0.11),
                center,
            );
            let data = pixels(&mut renderer, &camera);
            let mut checked = 0;
            for x in -8..=8 {
                for y in -8..=8 {
                    let point = center + u * x as f32 * 0.05 + v * y as f32 * 0.05;
                    if let Some(value) = sample(&data, &camera, point) {
                        assert!(value>=250,"self-shadow acne on axis {axis}, camera distance {distance}, sample ({x},{y}): {value}");
                        checked += 1;
                    }
                }
            }
            assert!(checked > 25);
        }
    }
    let mut world = World::empty();
    for x in 280..360 {
        for z in 280..360 {
            world.set(x, 24, z, world::STONE);
        }
    }
    for x in 310..330 {
        for y in 25..55 {
            for z in 310..330 {
                world.set(x, y, z, world::STONE);
            }
        }
    }
    scene(&mut renderer, world);
    for (point, shadowed) in [
        (
            World::meters(340, 25, 310) + Vec3::new(0.03, 0.0, 0.03),
            true,
        ),
        (
            World::meters(350, 25, 340) + Vec3::new(0.03, 0.0, 0.03),
            false,
        ),
    ] {
        let camera = looking_at(point + Vec3::new(0.0, 3.0, -2.0), point);
        let data = pixels(&mut renderer, &camera);
        let value = sample(&data, &camera, point).unwrap();
        if shadowed {
            assert!(value < 30, "real blocker lost its shadow: {value}");
        } else {
            assert!(value >= 250, "unoccluded receiver is shadowed: {value}");
        }
    }
}
