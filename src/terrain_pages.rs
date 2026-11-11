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
use std::sync::{mpsc, Arc, Mutex};
type Tile = Arc<[u8; 8 * 8 * 8 * 3]>;
enum Job {
    Page(Key),
    Mesh(Arc<World>, Key, crate::terrain_lod::Level),
    Tiles(Arc<World>, Vec<Key>),
    Sky(Arc<World>, Vec<[i32; 2]>),
}
enum Done {
    Page(Key, Chunk),
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
            .clamp(1, 4);
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
                                Job::Page(key) => Done::Page(
                                    key,
                                    page::generate(preview, &mut sampler, key.map(|v| v * 32)).data,
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
            complete.push(
                self.results
                    .recv()
                    .expect("sampling pool stopped")
                    .expect("terrain sampling failed"),
            );
            active -= 1;
        }
        complete
    }
    pub fn generate(&self, keys: Vec<Key>, cancel: &dyn Fn() -> bool) -> Vec<(Key, Chunk)> {
        self.execute(keys.into_iter().map(Job::Page).collect(), cancel)
            .into_iter()
            .map(|d| {
                let Done::Page(k, c) = d else { unreachable!() };
                (k, c)
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
