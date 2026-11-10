use crate::gpu_vertex::GpuVertex;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub struct Pipelines {
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub selection_pipeline: wgpu::RenderPipeline,
    pub world_pipeline: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub hud_pipeline: wgpu::RenderPipeline,
}
impl Pipelines {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        uniform_layout: &wgpu::BindGroupLayout,
        shadow_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
        water_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world pipeline layout"),
            bind_group_layouts: &[uniform_layout, shadow_layout, lighting_layout],
            push_constant_ranges: &[],
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow pipeline layout"),
                bind_group_layouts: &[uniform_layout],
                push_constant_ranges: &[],
            });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Snorm8x4,2=>Unorm8x4,3=>Unorm8x4,5=>Unorm16x2];
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        };
        let target = Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let targets = [
            target,
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ];
        let world_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel surface"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_world",
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_world",
                targets: &targets,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true)),
            multisample: Default::default(),
            multiview: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sun shadows"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_shadow",
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
                ..depth_state(true)
            }),
            multisample: Default::default(),
            multiview: None,
        });
        let transparent_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water and glass layout"),
            bind_group_layouts: &[uniform_layout, shadow_layout, lighting_layout, water_layout],
            push_constant_ranges: &[],
        });
        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("translucent water and glass"),
            layout: Some(&transparent_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_world",
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_transparent",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(false)),
            multisample: Default::default(),
            multiview: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_sky",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_sky",
                targets: &targets,
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(depth_state(false)),
            multisample: Default::default(),
            multiview: None,
        });
        let hud_pipeline = crate::hud::pipeline(device, format);
        let selection_attributes = wgpu::vertex_attr_array![0=>Float32x3];
        let selection_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel selection outline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_selection",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &selection_attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_selection",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(false)),
            multisample: Default::default(),
            multiview: None,
        });
        Self {
            transparent_pipeline,
            selection_pipeline,
            world_pipeline,
            shadow_pipeline,
            sky_pipeline,
            hud_pipeline,
        }
    }
}
fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH,
        depth_write_enabled: write,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: Default::default(),
        bias: Default::default(),
    }
}
