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
    sync::{mpsc, Arc, Mutex},
};
type Tile = Arc<[u8; 8 * 8 * 8 * 3]>;
enum Job {
    Pages(Vec<Key>),
    Mesh(Arc<World>, Key, crate::terrain_lod::Level),
    Tiles(Arc<World>, Vec<Key>),
    Sky(Arc<World>, Vec<[i32; 2]>),
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
    Pages(Vec<(Key, Chunk)>),
    Mesh(Key, Arc<crate::gpu_vertex::PackedMesh>),
    Tiles(Vec<(Key, Tile)>),
    Sky(Vec<([i32; 2], Column)>),
}
pub struct Pages {
    jobs: mpsc::SyncSender<Job>,
    results: mpsc::Receiver<Result<Done, String>>,
    workers: usize,
    meshes: Arc<Mutex<crate::terrain_meshes::Cache>>,
}
impl Pages {
    pub fn new(preview: Preview, shared: Arc<crate::worldgen::features::Shared>) -> Self {
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .saturating_sub(2)
            .clamp(1, 8);
        let (jobs, rx) = mpsc::sync_channel::<Job>(workers * 2);
        let rx = Arc::new(Mutex::new(rx));
        let (tx, results) = mpsc::sync_channel(workers * 2);
        let meshes = Arc::new(Mutex::new(crate::terrain_meshes::Cache::default()));
        for id in 0..workers {
            let rx = rx.clone();
            let tx = tx.clone();
            let shared = shared.clone();
            let meshes = meshes.clone();
            std::thread::Builder::new()
                .name(format!("terrain-samples-{id}"))
                .spawn(move || {
                    let mut generator = Generator::new(preview.seed).expect("page generator");
                    generator.feature_plans = shared;
                    let mut sampler = generator.sampler();
                    loop {
                        let Ok(job) = rx.lock().unwrap().recv() else {
                            return;
                        };
                        let result =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
                                Job::Pages(keys) => Done::Pages(
                                    keys.into_iter()
                                        .map(|key| {
                                            (
                                                key,
                                                page::generate(
                                                    preview,
                                                    &mut sampler,
                                                    key.map(|v| v * 32),
                                                )
                                                .data,
                                            )
                                        })
                                        .collect(),
                                ),
                                Job::Mesh(world, key, level) => {
                                    let cached = meshes.lock().unwrap().get(key);
                                    let mesh = cached.unwrap_or_else(|| {
                                        let mesh =
                                            Arc::new(crate::mesh::Mesh::build_chunk(&world, key));
                                        meshes.lock().unwrap().insert(key, mesh.clone());
                                        mesh
                                    });
                                    Done::Mesh(
                                        key,
                                        Arc::new(crate::terrain_lod::reduce_cached(
                                            &mesh, key, level,
                                        )),
                                    )
                                }
                                Job::Tiles(world, keys) => Done::Tiles(
                                    keys.into_iter()
                                        .map(|k| {
                                            (k, light_tiles::generate(&world, &mut sampler, k))
                                        })
                                        .collect(),
                                ),
                                Job::Sky(world, points) => Done::Sky(
                                    points
                                        .into_iter()
                                        .map(|p| (p, sky::sample_column(&world, &mut sampler, p)))
                                        .collect(),
                                ),
                            }))
                            .map_err(|_| "Terrain sampling worker failed".to_string());
                        if tx.send(result).is_err() {
                            return;
                        }
                    }
                })
                .expect("terrain sampling worker");
        }
        Self {
            jobs,
            results,
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
        loop {
            cancelled |= cancel();
            while !cancelled && active < self.workers * 2 {
                let Some(job) = next.next() else {
                    break;
                };
                self.jobs.send(job).expect("sampling pool stopped");
                active += 1;
            }
            if active == 0 {
                break;
            }
            let done = self
                .results
                .recv()
                .expect("sampling pool stopped")
                .expect("terrain sampling failed");
            observe(&done);
            complete.push(done);
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
