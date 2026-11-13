//! Bounded persistent workers share immutable world snapshots and retain their
//! own exact noise caches. Only a few obsolete jobs can remain in flight.
use crate::{
    chunk::Chunk,
    light_tiles,
    terrain_region::Key,
    world::World,
    worldgen::{
        page,
        preview::Preview,
        sky::{self, Column},
        Generator,
    },
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
};
type Tile = Arc<[u8; 8 * 8 * 8 * 3]>;
enum Job {
    Pages(Vec<Key>),
    Mesh(Arc<World>, Key, crate::terrain_lod::Level),
    Tiles(Arc<World>, Vec<Key>),
    Sky(Arc<World>, Vec<[i32; 2]>),
}
struct QueuedJob {
    job: Job,
    cancel: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Result<Done, String>>,
}

#[cfg(test)]
#[test]
fn column_batches_match_independent_decorated_pages_and_cancel() {
    for (seed, x, z) in [(8349847389, 42, 14), (1794358123416304200, -900, -1028)] {
        let generator = Generator::new(seed).unwrap();
        let preview = Preview {
            seed,
            origin: [0, -64, 0],
            surfaces: true,
            features: true,
            ..Default::default()
        };
        let keys: Vec<_> = (62..68).flat_map(|y| [[x, y, z], [x + 1, y, z]]).collect();
        let mut reference = generator.sampler();
        let expected: BTreeMap<_, _> = keys
            .iter()
            .map(|&key| {
                (
                    key,
                    page::generate(preview, &mut reference, key.map(|v| v * 32))
                        .data
                        .to_vec(),
                )
            })
            .collect();
        let pool = Pages::new(preview, generator.feature_plans.clone());
        let actual = pool.generate(keys.iter().rev().copied().collect(), &|| false);
        assert_eq!(actual.len(), expected.len());
        for (key, data) in actual {
            assert_eq!(data.to_vec(), expected[&key], "seed {seed} at {key:?}");
        }
        assert!(pool.generate(keys, &|| true).is_empty());
    }
}
enum Done {
    Cancelled,
    Pages(Vec<(Key, Chunk)>),
    Mesh(Key, Arc<crate::gpu_vertex::PackedMesh>),
    Tiles(Vec<(Key, Tile)>),
    Sky(Vec<([i32; 2], Column)>),
}

#[cfg(test)]
#[test]
fn worker_timings_keep_generation_and_identify_source_mesh_reuse() {
    let preview = Preview {
        height_m: 12,
        origin: [120, 58, -20],
        surfaces: true,
        features: true,
        ..Default::default()
    };
    let generator = Generator::new(preview.seed).unwrap();
    let mut world = World::with_height(192);
    world.infinite = true;
    world.noise_preview = Some(preview);
    let key = [5, 1, 5];
    let pool = Pages::with_timings(
        preview,
        generator.feature_plans.clone(),
        world.chunk_timings.clone(),
    );
    for (key, data) in pool.generate(vec![key], &|| false) {
        world.insert_page(key, data);
    }
    let generated = world
        .chunk_timings
        .lock()
        .unwrap()
        .get(key)
        .generation_ms
        .unwrap();
    assert!(generated.is_finite() && generated >= 0.0);
    pool.meshes(&world, vec![key], crate::terrain_lod::Level::Full, &|| {
        false
    });
    let first = world.chunk_timings.lock().unwrap().get(key);
    assert_eq!(first.generation_ms, Some(generated));
    assert!(first.mesh_ms.unwrap().is_finite());
    assert!(!first.source_cached);
    pool.meshes(
        &world,
        vec![key],
        crate::terrain_lod::Level::Medium,
        &|| false,
    );
    let second = world.chunk_timings.lock().unwrap().get(key);
    assert_eq!(second.generation_ms, Some(generated));
    assert!(second.source_cached);
    assert!(!second.mesh_cached);
}
#[derive(Clone)]
pub struct Pages {
    jobs: mpsc::SyncSender<QueuedJob>,
    workers: usize,
    meshes: Arc<Mutex<crate::terrain_meshes::Cache>>,
}

#[cfg(test)]
#[test]
fn shared_pool_routes_concurrent_stage_results_and_cancellation_independently() {
    let preview = Preview {
        origin: [120, 318, -20],
        height_m: 2,
        ..Default::default()
    };
    let generator = Generator::new(preview.seed).unwrap();
    let mut world = World::with_height(32);
    world.infinite = true;
    world.noise_preview = Some(preview);
    let pool = Pages::with_workers(
        preview,
        generator.feature_plans.clone(),
        world.chunk_timings.clone(),
        2,
    );
    std::thread::scope(|scope| {
        let a = pool.clone();
        let b = pool.clone();
        let c = pool.clone();
        let world = &world;
        let generated =
            scope.spawn(move || a.generate((0..12).map(|x| [x, 0, 0]).collect(), &|| false));
        let sky = scope.spawn(move || b.sky(world, vec![[2, 2], [6, 2], [10, 2]], &|| false));
        let cancelled = scope.spawn(move || c.generate(vec![[100, 0, 0]], &|| true));
        assert_eq!(generated.join().unwrap().len(), 12);
        assert_eq!(sky.join().unwrap().len(), 3);
        assert!(cancelled.join().unwrap().is_empty());
    });
}
impl Pages {
    #[cfg(test)]
    pub fn new(preview: Preview, shared: Arc<crate::worldgen::features::Shared>) -> Self {
        Self::with_timings(preview, shared, Default::default())
    }
    pub fn with_timings(
        preview: Preview,
        shared: Arc<crate::worldgen::features::Shared>,
        timings: crate::chunk_timings::Shared,
    ) -> Self {
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .saturating_sub(2)
            .clamp(1, 8);
        Self::with_workers(preview, shared, timings, workers)
    }
    pub fn with_workers(
        preview: Preview,
        shared: Arc<crate::worldgen::features::Shared>,
        timings: crate::chunk_timings::Shared,
        workers: usize,
    ) -> Self {
        let workers = workers.max(1);
        let (jobs, rx) = mpsc::sync_channel::<QueuedJob>(workers * 2);
        let rx = Arc::new(Mutex::new(rx));
        let meshes = Arc::new(Mutex::new(crate::terrain_meshes::Cache::default()));
        for id in 0..workers {
            let rx = rx.clone();
            let shared = shared.clone();
            let meshes = meshes.clone();
            let timings = timings.clone();
            std::thread::Builder::new()
                .name(format!("terrain-samples-{id}"))
                .spawn(move || {
                    let mut generator = Generator::new(preview.seed).expect("page generator");
                    generator.feature_plans = shared;
                    let mut sampler = generator.sampler();
                    loop {
                        let Ok(QueuedJob { job, cancel, reply }) = rx.lock().unwrap().recv() else {
                            return;
                        };
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if cancel.load(Ordering::Relaxed) {
                                return Done::Cancelled;
                            }
                            match job {
                                Job::Pages(keys) => Done::Pages(
                                    keys.into_iter()
                                        .take_while(|_| !cancel.load(Ordering::Relaxed))
                                        .map(|key| {
                                            let started = std::time::Instant::now();
                                            let data = page::generate(
                                                preview,
                                                &mut sampler,
                                                key.map(|v| v * 32),
                                            )
                                            .data;
                                            let ms = started.elapsed().as_secs_f32() * 1000.0;
                                            timings.lock().unwrap().generation(key, ms);
                                            (key, data)
                                        })
                                        .collect(),
                                ),
                                Job::Mesh(world, key, level) => {
                                    let started = std::time::Instant::now();
                                    let cached = meshes.lock().unwrap().get(key);
                                    let source_cached = cached.is_some();
                                    let mesh = cached.unwrap_or_else(|| {
                                        let mesh =
                                            Arc::new(crate::mesh::Mesh::build_chunk(&world, key));
                                        meshes.lock().unwrap().insert(key, mesh.clone());
                                        mesh
                                    });
                                    if cancel.load(Ordering::Relaxed) {
                                        return Done::Cancelled;
                                    }
                                    let packed = Arc::new(crate::terrain_lod::reduce_cached(
                                        &mesh, key, level,
                                    ));
                                    let ms = started.elapsed().as_secs_f32() * 1000.0;
                                    timings.lock().unwrap().mesh(key, ms, source_cached);
                                    Done::Mesh(key, packed)
                                }
                                Job::Tiles(world, keys) => Done::Tiles(
                                    keys.into_iter()
                                        .take_while(|_| !cancel.load(Ordering::Relaxed))
                                        .map(|k| {
                                            (k, light_tiles::generate(&world, &mut sampler, k))
                                        })
                                        .collect(),
                                ),
                                Job::Sky(world, points) => Done::Sky(
                                    points
                                        .into_iter()
                                        .take_while(|_| !cancel.load(Ordering::Relaxed))
                                        .map(|p| (p, sky::sample_column(&world, &mut sampler, p)))
                                        .collect(),
                                ),
                            }
                        }))
                        .map_err(|_| "Terrain sampling worker failed".to_string());
                        // Reply routing is per execution, so geometry and lighting may
                        // share this worker pool without consuming each other's results.
                        let _ = reply.send(result);
                    }
                })
                .expect("terrain sampling worker");
        }
        Self {
            jobs,
            workers,
            meshes,
        }
    }
    pub fn invalidate_meshes(&self, cells: &[[i32; 3]]) {
        self.meshes.lock().unwrap().invalidate(cells);
    }
    fn execute(&self, jobs: Vec<Job>, cancel: &dyn Fn() -> bool) -> Vec<Done> {
        self.execute_observed(jobs, cancel, |_| {})
    }
    fn execute_observed(
        &self,
        jobs: Vec<Job>,
        cancel: &dyn Fn() -> bool,
        mut observe: impl FnMut(&Done),
    ) -> Vec<Done> {
        let mut next = jobs.into_iter();
        let mut active = 0;
        let mut complete = Vec::new();
        let mut cancelled = false;
        let token = Arc::new(AtomicBool::new(false));
        let (reply, results) = mpsc::sync_channel(self.workers * 2);
        loop {
            cancelled |= cancel();
            if cancelled {
                token.store(true, Ordering::Relaxed);
            }
            while !cancelled && active < self.workers * 2 {
                let Some(job) = next.next() else {
                    break;
                };
                self.jobs
                    .send(QueuedJob {
                        job,
                        cancel: token.clone(),
                        reply: reply.clone(),
                    })
                    .expect("sampling pool stopped");
                active += 1;
            }
            if active == 0 {
                break;
            }
            let done = match results.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(done) => done.expect("terrain sampling failed"),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => panic!("sampling pool stopped"),
            };
            if !matches!(done, Done::Cancelled) {
                observe(&done);
                complete.push(done);
            }
            active -= 1;
        }
        complete
    }
    pub fn generate(&self, keys: Vec<Key>, cancel: &dyn Fn() -> bool) -> Vec<(Key, Chunk)> {
        self.generate_with_progress(keys, cancel, |_| {})
    }
    pub fn generate_with_progress(
        &self,
        keys: Vec<Key>,
        cancel: &dyn Fn() -> bool,
        mut completed: impl FnMut(usize),
    ) -> Vec<(Key, Chunk)> {
        // A worker keeps the expensive fine-column surface context across
        // vertical pages. Limit batches so obsolete work remains bounded.
        let mut columns = BTreeMap::<[i32; 2], Vec<Key>>::new();
        for key in keys {
            columns.entry([key[0], key[2]]).or_default().push(key);
        }
        let jobs = columns
            .into_values()
            .flat_map(|mut keys| {
                keys.sort_unstable_by_key(|key| key[1]);
                keys.chunks(4)
                    .map(|keys| Job::Pages(keys.to_vec()))
                    .collect::<Vec<_>>()
            })
            .collect();
        self.execute_observed(jobs, cancel, |done| {
            if let Done::Pages(pages) = done {
                completed(pages.len());
            }
        })
        .into_iter()
        .flat_map(|d| {
            let Done::Pages(pages) = d else {
                unreachable!()
            };
            pages
        })
        .collect()
    }
    pub fn meshes(
        &self,
        world: &World,
        keys: Vec<Key>,
        level: crate::terrain_lod::Level,
        cancel: &dyn Fn() -> bool,
    ) -> Vec<(Key, Arc<crate::gpu_vertex::PackedMesh>)> {
        let world = Arc::new(world.clone());
        self.execute(
            keys.into_iter()
                .map(|key| Job::Mesh(world.clone(), key, level))
                .collect(),
            cancel,
        )
        .into_iter()
        .map(|done| {
            let Done::Mesh(key, mesh) = done else {
                unreachable!()
            };
            (key, mesh)
        })
        .collect()
    }
    pub fn tiles(
        &self,
        world: &World,
        keys: Vec<Key>,
        cancel: &dyn Fn() -> bool,
    ) -> Vec<(Key, Tile)> {
        if keys.is_empty() {
            return Vec::new();
        }
        let world = Arc::new(world.clone());
        self.execute(
            keys.chunks(16)
                .map(|k| Job::Tiles(world.clone(), k.to_vec()))
                .collect(),
            cancel,
        )
        .into_iter()
        .flat_map(|d| {
            let Done::Tiles(v) = d else { unreachable!() };
            v
        })
        .collect()
    }
    pub fn sky(
        &self,
        world: &World,
        points: Vec<[i32; 2]>,
        cancel: &dyn Fn() -> bool,
    ) -> Vec<([i32; 2], Column)> {
        if points.is_empty() {
            return Vec::new();
        }
        let world = Arc::new(world.clone());
        self.execute(
            points
                .chunks(64)
                .map(|p| Job::Sky(world.clone(), p.to_vec()))
                .collect(),
            cancel,
        )
        .into_iter()
        .flat_map(|d| {
            let Done::Sky(v) = d else { unreachable!() };
            v
        })
        .collect()
    }
}
