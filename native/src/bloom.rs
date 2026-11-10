/// A small quarter-resolution bright-pass glow. Five filtered taps per blur axis.
pub struct Bloom {
    layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    down: wgpu::RenderPipeline,
    horizontal: wgpu::RenderPipeline,
    vertical: wgpu::RenderPipeline,
    targets: Option<Targets>,
}
struct Targets {
    first: wgpu::TextureView,
    second: wgpu::TextureView,
    source: wgpu::BindGroup,
    first_input: wgpu::BindGroup,
    second_input: wgpu::BindGroup,
}
impl Bloom {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom linear filter"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("soft lamp glow"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bloom.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_fullscreen",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: entry,
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
            })
        };
        Self {
            down: pipeline("fs_down"),
            horizontal: pipeline("fs_horizontal"),
            vertical: pipeline("fs_vertical"),
            layout,
            sampler,
            targets: None,
        }
    }
    pub fn resize(&mut self, device: &wgpu::Device, source: &wgpu::TextureView, size: (u32, u32)) {
        let texture = |label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0.div_ceil(4),
                        height: size.1.div_ceil(4),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let first = texture("bloom bright pass");
        let second = texture("bloom blur");
        let group = |view| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom source"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let targets = Targets {
            source: group(source),
            first_input: group(&first),
            second_input: group(&second),
            first,
            second,
        };
        self.targets = Some(targets);
    }
    pub fn output(&self) -> &wgpu::TextureView {
        &self.targets.as_ref().expect("sized bloom").first
    }
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder) {
        let targets = self.targets.as_ref().expect("sized bloom");
        for (view, pipeline, source) in [
            (&targets.first, &self.down, &targets.source),
            (&targets.second, &self.horizontal, &targets.first_input),
            (&targets.first, &self.vertical, &targets.second_input),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lamp bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, source, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
