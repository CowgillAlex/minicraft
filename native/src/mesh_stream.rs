use crate::{
    mesh::Mesh,
    world::{World, CHUNK, NX, NZ},
};
use glam::Vec3;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, TrySendError},
        Arc,
    },
};

pub const RADIUS: f32 = 28.0;
pub type Key = [i32; 3];
struct Job {
    epoch: u64,
    world: World,
    keys: Vec<Key>,
}
pub struct ResultMesh {
    pub epoch: u64,
    pub key: Key,
    pub mesh: Mesh,
}

/// One coalesced job and eight results bound CPU work and memory while the player moves.
pub struct Stream {
    epoch: Arc<AtomicU64>,
    jobs: mpsc::SyncSender<Job>,
    pub results: mpsc::Receiver<ResultMesh>,
    queued: Option<Job>,
    pub center: Key,
    pub order: Vec<Key>,
    pub desired: HashSet<Key>,
}

pub fn center(position: Vec3) -> Key {
    crate::ray::grid_position(position)
        .floor()
        .as_ivec3()
        .to_array()
        .map(|v| v.div_euclid(CHUNK as i32))
}
pub fn wanted(world: &World, position: Vec3) -> Vec<Key> {
    let c = center(position);
    let r = (RADIUS / 2.0).ceil() as i32 + 1;
    let mut keys = Vec::new();
    for y in (c[1] - r).max(0)..=(c[1] + r).min(world.chunks_y() as i32 - 1) {
        for z in (c[2] - r).max(0)..=(c[2] + r).min(NZ as i32 - 1) {
            for x in (c[0] - r).max(0)..=(c[0] + r).min(NX as i32 - 1) {
                let key = [x, y, z];
                let midpoint = World::meters(x * 32 + 16, y * 32 + 16, z * 32 + 16);
                if midpoint.distance(position) > RADIUS + 3.0_f32.sqrt()
                    || !world.chunk_exists(x as usize, y as usize, z as usize)
                    || world.buried_chunk(key)
                {
                    continue;
                }
                keys.push(key);
            }
        }
    }
    keys.sort_by(|a, b| {
        let point = |key: Key| World::meters(key[0] * 32 + 16, key[1] * 32 + 16, key[2] * 32 + 16);
        point(*a)
            .distance_squared(position)
            .total_cmp(&point(*b).distance_squared(position))
    });
    keys
}

impl Stream {
    pub fn new(position: Vec3, order: Vec<Key>) -> Self {
        let epoch = Arc::new(AtomicU64::new(1));
        let token = epoch.clone();
        let (jobs, requests) = mpsc::sync_channel::<Job>(1);
        let (sender, results) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("nearby-meshes".into())
            .spawn(move || {
                while let Ok(job) = requests.recv() {
                    for key in job.keys {
                        if token.load(Ordering::Relaxed) != job.epoch {
                            break;
                        }
                        let mesh = Mesh::build_chunk(&job.world, key);
                        if sender
                            .send(ResultMesh {
                                epoch: job.epoch,
                                key,
                                mesh,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            })
            .expect("mesh worker");
        let desired = order.iter().copied().collect();
        Self {
            epoch,
            jobs,
            results,
            queued: None,
            center: center(position),
            order,
            desired,
        }
    }
    pub fn request(&mut self, world: &World, position: Vec3, loaded: impl Fn(&Key) -> bool) {
        let epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        self.order = wanted(world, position);
        self.desired = self.order.iter().copied().collect();
        let keys = self
            .order
            .iter()
            .filter(|key| !loaded(key))
            .copied()
            .collect();
        self.center = center(position);
        self.queued = Some(Job {
            epoch,
            world: world.clone(),
            keys,
        });
        self.submit();
    }
    pub fn submit(&mut self) {
        if let Some(job) = self.queued.take() {
            match self.jobs.try_send(job) {
                Ok(()) => {}
                Err(TrySendError::Full(job)) => self.queued = Some(job),
                Err(TrySendError::Disconnected(_)) => panic!("Mesh worker stopped"),
            }
        }
    }
    pub fn accepts(&self, result: &ResultMesh) -> bool {
        result.epoch == self.epoch.load(Ordering::Relaxed) && self.desired.contains(&result.key)
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearby_chunks_follow_height_and_exclude_distant_geometry() {
        let mut world = World::with_height(384 * 16);
        world.set(320, 2000, 320, crate::world::STONE);
        world.set(320, 4000, 320, crate::world::STONE);
        let high = wanted(&world, World::meters(320, 4000, 320));
        assert!(high.contains(&[10, 125, 10]));
        assert!(!high.contains(&[10, 62, 10]));
    }
    #[test]
    fn edited_snapshots_reject_old_mesh_results() {
        let mut world = World::with_height(384 * 16);
        world.set(320, 4000, 320, crate::world::STONE);
        let position = World::meters(320, 4000, 320);
        let mut stream = Stream::new(position, Vec::new());
        stream.request(&world, position, |_| false);
        let old = stream
            .results
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert!(stream.accepts(&old));
        world.set(320, 4000, 320, crate::world::GLASS);
        stream.request(&world, position, |_| false);
        assert!(!stream.accepts(&old));
        let new = stream
            .results
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        assert!(stream.accepts(&new));
        assert!(new.mesh.indices.is_empty());
        assert!(!new.mesh.transparent_indices.is_empty());
    }
}
