/// Half-resolution screen-space AO, followed by edge-aware reconstruction.
/// Color/depth/normal targets are recreated only when the drawable size changes.
pub struct Occlusion {
    bloom: crate::bloom::Bloom,
    ao_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    ao_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    pub targets: Option<Targets>,
}
pub struct Targets {
    pub size: (u32, u32),
    pub color: wgpu::TextureView,
    pub normals: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    ao: wgpu::TextureView,
    ao_group: wgpu::BindGroup,
    composite_group: wgpu::BindGroup,
}
impl Occlusion {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        uniform_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let mut entries: Vec<_> = (0..4)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: if binding == 0 {
                        wgpu::TextureSampleType::Depth
                    } else {
                        wgpu::TextureSampleType::Float { filterable: false }
                    },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let ao_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AO depth and normal inputs"),
            entries: &entries[..2],
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AO reconstruction inputs"),
            entries: &entries,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("screen space ambient occlusion"),
            source: wgpu::ShaderSource::Wgsl(include_str!("occlusion.wgsl").into()),
        });
        let pipeline = |label, entry, format, inputs| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[uniform_layout, inputs],
                push_constant_ranges: &[],
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
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
                        format,
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
        let ao_pipeline = pipeline(
            "half resolution AO",
            "fs_ao",
            wgpu::TextureFormat::R8Unorm,
            &ao_layout,
        );
        let composite_pipeline = pipeline(
            "bilateral AO composite",
            "fs_composite",
            format,
            &composite_layout,
        );
        Self {
            bloom: crate::bloom::Bloom::new(device),
            ao_pipeline,
            composite_pipeline,
            ao_layout,
            composite_layout,
            targets: None,
        }
    }
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        _format: wgpu::TextureFormat,
        size: (u32, u32),
    ) {
        if self
            .targets
            .as_ref()
            .is_some_and(|targets| targets.size == size)
        {
            return;
        }
        let texture = |label, width, height, format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture(
            "HDR scene color",
            size.0,
            size.1,
            wgpu::TextureFormat::Rgba16Float,
        );
        let normals = texture(
            "scene normals and ambient fraction",
            size.0,
            size.1,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let depth = texture(
            "sampleable scene depth",
            size.0,
            size.1,
            wgpu::TextureFormat::Depth32Float,
        );
        let ao = texture(
            "half resolution AO",
            size.0.div_ceil(2),
            size.1.div_ceil(2),
            wgpu::TextureFormat::R8Unorm,
        );
        self.bloom.resize(device, &color, size);
        let mut entries: Vec<_> = [&depth, &normals, &ao, &color, self.bloom.output()]
            .iter()
            .enumerate()
            .map(|(binding, view)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: wgpu::BindingResource::Sampler(&self.bloom.sampler),
        });
        let ao_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AO inputs"),
            layout: &self.ao_layout,
            entries: &entries[..2],
        });
        let composite_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("AO composite inputs"),
            layout: &self.composite_layout,
            entries: &entries,
        });
        self.targets = Some(Targets {
            size,
            color,
            normals,
            depth,
            ao,
            ao_group,
            composite_group,
        });
    }
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        uniforms: &wgpu::BindGroup,
        enabled: bool,
        bloom: bool,
        timings: &crate::gpu_timing::Timings,
    ) {
        let targets = self
            .targets
            .as_ref()
            .expect("AO targets must be sized before rendering");
        if enabled {
            fullscreen_pass(
                encoder,
                "ambient occlusion",
                &targets.ao,
                &self.ao_pipeline,
                uniforms,
                &targets.ao_group,
            );
        }
        timings.stamp(encoder, 3);
        if bloom {
            self.bloom.render(encoder);
        }
        timings.stamp(encoder, 4);
        fullscreen_pass(
            encoder,
            "ambient composite",
            output,
            &self.composite_pipeline,
            uniforms,
            &targets.composite_group,
        );
        timings.stamp(encoder, 5);
    }
}
fn fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    output: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    uniforms: &wgpu::BindGroup,
    inputs: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: output,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, uniforms, &[]);
    pass.set_bind_group(1, inputs, &[]);
    pass.draw(0..3, 0..1);
}
