use crate::{
    camera::{Camera, Frustum},
    editing::{self, Hit},
    hud::Hud,
    occlusion::Occlusion,
    pipelines::Pipelines,
    scene_mesh::SceneMesh,
    world::World,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::{error::Error, time::Instant};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    inverse_view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    sun: [f32; 4],
    options: [f32; 4],
}
pub struct FrameInfo {
    pub size: (u32, u32),
    pub fps: f32,
    pub captured: bool,
    pub vsync: bool,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub format: wgpu::TextureFormat,
    pipelines: Pipelines,
    occlusion: Occlusion,
    pub ao_enabled: bool,
    shadow_debug: bool,
    uniform: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    shadow_group: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    scene: SceneMesh,
    selection: wgpu::Buffer,
    target: Option<Hit>,
    pub selected_material: u8,
    hud_buffer: wgpu::Buffer,
    hud_count: u32,

    shadow_ready: bool,
    started: Instant,
    pub visible_chunks: usize,
    pub visible_triangles: usize,
    pub hud_enabled: bool,
    pub grid: bool,
}
impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No compatible GPU found. Update your graphics driver.")?;
        let format = surface
            .map(|s| {
                let caps = s.get_capabilities(&adapter);
                caps.formats
                    .iter()
                    .copied()
                    .find(|f| f.is_srgb())
                    .unwrap_or(caps.formats[0])
            })
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        let info = adapter.get_info();
        let adapter_name = format!("{} / {:?}", info.name, info.backend);
        println!("GPU: {adapter_name}");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("voxel device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;
        let start = Instant::now();
        println!("Generating 6.25 cm voxel world...");
        let world = World::generate();
        let voxel_count = world.voxels;
        println!(
            "{} voxels generated in {:.2}s. Meshing...",
            voxel_count,
            start.elapsed().as_secs_f32()
        );
        let scene = SceneMesh::new(&device, world);
        println!("World meshed in {:.2}s", start.elapsed().as_secs_f32());
        let selection = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("target voxel outline"),
            size: 24 * 12,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera and lighting"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shadow = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cached sun shadow map"),
            size: wgpu::Extent3d {
                width: 4096,
                height: 4096,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow group"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipelines = Pipelines::new(&device, format, &uniform_layout, &shadow_layout);
        let occlusion = Occlusion::new(&device, format, &uniform_layout);
        let hud_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HUD vertices"),
            size: 2_000_000,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            device,
            queue,
            adapter_name,
            format,
            pipelines,
            occlusion,
            ao_enabled: true,
            shadow_debug: false,
            uniform,
            uniform_group,
            shadow_group,
            shadow_view,
            scene,
            selection,
            target: None,
            selected_material: crate::world::STONE,
            hud_buffer,
            hud_count: 0,

            shadow_ready: false,
            started: Instant::now(),
            visible_chunks: 0,
            visible_triangles: 0,
            hud_enabled: true,
            grid: false,
        })
    }
    pub fn edit(&mut self, camera: &Camera, place: bool) -> bool {
        let Some(hit) = editing::raycast(
            &self.scene.world,
            camera.position,
            camera.direction(),
            editing::REACH,
        ) else {
            return false;
        };
        let Some(cell) = editing::edit_cell(hit, camera.position, place) else {
            return false;
        };
        let material = if place {
            self.selected_material
        } else {
            crate::world::AIR
        };
        let changed = self.scene.edit(&self.device, cell, material);
        if changed {
            self.shadow_ready = false;
        }
        changed
    }
    pub fn pick_material(&mut self, camera: &Camera) {
        if let Some(hit) = editing::raycast(
            &self.scene.world,
            camera.position,
            camera.direction(),
            editing::REACH,
        ) {
            self.selected_material = self.scene.world.get(hit.cell[0], hit.cell[1], hit.cell[2]);
        }
    }
    pub fn render(&mut self, view: &wgpu::TextureView, camera: &Camera, info: FrameInfo) {
        let FrameInfo {
            size: (width, height),
            fps,
            captured,
            vsync,
        } = info;
        self.occlusion
            .resize(&self.device, self.format, (width, height));
        let targets = self.occlusion.targets.as_ref().expect("sized AO targets");
        let matrix = camera.matrix(width as f32 / height as f32);
        let sun = Vec3::new(-0.55, 0.78, 0.30).normalize();
        let light_matrix = Mat4::orthographic_rh(-32.0, 32.0, -32.0, 32.0, 0.1, 130.0)
            * Mat4::look_at_rh(sun * 65.0, Vec3::ZERO, Vec3::Y);
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj: matrix.to_cols_array_2d(),
                inverse_view_proj: matrix.inverse().to_cols_array_2d(),
                light_view_proj: light_matrix.to_cols_array_2d(),
                camera: camera.position.extend(1.0).to_array(),
                sun: sun.extend(0.0).to_array(),
                options: [
                    self.started.elapsed().as_secs_f32(),
                    f32::from(u8::from(self.grid)),
                    f32::from(u8::from(self.ao_enabled)),
                    f32::from(u8::from(self.shadow_debug)),
                ],
            }),
        );
        let frustum = Frustum::new(matrix);
        self.target = if captured {
            editing::raycast(
                &self.scene.world,
                camera.position,
                camera.direction(),
                editing::REACH,
            )
        } else {
            None
        };
        if let Some(hit) = self.target {
            self.queue.write_buffer(
                &self.selection,
                0,
                bytemuck::cast_slice(&editing::outline(hit)),
            );
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        if !self.shadow_ready {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cached shadows"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipelines.shadow_pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            self.scene.draw(&mut pass, None);
            self.shadow_ready = true;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world and sky"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &targets.color,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &targets.normals,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_pipeline(&self.pipelines.sky_pipeline);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipelines.world_pipeline);
            pass.set_bind_group(1, &self.shadow_group, &[]);
            (self.visible_chunks, self.visible_triangles) =
                self.scene.draw(&mut pass, Some(&frustum));
        }
        self.occlusion
            .render(&mut encoder, view, &self.uniform_group, self.ao_enabled);
        if self.target.is_some() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("target after AO"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipelines.selection_pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_vertex_buffer(0, self.selection.slice(..));
            pass.draw(0..24, 0..1);
        }
        if self.hud_enabled {
            self.update_hud(camera, (width, height), fps, captured, vsync);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&self.pipelines.hud_pipeline);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(..));
            pass.draw(0..self.hud_count, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }
    fn update_hud(
        &mut self,
        camera: &Camera,
        size: (u32, u32),
        fps: f32,
        captured: bool,
        vsync: bool,
    ) {
        let mut hud = Hud::new(size.0, size.1);
        let text = [0.78, 0.87, 0.83, 1.0];
        let muted = [0.43, 0.58, 0.54, 1.0];
        let accent = [0.31, 0.84, 0.62, 1.0];
        hud.rect(16.0, 16.0, 350.0, 151.0, [0.015, 0.028, 0.027, 0.88]);
        hud.rect(16.0, 16.0, 3.0, 151.0, accent);
        hud.text("CEDAR / VOXEL LAB", 31.0, 29.0, 2.0, text);
        hud.text(
            &format!(
                "V{}  /  AO {}",
                env!("CARGO_PKG_VERSION"),
                if self.ao_enabled { "ON" } else { "OFF" }
            ),
            31.0,
            52.0,
            1.5,
            muted,
        );
        hud.text(
            &format!(
                "{:3.0} FPS   {:4.1} MS   {}",
                fps,
                if fps > 0.0 { 1000.0 / fps } else { 0.0 },
                if vsync { "VSYNC" } else { "UNCAPPED" }
            ),
            31.0,
            76.0,
            1.5,
            accent,
        );
        hud.text(
            &format!(
                "{:.2}M VOXELS  /  6.25 CM",
                self.scene.world.voxels as f32 / 1e6
            ),
            31.0,
            97.0,
            1.5,
            text,
        );
        hud.text(
            &format!(
                "{}K TRIS  /  {} CHUNKS",
                self.visible_triangles / 1000,
                self.visible_chunks
            ),
            31.0,
            116.0,
            1.5,
            text,
        );
        hud.text(
            &format!(
                "XYZ {:.1} {:.1} {:.1}",
                camera.position.x, camera.position.y, camera.position.z
            ),
            31.0,
            139.0,
            1.5,
            muted,
        );
        let palette_x = size.0 as f32 - 374.0;
        hud.rect(palette_x, 16.0, 358.0, 66.0, [0.015, 0.028, 0.027, 0.88]);
        hud.text(
            "MATERIAL / 1-0 SELECT / MMB PICK",
            palette_x + 12.0,
            26.0,
            1.0,
            muted,
        );
        for (index, material) in editing::PALETTE.iter().enumerate() {
            let x = palette_x + 13.0 + index as f32 * 33.0;
            if *material == self.selected_material {
                hud.rect(x - 2.0, 42.0, 28.0, 28.0, accent);
            }
            let color = crate::world::color(*material);
            hud.rect(x, 44.0, 24.0, 24.0, [color[0], color[1], color[2], 1.0]);
            hud.text(
                &((index + 1) % 10).to_string(),
                x + 3.0,
                47.0,
                1.0,
                [0.015, 0.028, 0.027, 1.0],
            );
        }
        let y = size.1 as f32 - 118.0;
        hud.rect(
            16.0,
            y,
            if size.0 < 1000 { 610.0 } else { 830.0 },
            102.0,
            [0.015, 0.028, 0.027, 0.87],
        );
        hud.text(
            if captured {
                "WASD MOVE / SPACE UP / SHIFT DOWN / CTRL BOOST"
            } else {
                "CLICK TO EXPLORE / SPACE UP / SHIFT DOWN / CTRL BOOST"
            },
            29.0,
            y + 12.0,
            1.5,
            text,
        );
        hud.text(
            "LMB BREAK / RMB PLACE / 1-0 MATERIAL / MMB PICK / SCROLL SPEED",
            29.0,
            y + 34.0,
            if size.0 < 1000 { 1.0 } else { 1.5 },
            muted,
        );
        hud.text(
            "ESC RELEASE / R RESET / H HUD / G GRID / O AO / V VSYNC / F11 FULLSCREEN",
            29.0,
            y + 55.0,
            if size.0 < 1000 { 1.0 } else { 1.5 },
            muted,
        );
        let material = format!(
            "PLACE {} / {}",
            editing::material_name(self.selected_material),
            self.target
                .map(|hit| format!(
                    "TARGET {} {} {} / {:.2}M",
                    hit.cell[0], hit.cell[1], hit.cell[2], hit.distance
                ))
                .unwrap_or_else(|| "REACH 8M / NO TARGET".into())
        );
        hud.text(&material, 29.0, y + 77.0, 1.5, accent);
        if captured {
            let x = size.0 as f32 / 2.0;
            let y = size.1 as f32 / 2.0;
            hud.rect(x - 4.0, y, 8.0, 1.0, [0.8, 0.95, 0.87, 0.7]);
            hud.rect(x, y - 4.0, 1.0, 8.0, [0.8, 0.95, 0.87, 0.7]);
        }
        self.hud_count = hud.vertices.len() as u32;
        self.queue
            .write_buffer(&self.hud_buffer, 0, bytemuck::cast_slice(&hud.vertices));
    }
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod shadow_tests;
