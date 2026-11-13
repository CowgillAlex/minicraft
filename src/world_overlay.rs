//! Camera-facing, world-anchored debug labels, drawn at native resolution.
use crate::{
    chunk_timings::Timing,
    hud::Hud,
    ui::{Layout, Rect},
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
pub struct Label {
    pub key: [i32; 3],
    pub anchor: Vec3,
    pub timing: Timing,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
    uv: [f32; 2],
    anchor_uv: [f32; 2],
}
const CAPACITY: u64 = 1024 * 1024;
pub struct WorldOverlay {
    pipeline: wgpu::RenderPipeline,
    depth_layout: wgpu::BindGroupLayout,
    depth_group: Option<wgpu::BindGroup>,
    depth_size: (u32, u32),
    vertices: wgpu::Buffer,
    pub labels_drawn: usize,
}
fn duration(ms: Option<f32>) -> String {
    ms.map_or_else(
        || "n/a".into(),
        |ms| {
            if ms < 1.0 {
                format!("{ms:.3} ms")
            } else {
                format!("{ms:.2} ms")
            }
        },
    )
}
fn geometry(labels: &[Label], matrix: Mat4, size: (u32, u32)) -> (Vec<Vertex>, usize) {
    let layout = Layout::new(size);
    let mut vertices = Vec::new();
    let mut placed: Vec<Rect> = Vec::new();
    for label in labels {
        let clip = matrix * label.anchor.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if !ndc.is_finite() || ndc.z <= 0.0 || ndc.z >= 1.0 {
            continue;
        }
        let x = (ndc.x * 0.5 + 0.5) * layout.width;
        let y = (0.5 - ndc.y * 0.5) * layout.height;
        let mut rows = vec![
            format!("Chunk {}, {}, {}", label.key[0], label.key[1], label.key[2]),
            format!("Gen {}", duration(label.timing.generation_ms)),
            if label.timing.mesh_cached {
                format!("Mesh cached / built {}", duration(label.timing.mesh_ms))
            } else {
                format!(
                    "Mesh {}{}",
                    duration(label.timing.mesh_ms),
                    if label.timing.source_cached {
                        " / source cache"
                    } else {
                        ""
                    }
                )
            },
        ];
        if let Some(l) = label.timing.pipeline {
            rows.push(format!(
                "R queue {:.0} / light wait {:.0} ms",
                l.queue_ms, l.light_queue_ms
            ));
            rows.push(format!(
                "R light {:.0} / publish {:.0} ms",
                l.light_ms, l.publish_ms
            ));
            rows.push(format!(
                "R ready {:.0} / upload CPU {:.1} ms",
                l.ready_ms, l.upload_ms
            ));
        }
        let width = rows
            .iter()
            .map(|s| Hud::text_width(s, 11.0, false))
            .fold(0.0_f32, f32::max)
            + 16.0;
        let rect = Rect {
            x: x - width / 2.0,
            y: y - (rows.len() as f32 * 15.0 + 11.0) / 2.0,
            w: width,
            h: rows.len() as f32 * 15.0 + 11.0,
        };
        if rect.x < 8.0
            || rect.x + rect.w > layout.width - 8.0
            || rect.y < 76.0
            || rect.y + rect.h > layout.height - 116.0
        {
            continue;
        }
        if placed.iter().any(|p| {
            rect.x < p.x + p.w + 8.0
                && rect.x + rect.w + 8.0 > p.x
                && rect.y < p.y + p.h + 6.0
                && rect.y + rect.h + 6.0 > p.y
        }) {
            continue;
        }
        let mut hud = Hud::new(layout.width, layout.height);
        hud.rect(rect.x, rect.y, rect.w, rect.h, [0.015, 0.025, 0.035, 0.88]);
        for (i, row) in rows.iter().enumerate() {
            hud.text(
                row,
                rect.x + 8.0,
                rect.y + 7.0 + i as f32 * 15.0,
                11.0,
                if i == 1 {
                    [0.65, 0.90, 0.65, 1.0]
                } else if i == 2 {
                    [0.4, 0.85, 0.95, 1.0]
                } else {
                    [0.94, 0.95, 0.93, 1.0]
                },
            );
        }
        if (vertices.len() + hud.vertices.len()) as u64 * std::mem::size_of::<Vertex>() as u64
            > CAPACITY
        {
            break;
        }
        vertices.extend(hud.vertices.into_iter().map(|v| Vertex {
            position: [v.position[0], v.position[1], ndc.z],
            color: v.color,
            uv: v.uv,
            anchor_uv: [ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5],
        }));
        placed.push(rect);
        if placed.len() == 16 {
            break;
        }
    }
    (vertices, placed.len())
}
impl WorldOverlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chunk label depth layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world chunk timing labels"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world_overlay.wgsl").into()),
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x4,2=>Float32x2,3=>Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world debug overlays"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&crate::hud::font_layout(device), &depth_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });
        Self {
            pipeline,
            depth_layout,
            depth_group: None,
            depth_size: (0, 0),
            vertices: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world timing label vertices"),
                size: CAPACITY,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            labels_drawn: 0,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        depth_size: (u32, u32),
        font: &wgpu::BindGroup,
        labels: &[Label],
        matrix: Mat4,
        size: (u32, u32),
    ) {
        let (vertices, count) = geometry(labels, matrix, size);
        self.labels_drawn = count;
        if vertices.is_empty() {
            return;
        }
        if self.depth_size != depth_size {
            self.depth_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("chunk label scene depth"),
                layout: &self.depth_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth),
                }],
            }));
            self.depth_size = depth_size;
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world chunk timing overlays"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, font, &[]);
        pass.set_bind_group(1, self.depth_group.as_ref().unwrap(), &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }
}
#[cfg(test)]
#[test]
fn labels_reject_behind_camera_and_overlaps_and_follow_camera_projection() {
    let m = Mat4::perspective_rh(1.0, 1.6, 0.03, 512.0);
    let label = |z| Label {
        key: [-1, 4, 5],
        anchor: Vec3::new(0.0, 0.0, z),
        timing: Timing {
            generation_ms: Some(2.0),
            mesh_ms: Some(1.0),
            ..Default::default()
        },
    };
    assert_eq!(geometry(&[label(2.0)], m, (1280, 800)).1, 0);
    let (vertices, count) = geometry(&[label(-3.0), label(-4.0)], m, (1280, 800));
    assert_eq!(count, 1);
    assert!(!vertices.is_empty());
    assert!(vertices
        .iter()
        .all(|v| v.position[2] > 0.0 && v.position[2] < 1.0));
}

#[cfg(test)]
#[test]
#[ignore = "requires a graphics adapter"]
fn gpu_labels_share_the_font_atlas_and_hide_behind_opaque_depth() {
    let instance = wgpu::Instance::default();
    let gpu = pollster::block_on(crate::gpu::Gpu::new(&instance, None)).unwrap();
    let device = &gpu.device;
    let queue = &gpu.queue;
    let mut overlay = WorldOverlay::new(device, gpu.format);
    let font_pipeline = crate::hud::pipeline(device, gpu.format);
    let font = crate::hud::font_group(device, queue, &font_pipeline).unwrap();
    let extent = wgpu::Extent3d {
        width: 256,
        height: 256,
        depth_or_array_layers: 1,
    };
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("label test output"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = color.create_view(&Default::default());
    let depth = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("label test opaque depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default());
    for (z, visible) in [(0.25, true), (0.75, false)] {
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear label test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.5),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        overlay.render(
            device,
            queue,
            &mut encoder,
            &view,
            &depth,
            (256, 256),
            &font,
            &[Label {
                key: [1, 2, 3],
                anchor: Vec3::new(0.0, 0.0, z),
                timing: Timing {
                    generation_ms: Some(1.23),
                    mesh_ms: Some(0.45),
                    ..Default::default()
                },
            }],
            Mat4::IDENTITY,
            (256, 256),
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("label test readback"),
            size: 256 * 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(1024),
                    rows_per_image: Some(256),
                },
            },
            extent,
        );
        queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let data = buffer.slice(..).get_mapped_range();
        let lit = data
            .chunks_exact(4)
            .filter(|p| p[..3].iter().any(|&v| v > 16))
            .count();
        assert_eq!(overlay.labels_drawn, 1);
        if visible {
            assert!(lit > 50, "front labels must render text");
        } else {
            assert_eq!(lit, 0, "labels behind opaque terrain must disappear");
        }
        drop(data);
        buffer.unmap();
    }
}
