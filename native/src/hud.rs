use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Glyph {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    left: f32,
    top: f32,
    advance: f32,
}
const ATLAS: [f32; 2] = [768.0, 576.0];
const METRICS: &[u8] = include_bytes!("../assets/ui-font.bin");
const WHITE: [f32; 2] = [0.5 / ATLAS[0], 0.5 / ATLAS[1]];
fn glyph(c: char, bold: bool) -> Glyph {
    let code = if c.is_ascii() && !c.is_ascii_control() {
        c as usize
    } else {
        b'?' as usize
    };
    let i = (code - 32 + usize::from(bold) * 95) * std::mem::size_of::<Glyph>();
    bytemuck::pod_read_unaligned(&METRICS[i..i + std::mem::size_of::<Glyph>()])
}
pub struct Hud {
    pub vertices: Vec<HudVertex>,
    width: f32,
    height: f32,
}
impl Hud {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            vertices: Vec::with_capacity(12_000),
            width,
            height,
        }
    }
    fn vertex(&mut self, p: [f32; 2], color: [f32; 4], uv: [f32; 2]) {
        self.vertices.push(HudVertex {
            position: [
                p[0] / self.width * 2.0 - 1.0,
                1.0 - p[1] / self.height * 2.0,
            ],
            color,
            uv,
        });
    }
    pub fn polygon(&mut self, points: &[[f32; 2]], color: [f32; 4]) {
        for i in 1..points.len().saturating_sub(1) {
            for p in [points[0], points[i], points[i + 1]] {
                self.vertex(p, color, WHITE);
            }
        }
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.polygon(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h]], color);
    }
    pub fn rounded_rect(&mut self, rect: crate::ui::Rect, radius: f32, color: [f32; 4]) {
        let crate::ui::Rect { x, y, w, h } = rect;
        let r = radius.min(w / 2.0).min(h / 2.0);
        let mut points = Vec::with_capacity(24);
        for (cx, cy, start) in [
            (x + w - r, y + r, -90.0_f32),
            (x + w - r, y + h - r, 0.0),
            (x + r, y + h - r, 90.0),
            (x + r, y + r, 180.0),
        ] {
            for step in 0..=5 {
                let angle = (start + step as f32 * 18.0).to_radians();
                points.push([cx + r * angle.cos(), cy + r * angle.sin()]);
            }
        }
        self.polygon(&points, color);
    }
    pub fn text_width(text: &str, size: f32, bold: bool) -> f32 {
        text.chars()
            .map(|c| glyph(c, bold).advance * size / 32.0)
            .sum()
    }
    pub fn text(&mut self, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
        self.styled_text(text, x, y, size, color, false);
    }
    pub fn centered(&mut self, text: &str, x: f32, y: f32, size: f32, color: [f32; 4], bold: bool) {
        self.styled_text(
            text,
            x - Self::text_width(text, size, bold) / 2.0,
            y,
            size,
            color,
            bold,
        );
    }
    pub fn styled_text(
        &mut self,
        text: &str,
        mut x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
        bold: bool,
    ) {
        let scale = size / 32.0;
        for c in text.chars() {
            let g = glyph(c, bold);
            if g.w > 0.0 && g.h > 0.0 {
                let left = x + g.left * scale;
                let top = y + (g.top + 25.6) * scale;
                for (px, py, u, v) in [
                    (left, top, g.x, g.y),
                    (left, top + g.h * scale, g.x, g.y + g.h),
                    (left + g.w * scale, top + g.h * scale, g.x + g.w, g.y + g.h),
                    (left, top, g.x, g.y),
                    (left + g.w * scale, top + g.h * scale, g.x + g.w, g.y + g.h),
                    (left + g.w * scale, top, g.x + g.w, g.y),
                ] {
                    self.vertex([px, py], color, [u / ATLAS[0], v / ATLAS[1]]);
                }
            }
            x += g.advance * scale;
        }
    }
}
/// One immutable glyph atlas; text and solid UI geometry share one draw call.
pub fn font_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
) -> crate::renderer::Result<wgpu::BindGroup> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../assets/ui-font.png"
    )));
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels)?;
    let size = wgpu::Extent3d {
        width: info.width,
        height: info.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("embedded UI glyphs"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels[..info.buffer_size()],
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(info.width),
            rows_per_image: Some(info.height),
        },
        size,
    );
    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("UI font"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    }))
}
