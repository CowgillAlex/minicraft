use crate::{
    light_field::{LightField, LightPatch, SIZE},
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
        let mut field = LightField::build(world);
        println!("Light field baked in {:.2}s", start.elapsed().as_secs_f32());
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("warm block light and skylight"),
            size: extent(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload(queue, &texture, [0; 3], SIZE, &field.pixels);
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
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smooth light volume"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light field"),
            layout: &layout,
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
        });
        let (tx, rx) = mpsc::channel::<LightJob>();
        let (result_tx, results) = mpsc::channel();
        let worker = thread::spawn(move || {
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
        });
        Self {
            layout,
            group,
            texture,
            jobs: Some(tx),
            results,
            worker: Some(worker),
            revision: 0,
            applied: 0,
        }
    }
    pub fn request(&mut self, world: &World, cell: [i32; 3]) {
        self.revision += 1;
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(LightJob {
                revision: self.revision,
                world: world.clone(),
                edits: vec![cell],
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
fn extent() -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: SIZE[0] as u32,
        height: SIZE[1] as u32,
        depth_or_array_layers: SIZE[2] as u32,
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
