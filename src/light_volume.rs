use crate::{
    light_field::{LightField, LightPatch},
    world::World,
};
use std::{sync::mpsc, thread};

struct LightJob {
    revision: u64,
    world: World,
    edits: Vec<[i32; 3]>,
}

/// A worker retains the transport graph and repairs only light paths affected by edits.
pub struct LightVolume {
    pub layout: wgpu::BindGroupLayout,
    pub group: wgpu::BindGroup,
    texture: wgpu::Texture,
    jobs: Option<mpsc::Sender<LightJob>>,
    results: mpsc::Receiver<(u64, Option<LightPatch>)>,
    worker: Option<thread::JoinHandle<()>>,
    revision: u64,
    applied: u64,
}
impl LightVolume {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, world: &World) -> Self {
        let start = std::time::Instant::now();
        let mut field = if world.infinite {
            LightField::dummy()
        } else {
            LightField::build(world)
        };
        println!("Light field baked in {:.2}s", start.elapsed().as_secs_f32());
        let size = field.size();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("warm block light and skylight"),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload(queue, &texture, [0; 3], size, &field.pixels);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel light field layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smooth light volume"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = make_group(
            device,
            &layout,
            &texture,
            &sampler,
            [-19.96875, 0.03125, -19.96875],
            size.map(|v| v as f32 * 0.25),
            world.infinite,
        );
        let (tx, rx) = mpsc::channel::<LightJob>();
        let (result_tx, results) = mpsc::channel();
        let worker = if world.infinite {
            None
        } else {
            Some(thread::spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        job.edits.extend(newer.edits);
                        job.world = newer.world;
                        job.revision = newer.revision;
                    }
                    let patch = field.update(&job.world, &job.edits);
                    if result_tx.send((job.revision, patch)).is_err() {
                        break;
                    }
                }
            }))
        };
        Self {
            layout,
            group,
            texture,
            jobs: (!world.infinite).then_some(tx),
            results,
            worker,
            revision: 0,
            applied: 0,
        }
    }
    pub fn request(&mut self, world: &World, cells: Vec<[i32; 3]>) {
        if world.infinite {
            return;
        }
        self.revision += 1;
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(LightJob {
                revision: self.revision,
                world: world.clone(),
                edits: cells,
            });
        }
    }
    pub fn poll(&mut self, queue: &wgpu::Queue) {
        while let Ok((revision, patch)) = self.results.try_recv() {
            self.apply(queue, revision, patch);
        }
    }
    fn apply(&mut self, queue: &wgpu::Queue, revision: u64, patch: Option<LightPatch>) {
        // Patches are cumulative. Apply every completed revision in order, even
        // while newer edits are queued, so continuous editing cannot starve updates.
        if let Some(patch) = patch {
            upload(
                queue,
                &self.texture,
                patch.origin,
                patch.size,
                &patch.pixels,
            );
        }
        self.applied = revision;
    }
    pub fn pending(&self) -> bool {
        self.applied != self.revision
    }
    pub fn wait(&mut self, queue: &wgpu::Queue) {
        while self.pending() {
            match self.results.recv() {
                Ok((revision, patch)) => self.apply(queue, revision, patch),
                Err(_) => break,
            }
        }
    }
}
impl Drop for LightVolume {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn extent(size: [usize; 3]) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: size[0] as u32,
        height: size[1] as u32,
        depth_or_array_layers: size[2] as u32,
    }
}
fn upload(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    origin: [usize; 3],
    size: [usize; 3],
    pixels: &[u8],
) {
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: origin[0] as u32,
                y: origin[1] as u32,
                z: origin[2] as u32,
            },
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(size[0] as u32 * 2),
            rows_per_image: Some(size[1] as u32),
        },
        wgpu::Extent3d {
            width: size[0] as u32,
            height: size[1] as u32,
            depth_or_array_layers: size[2] as u32,
        },
    );
}

pub struct RegionLight {
    pub group: wgpu::BindGroup,
    texture: wgpu::Texture,
}
impl RegionLight {
    pub fn write(&self, queue: &wgpu::Queue, pixels: &[u8]) {
        upload(queue, &self.texture, [0; 3], [34; 3], pixels);
    }
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        origin: [i32; 3],
        pixels: &[u8],
    ) -> Self {
        let size = [34; 3];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("streamed regional light"),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload(queue, &texture, [0; 3], size, pixels);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("regional light filter"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let position =
            World::meters(origin[0] - 4, origin[1] - 4, origin[2] - 4) + glam::Vec3::splat(0.03125);
        Self {
            group: make_group(
                device,
                layout,
                &texture,
                &sampler,
                position.to_array(),
                [8.5; 3],
                true,
            ),
            texture,
        }
    }
}
fn make_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::Texture,
    sampler: &wgpu::Sampler,
    origin: [f32; 3],
    size: [f32; 3],
    infinite: bool,
) -> wgpu::BindGroup {
    use wgpu::util::DeviceExt;
    let params = [
        origin[0],
        origin[1],
        origin[2],
        f32::from(u8::from(infinite)),
        size[0],
        size[1],
        size[2],
        0.0,
    ];
    let transform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("light region transform"),
        contents: bytemuck::cast_slice(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("light region"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: transform.as_entire_binding(),
            },
        ],
    })
}
