use crate::{
    terrain_region::{self, Builder, Key, Region},
    world::World,
    worldgen::{sky::Sky, Generator},
};
use glam::Vec3;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
};

struct Request {
    position: Vec3,
    direction: Vec3,
    travel: Vec3,
    radius: f32,
    loaded: HashMap<Key, usize>,
    edits: Option<World>,
    cells: Vec<[i32; 3]>,
    revision: u64,
}
struct Shared {
    request: Mutex<Request>,
    wake: Condvar,
    epoch: AtomicU64,
    stop: AtomicBool,
    pending: AtomicUsize,
    waiting: AtomicUsize,
    full: AtomicBool,
}
pub enum Update {
    Region(Region),
    Evicted(Vec<Key>),
    Pages(Vec<(Key, crate::chunk::Chunk)>),
}
pub struct Stream {
    shared: Arc<Shared>,
    pub results: mpsc::Receiver<Result<Update, String>>,
    pub center: [i32; 3],
    pub radius: f32,
    pub direction: Vec3,
    pub revision: u64,
}
impl Stream {
    pub fn new(
        world: World,
        sky: Sky,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, usize>,
    ) -> Self {
        let shared = Arc::new(Shared {
            request: Mutex::new(Request {
                position,
                direction: Vec3::ZERO,
                travel: Vec3::ZERO,
                radius,
                loaded,
                edits: None,
                cells: Vec::new(),
                revision: 0,
            }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(1),
            stop: AtomicBool::new(false),
            pending: AtomicUsize::new(1),
            waiting: AtomicUsize::new(0),
            full: AtomicBool::new(false),
        });
        let work = shared.clone();
        let (sender, results) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("infinite-terrain".into())
            .spawn(move || {
                let failure = sender.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(world, sky, &work, sender)
                }));
                if let Err(error) = result {
                    let message = error
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "Terrain worker stopped".into());
                    let _ = failure.send(Err(message));
                }
            })
            .expect("terrain worker");
        Self {
            shared,
            results,
            center: crate::mesh_stream::center(position),
            radius,
            direction: Vec3::ZERO,
            revision: 0,
        }
    }
    pub fn request(
        &mut self,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, usize>,
        edits: Option<(&World, &[[i32; 3]])>,
    ) {
        let mut request = self.shared.request.lock().unwrap();
        request.travel = (position - request.position).normalize_or_zero();
        request.direction = self.direction;
        request.position = position;
        request.radius = radius;
        request.loaded = loaded;
        if let Some((world, cells)) = edits {
            self.revision += 1;
            request.edits = Some(world.clone());
            request.cells.extend_from_slice(cells);
        }
        request.revision = self.revision;
        self.center = crate::mesh_stream::center(position);
        self.radius = radius;
        self.shared.pending.store(1, Ordering::Relaxed);
        self.shared.full.store(false, Ordering::Relaxed);
        self.shared.epoch.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_one();
    }
    pub fn pending(&self) -> bool {
        self.shared.pending.load(Ordering::Relaxed) > 0
            || self.shared.waiting.load(Ordering::Relaxed) > 0
    }
    pub fn received(&self) {
        self.shared.waiting.fetch_sub(1, Ordering::Relaxed);
    }
    pub fn mark_full(&self) {
        self.shared.full.store(true, Ordering::Relaxed);
    }
    pub fn full(&self) -> bool {
        self.shared.full.load(Ordering::Relaxed)
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        let _guard = self
            .shared
            .request
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        self.shared.stop.store(true, Ordering::Release);
        self.shared.epoch.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_one();
    }
}
fn run(world: World, sky: Sky, shared: &Shared, sender: mpsc::SyncSender<Result<Update, String>>) {
    let generator = Generator::new(world.noise_preview.expect("procedural world").seed)
        .expect("world generator");
    let mut builder = Builder::new(world, &generator, sky);
    let mut epoch = 0;
    let mut revision = 0;
    let mut position = Vec3::ZERO;
    let mut ready = HashMap::new();
    let mut wanted = VecDeque::new();
    let mut recent = HashMap::<Key, (u64, u64, usize)>::new();
    let mut bytes = 0;
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        let current = shared.epoch.load(Ordering::Acquire);
        if current != epoch {
            let mut request = shared.request.lock().unwrap();
            epoch = shared.epoch.load(Ordering::Acquire);
            revision = request.revision;
            position = request.position;
            let radius = request.radius;
            let direction = request.direction;
            let travel = request.travel;
            ready = request.loaded.clone();
            recent.retain(|key, (e, r, _)| {
                *e >= epoch && *r == revision && terrain_region::visible(*key, position, radius)
            });
            for (key, (_, _, size)) in &recent {
                ready.insert(*key, *size);
            }
            bytes = ready.values().sum();
            if let Some(world) = request.edits.take() {
                builder.edit(&world, &request.cells);
                request.cells.clear();
            }
            drop(request);
            let evicted = builder.trim(position);
            if !evicted.is_empty() {
                shared.waiting.fetch_add(1, Ordering::Relaxed);
                if sender.send(Ok(Update::Evicted(evicted))).is_err() {
                    return;
                }
            }
            let pages = builder.collision(position, travel, &|| {
                shared.stop.load(Ordering::Acquire) || shared.epoch.load(Ordering::Acquire) != epoch
            });
            if !pages.is_empty() {
                shared.waiting.fetch_add(1, Ordering::Relaxed);
                if sender.send(Ok(Update::Pages(pages))).is_err() {
                    return;
                }
            }
            wanted =
                terrain_region::candidates(&builder.world, position, radius, direction, travel)
                    .into();
        }
        let next = std::iter::from_fn(|| wanted.pop_front()).find(|key| {
            !ready.contains_key(key)
                || (terrain_region::center(*key).distance(position) < 16.0
                    && !builder.has_near_data(*key))
        });
        let Some(key) = next else {
            shared.pending.store(0, Ordering::Relaxed);
            let mut request = shared.request.lock().unwrap();
            while shared.epoch.load(Ordering::Acquire) == epoch
                && !shared.stop.load(Ordering::Acquire)
            {
                request = shared.wake.wait(request).unwrap();
            }
            continue;
        };
        shared.pending.store(1, Ordering::Relaxed);
        if bytes >= terrain_region::MESH_BUDGET
            && !ready.contains_key(&key)
            && terrain_region::center(key).distance(position) > 16.0
        {
            shared.full.store(true, Ordering::Relaxed);
            continue;
        }
        let Some(region) = builder.build(key, revision, position, || {
            shared.stop.load(Ordering::Acquire)
                || (shared.epoch.load(Ordering::Acquire) != epoch && {
                    let request = shared.request.lock().unwrap();
                    request.revision != revision
                        || !terrain_region::visible(key, request.position, request.radius)
                })
        }) else {
            continue;
        };
        bytes -= ready.insert(key, region.bytes).unwrap_or(0);
        bytes += region.bytes;
        recent.insert(
            key,
            (shared.epoch.load(Ordering::Acquire), revision, region.bytes),
        );
        shared.waiting.fetch_add(1, Ordering::Relaxed);
        if sender.send(Ok(Update::Region(region))).is_err() {
            return;
        }
    }
}
