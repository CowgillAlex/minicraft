//! Optional GPU timestamps with a bounded, nonblocking readback ring.
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
pub const LABELS: [&str; 8] = [
    "Shadow",
    "World + sky",
    "SSAO",
    "Bloom",
    "Composite",
    "Water + target",
    "Upscale",
    "HUD + overlays",
];
const COUNT: u32 = 9;
struct Slot {
    readback: wgpu::Buffer,
    state: Arc<AtomicU8>,
}
pub struct Timings {
    queries: Option<wgpu::QuerySet>,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    active: Option<usize>,
    last_sample: Instant,
    period: f32,
    pub ms: Option<[f32; 8]>,
    pub profile: bool,
}
impl Timings {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let enabled = device.features().contains(
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        );
        Self {
            queries: enabled.then(|| {
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("GPU frame stages"),
                    ty: wgpu::QueryType::Timestamp,
                    count: COUNT * 3,
                })
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU timing resolve"),
                size: 256 * 3,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            slots: (0..3)
                .map(|_| Slot {
                    readback: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("GPU timing readback"),
                        size: COUNT as u64 * 8,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    state: Arc::new(AtomicU8::new(0)),
                })
                .collect(),
            active: None,
            last_sample: Instant::now() - Duration::from_secs(1),
            period: queue.get_timestamp_period(),
            ms: None,
            profile: std::env::var_os("MINICRAFT_GPU_PROFILE").is_some(),
        }
    }
    pub fn supported(&self) -> bool {
        self.queries.is_some()
    }
    pub fn begin(&mut self, device: &wgpu::Device, enabled: bool) {
        self.active = None;
        if self.queries.is_none() {
            return;
        }
        if !enabled
            && self
                .slots
                .iter()
                .all(|s| s.state.load(Ordering::Acquire) == 0)
        {
            return;
        }
        device.poll(wgpu::Maintain::Poll);
        for slot in &self.slots {
            match slot.state.load(Ordering::Acquire) {
                2 => {
                    let data = slot.readback.slice(..).get_mapped_range();
                    let stamps: Vec<u64> = data
                        .chunks_exact(8)
                        .map(bytemuck::pod_read_unaligned)
                        .collect();
                    let ms = std::array::from_fn(|i| {
                        stamps[i + 1].saturating_sub(stamps[i]) as f32 * self.period / 1_000_000.0
                    });
                    drop(data);
                    slot.readback.unmap();
                    self.ms = Some(ms);
                    slot.state.store(0, Ordering::Release);
                }
                3 => {
                    slot.readback.unmap();
                    slot.state.store(0, Ordering::Release);
                }
                _ => {}
            }
        }
        if enabled && self.last_sample.elapsed() >= Duration::from_millis(100) {
            self.active = self
                .slots
                .iter()
                .position(|s| s.state.load(Ordering::Acquire) == 0);
            if self.active.is_some() {
                self.last_sample = Instant::now();
            }
        }
    }
    pub fn stamp(&self, encoder: &mut wgpu::CommandEncoder, boundary: u32) {
        if let (Some(queries), Some(slot)) = (&self.queries, self.active) {
            encoder.write_timestamp(queries, slot as u32 * COUNT + boundary);
        }
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        if let (Some(queries), Some(slot)) = (&self.queries, self.active) {
            let start = slot as u32 * COUNT;
            encoder.resolve_query_set(
                queries,
                start..start + COUNT,
                &self.resolve,
                slot as u64 * 256,
            );
            encoder.copy_buffer_to_buffer(
                &self.resolve,
                slot as u64 * 256,
                &self.slots[slot].readback,
                0,
                COUNT as u64 * 8,
            );
        }
    }
    pub fn submitted(&self) {
        if let Some(index) = self.active {
            let slot = &self.slots[index];
            slot.state.store(1, Ordering::Release);
            let state = slot.state.clone();
            slot.readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    state.store(if result.is_ok() { 2 } else { 3 }, Ordering::Release)
                });
        }
    }
}
