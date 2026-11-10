use crate::{
    gpu::Gpu,
    hud::{Hud, HudVertex},
    renderer::Result,
    startup::{Progress, Stage},
    ui::Rect,
};
use std::sync::Arc;

/// Small UI renderer independent of terrain, lighting and world pipelines.
pub struct LoadingScreen {
    pub device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    font: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    pub presented: bool,
}

impl LoadingScreen {
    pub fn new(gpu: &Gpu) -> Result<Self> {
        let pipeline = crate::hud::pipeline(&gpu.device, gpu.format);
        let font = crate::hud::font_group(&gpu.device, &gpu.queue, &pipeline)?;
        let vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loading UI vertices"),
            size: 256_000,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            format: gpu.format,
            pipeline,
            font,
            vertices,
            presented: false,
        })
    }

    pub fn render(&self, view: &wgpu::TextureView, size: (u32, u32), progress: &Progress) {
        self.draw(view, &vertices(size, progress));
    }

    pub fn draw(&self, view: &wgpu::TextureView, vertices: &[HudVertex]) {
        self.queue
            .write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("loading frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("loading screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.006,
                            g: 0.009,
                            b: 0.012,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.font, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

fn vertices(size: (u32, u32), progress: &Progress) -> Vec<HudVertex> {
    let scale = (size.1 as f32 / 900.0).clamp(0.85, 1.4);
    let (width, height) = (size.0 as f32 / scale, size.1 as f32 / scale);
    let mut hud = Hud::new(width, height);
    let white = [0.94, 0.95, 0.93, 1.0];
    let muted = [0.57, 0.63, 0.64, 1.0];
    let mint = [0.64, 0.84, 0.69, 1.0];
    hud.styled_text("MiniCraft", 28.0, 26.0, 18.0, white, true);
    hud.text(
        &format!("v{}", env!("CARGO_PKG_VERSION")),
        28.0,
        52.0,
        11.0,
        muted,
    );
    let w = 440.0_f32.min(width - 80.0);
    let x = (width - w) / 2.0;
    let y = height / 2.0 - 58.0;
    let snapshot = progress.snapshot();
    let elapsed = progress.elapsed();
    let stages = [
        Stage::Terrain,
        Stage::Mesh,
        Stage::Lighting,
        Stage::Preparing,
    ];
    let current = stages.iter().position(|s| *s == snapshot.stage);
    let step = current
        .map(|i| format!("{} / 4", i + 1))
        .unwrap_or_default();
    hud.text(&step, x, y - 31.0, 11.0, muted);
    hud.styled_text(snapshot.stage.label(), x, y, 22.0, white, true);
    let bar = Rect {
        x,
        y: y + 44.0,
        w,
        h: 5.0,
    };
    hud.rounded_rect(bar, 2.5, [0.15, 0.19, 0.20, 1.0]);
    if snapshot.total > 0 {
        let ratio = snapshot.completed as f32 / snapshot.total as f32;
        if ratio > 0.0 {
            hud.rounded_rect(
                Rect {
                    w: w * ratio,
                    ..bar
                },
                2.5,
                mint,
            );
        }
        hud.text(
            &format!("{} / {} pages", snapshot.completed, snapshot.total),
            x,
            y + 66.0,
            12.0,
            muted,
        );
        let percent = format!("{:.0}%", ratio * 100.0);
        hud.text(
            &percent,
            x + w - Hud::text_width(&percent, 12.0, false),
            y + 66.0,
            12.0,
            muted,
        );
    } else {
        let t = (elapsed * 0.6).fract();
        let span = w * 0.2;
        hud.rounded_rect(
            Rect {
                x: x + (w - span) * (0.5 - 0.5 * (t * std::f32::consts::TAU).cos()),
                w: span,
                ..bar
            },
            2.5,
            mint,
        );
    }
    for (index, stage) in stages.iter().enumerate() {
        let active =
            current.is_some_and(|current| index <= current) || snapshot.stage == Stage::Ready;
        let label = match stage {
            Stage::Terrain => "Terrain",
            Stage::Mesh => "Meshes",
            Stage::Lighting => "Lighting",
            _ => "Ready",
        };
        hud.text(
            label,
            x + index as f32 * w / 4.0,
            y + 113.0,
            11.0,
            if active { mint } else { muted },
        );
    }
    hud.text("Esc to cancel", 28.0, height - 36.0, 12.0, muted);
    let duration = format!("{elapsed:.0}s elapsed");
    hud.text(
        &duration,
        width - 28.0 - Hud::text_width(&duration, 12.0, false),
        height - 36.0,
        12.0,
        muted,
    );
    // Ui coordinates are logical, but the vertex shader receives normalized positions.
    hud.vertices
}
