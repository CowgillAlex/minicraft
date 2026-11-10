use crate::renderer::Result;
use std::sync::Arc;

/// One device shared by the loading screen and background renderer initialization.
#[derive(Clone)]
pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    pub adapter_name: String,
}

impl Gpu {
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
            .map(|surface| {
                let caps = surface.get_capabilities(&adapter);
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
        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            format,
            adapter_name,
        })
    }
}
