use crate::{
    camera::{Camera, Frustum},
    daylight::Daylight,
    door_gpu::DoorGpu,
    editing::{self},
    light_volume::LightVolume,
    occlusion::Occlusion,
    picking::{self, Target},
    pipelines::Pipelines,
    scene_mesh::SceneMesh,
    world::World,
};
use bytemuck::{Pod, Zeroable};

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
    daylight: [f32; 4],
    light_color: [f32; 4],
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
    lighting: LightVolume,
    pub daylight: Daylight,
    selection: wgpu::Buffer,
    target: Option<Target>,
    door_meshes: Vec<DoorGpu>,
    door_notice: Option<(&'static str, Instant)>,
    pub selected_material: u8,
    hud_buffer: wgpu::Buffer,
    hud_font: wgpu::BindGroup,
    pub ui: crate::ui::UiState,
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
        source: crate::worldgen::preview::Scene,
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
        let world = source.build()?;
        let voxel_count = world.voxels;
        println!(
            "{} voxels generated in {:.2}s. Meshing...",
            voxel_count,
            start.elapsed().as_secs_f32()
        );
        let scene = SceneMesh::new(&device, world);
        println!("World meshed in {:.2}s", start.elapsed().as_secs_f32());
        let lighting = LightVolume::new(&device, &queue, &scene.world);
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
        let pipelines = Pipelines::new(
            &device,
            format,
            &uniform_layout,
            &shadow_layout,
            &lighting.layout,
        );
        let occlusion = Occlusion::new(&device, format, &uniform_layout);
        let hud_font = crate::hud::font_group(&device, &queue, &pipelines.hud_pipeline)?;
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
            lighting,
            daylight: Daylight::default(),
            selection,
            target: None,
            door_meshes: Vec::new(),
            door_notice: None,
            selected_material: crate::world::STONE,
            hud_buffer,
            hud_font,
            ui: crate::ui::UiState::default(),
            hud_count: 0,

            shadow_ready: false,
            started: Instant::now(),
            visible_chunks: 0,
            visible_triangles: 0,
            hud_enabled: true,
            grid: false,
        })
    }
    pub fn world(&self) -> &World {
        &self.scene.world
    }
    pub fn target_cell(&self, camera: &Camera, place: bool) -> Option<[i32; 3]> {
        let target = picking::cast(&self.scene.world, &self.door_meshes, camera)?;
        let hit = target.hit(&self.scene.world);
        editing::edit_cell(hit, camera.position, place)
    }
    pub fn edit(&mut self, camera: &Camera, place: bool) -> bool {
        let Some(cell) = self.target_cell(camera, place) else {
            return false;
        };
        let material = if place {
            self.selected_material
        } else {
            crate::world::AIR
        };
        let Some(changes) = self.scene.edit(&self.device, cell, material) else {
            return false;
        };
        self.shadow_ready = false;
        self.door_notice = None;
        self.lighting.request(&self.scene.world, changes);
        true
    }
    pub fn toggle_local_time(&mut self) {
        self.daylight
            .set_local_time(!self.daylight.follows_local_time());
        if self.daylight.sync_clock() {
            self.shadow_ready = false;
        }
    }
    pub fn adjust_time(&mut self, hours: f32) {
        self.daylight.advance(hours);
        self.shadow_ready = false;
    }
    pub fn finish_lighting(&mut self) {
        self.lighting.wait(&self.queue);
    }
    pub fn pick_material(&mut self, camera: &Camera) {
        if let Some(target) = picking::cast(&self.scene.world, &self.door_meshes, camera) {
            self.selected_material = target.material(&self.scene.world);
        }
    }
    pub fn render(&mut self, view: &wgpu::TextureView, camera: &Camera, info: FrameInfo) {
        if self.daylight.sync_clock() {
            self.shadow_ready = false;
        }
        self.lighting.poll(&self.queue);
        self.sync_doors();
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
        let (sun, power, color, daylight) = self.daylight.parameters();
        let light_matrix = Daylight::light_matrix(sun);
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj: matrix.to_cols_array_2d(),
                inverse_view_proj: matrix.inverse().to_cols_array_2d(),
                light_view_proj: light_matrix.to_cols_array_2d(),
                camera: camera.position.extend(1.0).to_array(),
                sun: sun.extend(power).to_array(),
                daylight,
                light_color: color.extend(0.0).to_array(),
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
            picking::cast(&self.scene.world, &self.door_meshes, camera)
        } else {
            None
        };
        if let Some(target) = self.target {
            self.queue.write_buffer(
                &self.selection,
                0,
                bytemuck::cast_slice(&target.outline(&self.scene.world, &self.door_meshes)),
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
            for door in &self.door_meshes {
                door.draw(&mut pass, None, false);
            }
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
            pass.set_bind_group(2, &self.lighting.group, &[]);
            (self.visible_chunks, self.visible_triangles) =
                self.scene.draw(&mut pass, Some(&frustum));
            for door in &self.door_meshes {
                let triangles = door.draw(&mut pass, Some(&frustum), false);
                if triangles > 0 {
                    self.visible_chunks += 1;
                    self.visible_triangles += triangles;
                }
            }
        }
        self.occlusion
            .render(&mut encoder, view, &self.uniform_group, self.ao_enabled);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("transparent glass"),
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
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipelines.glass_pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_bind_group(1, &self.shadow_group, &[]);
            pass.set_bind_group(2, &self.lighting.group, &[]);
            let (chunks, triangles) = self.scene.draw_glass(&mut pass, &frustum, camera.position);
            self.visible_chunks += chunks;
            self.visible_triangles += triangles;
            for door in &self.door_meshes {
                let triangles = door.draw(&mut pass, Some(&frustum), true);
                if triangles > 0 {
                    self.visible_chunks += 1;
                    self.visible_triangles += triangles;
                }
            }
        }
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
            pass.set_bind_group(0, &self.hud_font, &[]);
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
        let time = self.daylight.label();
        let vertices = crate::overlay::vertices(
            camera,
            size,
            fps,
            captured,
            vsync,
            crate::overlay::OverlayStats {
                noise_preview: self.scene.world.noise_preview,
                voxels: self.scene.world.voxels,
                triangles: self.visible_triangles,
                chunks: self.visible_chunks,
                ao_enabled: self.ao_enabled,
                grid: self.grid,
                local_time: self.daylight.follows_local_time(),
                material: self.selected_material,
                target: self.target.map(|t| t.hit(&self.scene.world)),
                interaction: self.interaction_hint(),
                time: &time,
                lighting_pending: self.lighting.pending(),
            },
            &self.ui,
        );
        self.hud_count = vertices.len() as u32;
        self.queue
            .write_buffer(&self.hud_buffer, 0, bytemuck::cast_slice(&vertices));
    }
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod shadow_tests;

#[cfg(test)]
#[path = "water_tests.rs"]
mod water_tests;

#[path = "door_interaction.rs"]
mod door_interaction;

#[path = "door_validation.rs"]
mod door_validation;
