/// Read-only opaque depth lets translucent water measure how much water covers the floor.
pub struct WaterDepth {
    pub layout: wgpu::BindGroupLayout,
    pub group: Option<wgpu::BindGroup>,
    size: (u32, u32),
}

impl WaterDepth {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("water floor depth"),
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
            }),
            group: None,
            size: (0, 0),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, depth: &wgpu::TextureView, size: (u32, u32)) {
        if size == self.size {
            return;
        }
        self.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water opaque depth input"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth),
            }],
        }));
        self.size = size;
    }
}
