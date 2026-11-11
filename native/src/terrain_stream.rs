use crate::{
    terrain_budget::{self, Loaded, View},
    terrain_lod::Level,
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
    lod: bool,
    loaded: HashMap<Key, Loaded>,
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
    mesh_budget: usize,
}
pub enum Update {
    Region(Region),
    Evicted(Vec<Key>),
    MeshesEvicted { keys: Vec<Key>, epoch: u64 },
    Pages(Vec<(Key, crate::chunk::Chunk)>),
}
pub struct Stream {
    shared: Arc<Shared>,
    pub results: mpsc::Receiver<Result<Update, String>>,
    pub center: [i32; 3],
    pub radius: f32,
    pub lod: bool,
    pub direction: Vec3,
    pub revision: u64,
}
impl Stream {
    pub fn new(
        world: World,
        sky: Sky,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, Loaded>,
    ) -> Self {
        Self::with_budget(
            world,
            sky,
            position,
            radius,
            loaded,
            terrain_region::MESH_BUDGET,
        )
    }
    fn with_budget(
        world: World,
        sky: Sky,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, Loaded>,
        mesh_budget: usize,
    ) -> Self {
        let shared = Arc::new(Shared {
            request: Mutex::new(Request {
                position,
                direction: Vec3::ZERO,
                travel: Vec3::ZERO,
                radius,
                lod: true,
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
            mesh_budget,
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
            lod: true,
            direction: Vec3::ZERO,
            revision: 0,
        }
    }
    pub fn request(
        &mut self,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, Loaded>,
        edits: Option<(&World, &[[i32; 3]])>,
    ) {
        let mut request = self.shared.request.lock().unwrap();
        request.travel = (position - request.position).normalize_or_zero();
        request.direction = self.direction;
        request.position = position;
        request.radius = radius;
        request.lod = self.lod;
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
    pub fn is_current(&self, epoch: u64) -> bool {
        self.shared.epoch.load(Ordering::Acquire) == epoch
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
    let mut lod = true;
    let mut ready = HashMap::new();
    let mut wanted = VecDeque::new();
    let mut recent = HashMap::<Key, (u64, u64, Loaded)>::new();
    let mut view = View {
        position,
        direction: Vec3::ZERO,
        travel: Vec3::ZERO,
        radius: 0.0,
    };
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
            lod = request.lod;
            let direction = request.direction;
            let travel = request.travel;
            ready = request.loaded.clone();
            recent.retain(|key, (e, r, _)| {
                *e >= epoch && *r == revision && terrain_region::visible(*key, position, radius)
            });
            for (key, (_, _, size)) in &recent {
                ready.insert(*key, *size);
            }
            view = View {
                position,
                direction,
                travel,
                radius,
            };
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
                || ready.get(key).is_some_and(|r| {
                    (if lod {
                        Level::select(*key, position, Some(r.level))
                    } else {
                        Level::Full
                    }) != r.level
                })
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
        if terrain_budget::victims(&ready, key, 256, view, shared.mesh_budget).is_none() {
            shared.full.store(true, Ordering::Relaxed);
            continue;
        }
        let level = if lod {
            Level::select(key, position, ready.get(&key).map(|r| r.level))
        } else {
            Level::Full
        };
        let Some(region) = builder.build_lod(key, revision, position, level, || {
            shared.stop.load(Ordering::Acquire)
                || (shared.epoch.load(Ordering::Acquire) != epoch && {
                    let request = shared.request.lock().unwrap();
                    let offset = terrain_region::center(key) - request.position;
                    request.revision != revision
                        || !terrain_region::visible(key, request.position, request.radius)
                        || (offset.length() > 16.0
                            && offset.normalize_or_zero().dot(request.direction) < -0.2)
                })
        }) else {
            continue;
        };
        let Some(victims) =
            terrain_budget::victims(&ready, key, region.bytes, view, shared.mesh_budget)
        else {
            shared.full.store(true, Ordering::Relaxed);
            continue;
        };
        if !victims.is_empty() {
            for key in &victims {
                ready.remove(key);
                recent.remove(key);
            }
            shared.waiting.fetch_add(1, Ordering::Relaxed);
            if sender
                .send(Ok(Update::MeshesEvicted {
                    keys: victims,
                    epoch,
                }))
                .is_err()
            {
                return;
            }
        }
        let loaded = Loaded {
            bytes: region.bytes,
            level: region.level,
        };
        ready.insert(key, loaded);
        recent.insert(key, (epoch, revision, loaded));
        shared.waiting.fetch_add(1, Ordering::Relaxed);
        if sender.send(Ok(Update::Region(region))).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::preview::Preview;
    use std::time::{Duration, Instant};
    fn settle(stream: &Stream, loaded: &mut HashMap<Key, Loaded>) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match stream.results.recv_timeout(Duration::from_millis(50)) {
                Ok(update) => {
                    stream.received();
                    match update.unwrap() {
                        Update::Region(region) => {
                            loaded.insert(
                                region.key,
                                Loaded {
                                    bytes: region.bytes,
                                    level: region.level,
                                },
                            );
                        }
                        Update::MeshesEvicted { keys, epoch } if stream.is_current(epoch) => {
                            for key in keys {
                                loaded.remove(&key);
                            }
                        }
                        _ => {}
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) if !stream.pending() => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("{error}"),
            }
            assert!(
                Instant::now() < deadline,
                "the cache must settle without waiting for movement"
            );
        }
    }
    #[test]
    fn turning_replaces_regions_in_a_full_cache_without_moving() {
        let mut world = World::with_height(32);
        world.infinite = true;
        world.noise_preview = Some(Preview {
            origin: [120, 318, -20],
            height_m: 2,
            ..Default::default()
        });
        let position = terrain_region::center([0, 0, 0]);
        let budget = 12 * 256;
        let mut stream = Stream::with_budget(
            world,
            Sky::default(),
            position,
            32.0,
            HashMap::new(),
            budget,
        );
        let mut loaded = HashMap::new();
        stream.direction = Vec3::X;
        stream.request(position, 32.0, loaded.clone(), None);
        settle(&stream, &mut loaded);
        assert!(stream.full());
        assert!(
            loaded.contains_key(&[2, 0, 0]),
            "the facing side must be loaded first"
        );
        assert!(loaded.values().map(|r| r.bytes).sum::<usize>() <= budget);
        let previous_epoch = stream.shared.epoch.load(Ordering::Acquire);
        stream.direction = -Vec3::X;
        stream.request(position, 32.0, loaded.clone(), None);
        assert!(
            !stream.is_current(previous_epoch),
            "old eviction messages must not alter the new view"
        );
        settle(&stream, &mut loaded);
        assert!(
            loaded.contains_key(&[-2, 0, 0]),
            "turning must bring in the new view despite a full cache"
        );
        assert!(
            !loaded.contains_key(&[2, 0, 0]),
            "old lower-priority regions must make room"
        );
        assert!(
            loaded.contains_key(&[0, 0, 0]),
            "nearby terrain must remain available"
        );
        assert!(loaded.values().map(|r| r.bytes).sum::<usize>() <= budget);
    }
}
