use crate::renderer::{FrameInfo, Renderer, Result};
use std::{fs::File, io::BufWriter, path::Path, sync::mpsc, time::Instant};

/// Render through the same GPU pipelines without a desktop window, for validation.
pub struct CaptureOptions<'a> {
    pub scene: crate::worldgen::preview::Scene,
    pub frames: u32,
    pub ui: Option<&'a str>,
    pub size: Option<(u32, u32)>,
    pub verify_edits: bool,
    pub verify_doors: bool,
    pub door: Option<&'a str>,
    pub closeup: bool,
    pub ao_enabled: bool,
    pub night_vision: bool,
    pub transparency: bool,
    pub distance: Option<u16>,
    pub fog: bool,
    pub view: Option<&'a str>,
    pub hours: Option<f32>,
}
pub fn capture(path: &Path, options: CaptureOptions<'_>) -> Result<()> {
    let CaptureOptions {
        scene,
        frames,
        ui,
        size,
        verify_edits,
        verify_doors,
        door,
        closeup,
        ao_enabled,
        night_vision,
        transparency,
        distance,
        fog,
        view: shadow_view,
        hours,
    } = options;
    let instance = wgpu::Instance::default();
    let mut renderer = pollster::block_on(Renderer::new(&instance, None, scene))?;
    let (width, height) = size.unwrap_or((1280, 800));
    if !(320..=7680).contains(&width) || !(240..=4320).contains(&height) {
        return Err("Capture dimensions must be between 320x240 and 7680x4320".into());
    }
    let captured = match ui.unwrap_or("play") {
        "play" => true,
        "debug" => {
            renderer.ui.debug = true;
            true
        }
        "pause" => false,
        "controls" => {
            renderer.ui.panel = crate::ui::Panel::Controls;
            false
        }
        "view-settings" => {
            renderer.ui.panel = crate::ui::Panel::View;
            false
        }
        "settings" => {
            renderer.ui.panel = crate::ui::Panel::Settings;
            false
        }
        _ => {
            return Err(
                "--ui expects play, pause, controls, settings, view-settings, or debug".into(),
            )
        }
    };
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen capture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: renderer.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    if let Some(distance) = distance {
        renderer.view.set_distance(distance)?;
    }
    renderer.view.fog = fog;
    renderer.ao_enabled = ao_enabled;
    renderer.night_vision = night_vision;
    renderer.transparency_enabled = transparency;
    if let Some(hours) = hours {
        if !hours.is_finite() {
            return Err("--time must be finite".into());
        }
        renderer.daylight.hours = hours.rem_euclid(24.0);
    }
    let mut camera = shadow_view
        .map(crate::capture_views::shadow_view)
        .transpose()?
        .unwrap_or_default();
    if shadow_view.is_none() {
        if let Some(preview) = renderer.world().noise_preview {
            if preview.height_m > 12 {
                camera.position.y += (58 - preview.origin[1]) as f32;
            }
        }
    }
    if closeup {
        let (x, z) = (155, 205);
        let base = crate::world::terrain_height(x, z) + 2;
        camera.position = crate::world::World::meters(x - 27, base + 22, z + 38)
            + glam::Vec3::splat(crate::world::VOXEL * 0.5);
        // Reproduce a small hand-carved window-frame recess with the real editing path.
        for dx in -28..=-26 {
            for dy in 20..=23 {
                let target = crate::world::World::meters(x + dx, base + dy, z + 28)
                    + glam::Vec3::splat(crate::world::VOXEL * 0.5);
                let direction = (target - camera.position).normalize();
                camera.yaw = direction.z.atan2(direction.x);
                camera.pitch = direction.y.asin();
                for _ in 0..2 {
                    if !renderer.edit(&camera, false) {
                        return Err("Could not carve AO validation recess".into());
                    }
                }
            }
        }
        let target = crate::world::World::meters(x - 26, base + 22, z + 27)
            + glam::Vec3::splat(crate::world::VOXEL * 0.5);
        let direction = (target - camera.position).normalize();
        camera.yaw = direction.z.atan2(direction.x);
        camera.pitch = direction.y.asin();
    }
    if verify_doors {
        renderer.verify_doors()?;
    }
    if let Some(state) = door {
        renderer.capture_door_state(state)?;
    }
    if verify_edits {
        camera.position =
            crate::world::World::meters(80, 80, 80) + glam::Vec3::splat(crate::world::VOXEL * 0.5);
        camera.pitch = -1.54;
        camera.yaw = 0.0;
        renderer.pick_material(&camera);
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured: true,
                vsync: false,
            },
        );
        let before = renderer.visible_triangles;
        if !renderer.edit(&camera, false) {
            return Err("Break edit failed".into());
        }
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured: true,
                vsync: false,
            },
        );
        if !renderer.edit(&camera, true) {
            return Err("Placement edit failed".into());
        }
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured: true,
                vsync: false,
            },
        );
        if renderer.visible_triangles != before {
            return Err("Break/place round trip changed the restored mesh".into());
        }
        let selected = renderer.selected_material;
        renderer.selected_material = crate::world::LIGHT;
        if !renderer.edit(&camera, true) {
            return Err("Lamp placement failed".into());
        }
        renderer.finish_lighting();
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured: true,
                vsync: false,
            },
        );
        if !renderer.edit(&camera, false) {
            return Err("Lamp removal failed".into());
        }
        renderer.finish_lighting();
        renderer.selected_material = selected;
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured: true,
                vsync: false,
            },
        );
        if renderer.visible_triangles != before {
            return Err("Lamp round trip changed the restored mesh".into());
        }
        renderer.device.poll(wgpu::Maintain::Wait);
        println!("GPU edit verification passed: break, place, chunk replacement, shadow refresh, target outline and incremental lamp uploads");
    }
    renderer.finish_doors();
    renderer.finish_meshes(&camera)?;
    renderer.finish_lighting();
    // Warm the pipelines and shadow cache before measuring steady frames.
    renderer.render(
        &view,
        &camera,
        FrameInfo {
            size: (width, height),
            fps: 0.0,
            captured,
            vsync: false,
        },
    );
    renderer.device.poll(wgpu::Maintain::Wait);
    let start = Instant::now();
    for _ in 0..frames {
        renderer.render(
            &view,
            &camera,
            FrameInfo {
                size: (width, height),
                fps: 0.0,
                captured,
                vsync: false,
            },
        );
        renderer.device.poll(wgpu::Maintain::Wait);
    }
    println!(
        "Rendered {} frames in {:.3}s on {}. {} visible triangles / {} chunks",
        frames,
        start.elapsed().as_secs_f32(),
        renderer.adapter_name,
        renderer.visible_triangles,
        renderer.visible_chunks
    );
    let bytes_per_row = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: bytes_per_row as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit(Some(encoder.finish()));
    let (tx, rx) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    renderer.device.poll(wgpu::Maintain::Wait);
    rx.recv()??;
    let data = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in data.chunks_exact(bytes_per_row as usize) {
        pixels.extend_from_slice(&row[..(width * 4) as usize]);
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!("Saved {}", path.display());
    Ok(())
}
