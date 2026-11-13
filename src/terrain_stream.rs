use crate::{
    terrain_budget::{self, Loaded, View},
    terrain_lod::Level,
    terrain_region::{self, Builder, Key, Region},
    world::World,
    worldgen::{sky::Sky, Generator},
};
use glam::Vec3;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    time::Instant,
};
#[path = "terrain_lighting.rs"]
mod lighting;
const MAX_IN_FLIGHT: usize = 3;

struct Request {
    position: Vec3,
    direction: Vec3,
    travel: Vec3,
    radius: f32,
    lod: bool,
    loaded: HashMap<Key, Loaded>,
    edits: Option<World>,
    cells: Vec<Key>,
    revision: u64,
    requested: Instant,
}
struct Shared {
    request: Mutex<Request>,
    wake: Condvar,
    epoch: AtomicU64,
    work_version: AtomicU64,
    stop: AtomicBool,
    pending: AtomicUsize,
    waiting: AtomicUsize,
    full: AtomicBool,
    mesh_budget: usize,
    inflight: Mutex<HashMap<Key, (u64, u64)>>,
    next_ticket: AtomicU64,
    light_jobs: Mutex<Vec<lighting::Job>>,
    light_wake: Condvar,
    light_dirty: Mutex<HashSet<Key>>,
    light_active: AtomicBool,
    geometry_active: AtomicBool,
    queued: AtomicUsize,
    oldest: Mutex<Option<Instant>>,
}
impl Shared {
    fn current(&self, key: Key, revision: u64, ticket: u64) -> bool {
        self.inflight.lock().unwrap().get(&key) == Some(&(ticket, revision))
    }
    fn send(&self, sender: &mpsc::SyncSender<Result<Update, String>>, update: Update) -> bool {
        self.waiting.fetch_add(1, Ordering::AcqRel);
        if sender.send(Ok(update)).is_err() {
            self.waiting.fetch_sub(1, Ordering::AcqRel);
            return false;
        }
        true
    }
}
pub enum Update {
    Geometry(Region),
    Region(Region),
    Cancelled {
        key: Key,
        revision: u64,
        ticket: u64,
    },
    Evicted(Vec<Key>),
    MeshesEvicted {
        keys: Vec<Key>,
        epoch: u64,
    },
    Pages(Vec<(Key, crate::chunk::Chunk)>),
}
pub struct Stream {
    shared: Arc<Shared>,
    pub results: mpsc::Receiver<Result<Update, String>>,
    pub center: Key,
    pub radius: f32,
    pub lod: bool,
    pub direction: Vec3,
    pub revision: u64,
    requested_direction: Vec3,
    requested_position: Vec3,
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
                requested: Instant::now(),
            }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(1),
            work_version: AtomicU64::new(1),
            stop: AtomicBool::new(false),
            pending: AtomicUsize::new(1),
            waiting: AtomicUsize::new(0),
            full: AtomicBool::new(false),
            mesh_budget,
            inflight: Mutex::new(HashMap::new()),
            next_ticket: AtomicU64::new(1),
            light_jobs: Mutex::new(Vec::new()),
            light_wake: Condvar::new(),
            light_dirty: Mutex::new(HashSet::new()),
            light_active: AtomicBool::new(false),
            geometry_active: AtomicBool::new(false),
            queued: AtomicUsize::new(0),
            oldest: Mutex::new(None),
        });
        let work = shared.clone();
        let (sender, results) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("terrain-geometry".into())
            .spawn(move || {
                let failure = sender.clone();
                if let Err(error) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(world, sky, &work, sender)
                })) {
                    let message = error
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "Terrain worker stopped".into());
                    let _ = failure.send(Err(message));
                }
            })
            .expect("terrain geometry worker");
        Self {
            shared,
            results,
            center: crate::mesh_stream::center(position),
            radius,
            lod: true,
            direction: Vec3::ZERO,
            requested_direction: Vec3::ZERO,
            requested_position: position,
            revision: 0,
        }
    }
    pub fn turned(&self, direction: Vec3) -> bool {
        view_turned(self.requested_direction, direction)
    }
    pub fn moved(&self, position: Vec3) -> bool {
        // Refresh body data before reaching an unloaded two-metre page boundary,
        // including when a stationary camera turn cleared the travel corridor.
        self.requested_position.distance_squared(position) >= 0.5 * 0.5
    }
    pub fn request(
        &mut self,
        position: Vec3,
        radius: f32,
        loaded: HashMap<Key, Loaded>,
        edits: Option<(&World, &[Key])>,
    ) {
        let mut request = self.shared.request.lock().unwrap();
        request.travel = (position - request.position).normalize_or_zero();
        request.direction = self.direction;
        self.requested_direction = self.direction;
        self.requested_position = position;
        request.position = position;
        request.radius = radius;
        request.lod = self.lod;
        request.loaded = loaded;
        request.requested = Instant::now();
        if let Some((world, cells)) = edits {
            self.revision += 1;
            request.edits = Some(world.clone());
            request.cells.extend_from_slice(cells);
            self.shared
                .light_dirty
                .lock()
                .unwrap()
                .extend(cells.iter().copied());
        }
        request.revision = self.revision;
        self.shared
            .inflight
            .lock()
            .unwrap()
            .retain(|key, (_, rev)| {
                *rev == self.revision && terrain_region::visible(*key, position, radius)
            });
        self.center = crate::mesh_stream::center(position);
        self.radius = radius;
        self.shared.pending.store(1, Ordering::Release);
        self.shared.full.store(false, Ordering::Release);
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        self.shared.work_version.fetch_add(1, Ordering::AcqRel);
        self.shared.wake.notify_all();
        self.shared.light_wake.notify_all();
    }
    pub fn is_current(&self, epoch: u64) -> bool {
        self.shared.epoch.load(Ordering::Acquire) == epoch
    }
    pub fn current_ticket(&self, key: Key, revision: u64, ticket: u64) -> bool {
        revision == self.revision && self.shared.current(key, revision, ticket)
    }
    pub fn completed(&self, key: Key, revision: u64, ticket: u64, loaded: Option<Loaded>) {
        let mut request = self.shared.request.lock().unwrap();
        let mut inflight = self.shared.inflight.lock().unwrap();
        if inflight.get(&key) != Some(&(ticket, revision)) {
            return;
        }
        inflight.remove(&key);
        if request.revision == revision {
            if let Some(loaded) = loaded {
                request.loaded.insert(key, loaded);
            } else {
                request.loaded.remove(&key);
            }
            self.shared.work_version.fetch_add(1, Ordering::AcqRel);
        }
        drop(inflight);
        drop(request);
        self.shared.wake.notify_all();
        self.shared.light_wake.notify_all();
    }
    pub fn pending(&self) -> bool {
        // Drop each guard before acquiring the next: the lighting worker checks
        // tickets while holding its jobs lock.
        let inflight = !self.shared.inflight.lock().unwrap().is_empty();
        let lighting = !self.shared.light_jobs.lock().unwrap().is_empty();
        self.shared.pending.load(Ordering::Acquire) > 0
            || self.shared.waiting.load(Ordering::Acquire) > 0
            || inflight
            || self.shared.light_active.load(Ordering::Acquire)
            || lighting
    }
    pub fn forget(&self, keys: &[Key]) {
        let mut request = self.shared.request.lock().unwrap();
        let mut inflight = self.shared.inflight.lock().unwrap();
        let mut changed = false;
        for key in keys {
            changed |= request.loaded.remove(key).is_some();
            changed |= inflight.remove(key).is_some();
        }
        if changed {
            self.shared.work_version.fetch_add(1, Ordering::AcqRel);
        }
        drop(inflight);
        drop(request);
        self.shared.wake.notify_all();
        self.shared.light_wake.notify_all();
    }
    pub fn stats(&self) -> crate::stream_metrics::Snapshot {
        let light_queued = self.shared.light_jobs.lock().unwrap().len();
        let oldest_ms = self
            .shared
            .oldest
            .lock()
            .unwrap()
            .map_or(0.0, |t| t.elapsed().as_secs_f32() * 1000.0);
        crate::stream_metrics::Snapshot {
            queued: self.shared.queued.load(Ordering::Acquire),
            geometry_active: self.shared.geometry_active.load(Ordering::Acquire),
            light_queued,
            light_active: self.shared.light_active.load(Ordering::Acquire),
            oldest_ms,
            ..Default::default()
        }
    }
    pub fn received(&self) {
        self.shared.waiting.fetch_sub(1, Ordering::AcqRel);
    }
    pub fn mark_full(&self) {
        self.shared.full.store(true, Ordering::Release);
    }
    pub fn full(&self) -> bool {
        self.shared.full.load(Ordering::Acquire)
    }
}
fn view_turned(previous: Vec3, direction: Vec3) -> bool {
    previous.length_squared() < 0.5 || previous.dot(direction) < 0.9848077
}
impl Drop for Stream {
    fn drop(&mut self) {
        let request = self
            .shared
            .request
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let jobs = self
            .shared
            .light_jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        self.shared.stop.store(true, Ordering::Release);
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        self.shared.work_version.fetch_add(1, Ordering::AcqRel);
        drop(jobs);
        drop(request);
        self.shared.wake.notify_all();
        self.shared.light_wake.notify_all();
    }
}
fn needed(
    builder: &Builder<'_>,
    ready: &HashMap<Key, Loaded>,
    key: Key,
    lod: bool,
    position: Vec3,
) -> bool {
    !ready.contains_key(&key)
        || ready.get(&key).is_some_and(|r| {
            (if lod {
                Level::select(key, position, Some(r.level))
            } else {
                Level::Full
            }) != r.level
        })
        || (terrain_region::center(key).distance(position) < 16.0 && !builder.has_near_data(key))
}
fn run(
    world: World,
    sky: Sky,
    shared: &Arc<Shared>,
    sender: mpsc::SyncSender<Result<Update, String>>,
) {
    let generator = Generator::new(world.noise_preview.expect("procedural world").seed)
        .expect("world generator");
    let budget = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .saturating_sub(2)
        .clamp(1, 8);
    let pages = crate::terrain_pages::Pages::with_workers(
        world.noise_preview.unwrap(),
        generator.feature_plans.clone(),
        world.chunk_timings.clone(),
        budget,
    );
    lighting::spawn(
        world.clone(),
        sky,
        generator.feature_plans.clone(),
        shared.clone(),
        sender.clone(),
        pages.clone(),
    );
    let mut builder = Builder::new(world, &generator, Sky::default());
    builder.set_page_pool(pages);
    let (mut epoch, mut revision) = (0, 0);
    let mut view_epoch = 0;
    let mut position = Vec3::ZERO;
    let mut lod = true;
    let mut ready = HashMap::new();
    let mut wanted = VecDeque::new();
    let mut recent = HashMap::<Key, (u64, u64, Loaded)>::new();
    let mut since = HashMap::<Key, Instant>::new();
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
        let current = shared.work_version.load(Ordering::Acquire);
        if current != epoch {
            let mut request = shared.request.lock().unwrap();
            epoch = shared.work_version.load(Ordering::Acquire);
            view_epoch = shared.epoch.load(Ordering::Acquire);
            if revision != request.revision {
                since.clear();
            }
            revision = request.revision;
            position = request.position;
            lod = request.lod;
            view = View {
                position,
                direction: request.direction,
                travel: request.travel,
                radius: request.radius,
            };
            ready = request.loaded.clone();
            let inflight = shared.inflight.lock().unwrap();
            recent.retain(|key, (ticket, rev, _)| {
                *rev == revision
                    && inflight.get(key) == Some(&(*ticket, *rev))
                    && !ready.contains_key(key)
                    && terrain_region::visible(*key, position, view.radius)
            });
            for (key, (_, _, loaded)) in &recent {
                ready.insert(*key, *loaded);
            }
            drop(inflight);
            if let Some(world) = request.edits.take() {
                builder.edit(&world, &request.cells);
                request.cells.clear();
            }
            let requested = request.requested;
            drop(request);
            let evicted = builder.trim(position);
            if !evicted.is_empty() && !shared.send(&sender, Update::Evicted(evicted)) {
                return;
            }
            shared.geometry_active.store(true, Ordering::Release);
            let pages = builder.collision(position, view.travel, &|| {
                shared.stop.load(Ordering::Acquire)
                    || shared.epoch.load(Ordering::Acquire) != view_epoch
            });
            shared.geometry_active.store(false, Ordering::Release);
            if !pages.is_empty() && !shared.send(&sender, Update::Pages(pages)) {
                return;
            }
            let inflight = shared.inflight.lock().unwrap();
            wanted = terrain_region::candidates(
                &builder.world,
                position,
                view.radius,
                view.direction,
                view.travel,
            )
            .into_iter()
            .filter(|key| {
                !inflight.contains_key(key) && needed(&builder, &ready, *key, lod, position)
            })
            .collect();
            since.retain(|key, _| wanted.contains(key) || inflight.contains_key(key));
            for key in &wanted {
                since.entry(*key).or_insert(requested);
            }
        }
        shared.queued.store(wanted.len(), Ordering::Release);
        *shared.oldest.lock().unwrap() = since.values().copied().min();
        if shared.inflight.lock().unwrap().len() >= MAX_IN_FLIGHT {
            let mut request = shared.request.lock().unwrap();
            while shared.work_version.load(Ordering::Acquire) == epoch
                && !shared.stop.load(Ordering::Acquire)
                && shared.inflight.lock().unwrap().len() >= MAX_IN_FLIGHT
            {
                request = shared.wake.wait(request).unwrap();
            }
            continue;
        }
        let Some(key) = wanted.pop_front() else {
            shared.pending.store(0, Ordering::Release);
            let mut request = shared.request.lock().unwrap();
            while shared.work_version.load(Ordering::Acquire) == epoch
                && !shared.stop.load(Ordering::Acquire)
            {
                request = shared.wake.wait(request).unwrap();
            }
            continue;
        };
        shared.queued.store(wanted.len(), Ordering::Release);
        shared.pending.store(1, Ordering::Release);
        if terrain_budget::victims(&ready, key, 256, view, shared.mesh_budget).is_none() {
            shared.full.store(true, Ordering::Release);
            since.remove(&key);
            continue;
        }
        let level = if lod {
            Level::select(key, position, ready.get(&key).map(|r| r.level))
        } else {
            Level::Full
        };
        shared.geometry_active.store(true, Ordering::Release);
        let region = builder.prepare_geometry(key, revision, position, level, || {
            shared.stop.load(Ordering::Acquire)
                || (shared.epoch.load(Ordering::Acquire) != view_epoch && {
                    let request = shared.request.lock().unwrap();
                    let offset = terrain_region::center(key) - request.position;
                    request.revision != revision
                        || !terrain_region::visible(key, request.position, request.radius)
                        || (offset.length() > 16.0
                            && offset.normalize_or_zero().dot(request.direction) < -0.2)
                })
        });
        shared.geometry_active.store(false, Ordering::Release);
        let Some(mut region) = region else {
            continue;
        };
        // Pages are procedural base data, with current edits reapplied by the
        // receiver. Their delivery must not depend on accepting a mesh ticket:
        // the builder already marked them published, even if this view/revision
        // changed during preparation. Keep raw data ordered before its geometry.
        let evicted = std::mem::take(&mut region.evicted);
        if !evicted.is_empty() && !shared.send(&sender, Update::Evicted(evicted)) {
            return;
        }
        let pages = std::mem::take(&mut region.pages);
        if !pages.is_empty() && !shared.send(&sender, Update::Pages(pages)) {
            return;
        }
        let Some(victims) =
            terrain_budget::victims(&ready, key, region.bytes, view, shared.mesh_budget)
        else {
            shared.full.store(true, Ordering::Release);
            since.remove(&key);
            continue;
        };
        if !victims.is_empty() {
            for key in &victims {
                ready.remove(key);
                recent.remove(key);
            }
            if !shared.send(
                &sender,
                Update::MeshesEvicted {
                    keys: victims,
                    epoch: view_epoch,
                },
            ) {
                return;
            }
        }
        region.timing.requested = *since.entry(key).or_insert(region.timing.started);
        region.ticket = shared.next_ticket.fetch_add(1, Ordering::AcqRel);
        let ticket = region.ticket;
        let loaded = Loaded {
            bytes: region.bytes,
            level: region.level,
        };
        // A view may have changed during preparation; edits and out-of-range jobs must never enter the pipeline.
        {
            let request = shared.request.lock().unwrap();
            if request.revision != revision
                || !terrain_region::visible(key, request.position, request.radius)
            {
                continue;
            }
            shared
                .inflight
                .lock()
                .unwrap()
                .insert(key, (ticket, revision));
        }
        ready.insert(key, loaded);
        recent.insert(key, (ticket, revision, loaded));
        if region.needs_light {
            let job = lighting::Job {
                region: region.lighting_request(),
                world: builder.world.clone(),
            };
            if !shared.send(&sender, Update::Geometry(region)) {
                return;
            }
            let mut jobs = shared.light_jobs.lock().unwrap();
            jobs.retain(|job| {
                shared.current(job.region.key, job.region.revision, job.region.ticket)
            });
            jobs.push(job);
            drop(jobs);
            shared.light_wake.notify_one();
        } else if !shared.send(&sender, Update::Region(region)) {
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
                            stream.completed(
                                region.key,
                                region.revision,
                                region.ticket,
                                Some(Loaded {
                                    bytes: region.bytes,
                                    level: region.level,
                                }),
                            );
                            loaded.insert(
                                region.key,
                                Loaded {
                                    bytes: region.bytes,
                                    level: region.level,
                                },
                            );
                        }
                        Update::MeshesEvicted { keys, epoch } if stream.is_current(epoch) => {
                            stream.forget(&keys);
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
    #[test]
    fn slow_turns_accumulate_against_the_last_requested_view() {
        let mut last = Vec3::X;
        let mut requests = 0;
        for degree in 1..=90 {
            let a = (degree as f32).to_radians();
            let direction = Vec3::new(a.cos(), 0.0, a.sin());
            if view_turned(last, direction) {
                requests += 1;
                last = direction;
            }
        }
        assert!(requests >= 7, "gradual turns must trigger prioritisation");
        assert!(!view_turned(last, last));
    }

    fn small_world() -> World {
        let mut world = World::with_height(32);
        world.infinite = true;
        world.noise_preview = Some(Preview {
            origin: [120, 318, -20],
            height_m: 2,
            ..Default::default()
        });
        world
    }
    #[test]
    fn walking_refreshes_collision_data_before_crossing_a_render_page() {
        let start = terrain_region::center([0; 3]);
        let mut stream = Stream::new(small_world(), Sky::default(), start, 0.0, HashMap::new());
        assert!(!stream.moved(start + Vec3::X * 0.49));
        let next = start + Vec3::X * 0.5;
        assert!(stream.moved(next));
        assert_eq!(
            crate::mesh_stream::center(start),
            crate::mesh_stream::center(next)
        );
        stream.request(next, 0.0, HashMap::new(), None);
        assert!(!stream.moved(next));
        assert!(stream.moved(next + Vec3::Z * 0.5));
    }
    #[test]
    fn editing_rejects_old_geometry_without_losing_its_collision_pages() {
        let mut frontend = small_world();
        let position = terrain_region::center([0; 3]);
        let mut stream = Stream::new(
            frontend.clone(),
            Sky::default(),
            position,
            0.0,
            HashMap::new(),
        );
        // Receive an old mesh result, then emulate an edit invalidating it before
        // the render thread accepts it. Raw data must already be independent.
        loop {
            let update = stream
                .results
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            stream.received();
            match update {
                Update::Pages(pages) => {
                    for (key, data) in pages {
                        frontend.insert_page(key, data);
                    }
                }
                Update::Region(region) => {
                    assert!(region.pages.is_empty());
                    break;
                }
                _ => {}
            }
        }
        let cell = [1, 1, 1];
        frontend.set(cell[0], cell[1], cell[2], crate::world::STONE);
        stream.request(position, 0.0, HashMap::new(), Some((&frontend, &[cell])));
        loop {
            let update = stream
                .results
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            stream.received();
            match update {
                Update::Pages(pages) => {
                    for (key, data) in pages {
                        frontend.insert_page(key, data);
                    }
                }
                Update::Geometry(region) if region.revision == 1 => break,
                _ => {}
            }
        }
        for z in 0..4 {
            for x in 0..4 {
                assert!(
                    frontend.page([x, 0, z]).is_some(),
                    "rendered region must not contain unloaded collision barriers at {x}, {z}"
                );
            }
        }
        assert_eq!(
            frontend.get_static(1, 1, 1),
            crate::world::STONE,
            "late base data must preserve current edits"
        );
        assert_eq!(frontend.get_static(100, 1, 100), crate::world::AIR);
    }
    #[test]
    fn geometry_is_delivered_before_lighting_and_inflight_work_is_bounded() {
        let mut world = small_world();
        world.insert_page([0, 0, 0], crate::chunk::Chunk::Uniform(crate::world::STONE));
        let stream = Stream::new(
            world,
            Sky::default(),
            terrain_region::center([0, 0, 0]),
            4.0,
            HashMap::new(),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut geometry = HashSet::new();
        let mut saw_split = false;
        loop {
            let update = stream
                .results
                .recv_timeout(Duration::from_secs(5))
                .expect("stream stalled")
                .unwrap();
            stream.received();
            assert!(stream.shared.inflight.lock().unwrap().len() <= MAX_IN_FLIGHT);
            match update {
                Update::Geometry(region) => {
                    assert!(region.needs_light);
                    assert!(region.light.is_none());
                    assert!(region.field.is_none());
                    assert!(!region.meshes.is_empty());
                    geometry.insert(region.ticket);
                }
                Update::Region(region) => {
                    if region.needs_light {
                        assert!(
                            geometry.contains(&region.ticket),
                            "lighting must follow geometry delivery"
                        );
                        assert!(region.light.is_some());
                        assert!(region.meshes.is_empty());
                        saw_split = true;
                    }
                    stream.completed(
                        region.key,
                        region.revision,
                        region.ticket,
                        Some(Loaded {
                            bytes: region.bytes,
                            level: region.level,
                        }),
                    );
                    if saw_split {
                        break;
                    }
                }
                _ => {}
            }
            assert!(Instant::now() < deadline);
        }
        assert!(saw_split);
    }
    #[test]
    fn stale_acknowledgements_cannot_release_a_newer_ticket_or_change_the_view_epoch() {
        let stream = Stream::new(
            small_world(),
            Sky::default(),
            terrain_region::center([0, 0, 0]),
            0.0,
            HashMap::new(),
        );
        let key = [500, 0, 500];
        stream.shared.inflight.lock().unwrap().insert(key, (200, 0));
        let epoch = stream.shared.epoch.load(Ordering::Acquire);
        stream.completed(
            key,
            0,
            199,
            Some(Loaded {
                bytes: 256,
                level: Level::Full,
            }),
        );
        assert_eq!(
            stream.shared.inflight.lock().unwrap().get(&key),
            Some(&(200, 0))
        );
        assert!(!stream
            .shared
            .request
            .lock()
            .unwrap()
            .loaded
            .contains_key(&key));
        stream.completed(
            key,
            0,
            200,
            Some(Loaded {
                bytes: 256,
                level: Level::Full,
            }),
        );
        assert!(!stream.shared.inflight.lock().unwrap().contains_key(&key));
        assert_eq!(
            stream.shared.epoch.load(Ordering::Acquire),
            epoch,
            "publication acknowledgements must not invalidate view-tagged evictions"
        );
        stream.forget(&[key]);
        assert!(!stream
            .shared
            .request
            .lock()
            .unwrap()
            .loaded
            .contains_key(&key));
    }
}
