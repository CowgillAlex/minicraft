//! Local edits repair cached light graphs independently of terrain generation.
use crate::{
    light_field::LightField,
    light_tiles::{Source, Tiles},
    terrain_region::{self, Key},
    world::World,
    worldgen::{preview::Preview, sky::Sky, Generator},
};
use glam::Vec3;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
};
pub struct Seed {
    pub key: Key,
    pub field: LightField,
    pub revision: u64,
    pub focus: Vec3,
}
pub struct Update {
    pub key: Key,
    pub pixels: Vec<u8>,
    pub revision: u64,
}
#[derive(Default)]
struct Work {
    seeds: Vec<Seed>,
    edit: Option<(World, u64)>,
    cells: Vec<Key>,
    stop: bool,
}
struct Shared {
    work: Mutex<Work>,
    wake: Condvar,
    pending: AtomicBool,
    waiting: AtomicUsize,
}
pub struct LocalLight {
    shared: Arc<Shared>,
    pub results: mpsc::Receiver<Update>,
}
impl LocalLight {
    pub fn new(preview: Preview) -> Self {
        let shared = Arc::new(Shared {
            work: Mutex::new(Work::default()),
            wake: Condvar::new(),
            pending: AtomicBool::new(false),
            waiting: AtomicUsize::new(0),
        });
        let thread = shared.clone();
        let (tx, results) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("local-light-edits".into())
            .spawn(move || run(preview, thread, tx))
            .expect("local lighting worker");
        Self { shared, results }
    }
    pub fn seed(&self, seed: Seed) {
        let mut work = self.shared.work.lock().unwrap();
        work.seeds.retain(|s| s.key != seed.key);
        if work.seeds.len() >= 8 {
            work.seeds.remove(0);
        }
        work.seeds.push(seed);
        self.shared.wake.notify_one();
    }
    pub fn edit(&self, world: &World, cells: &[Key], revision: u64) {
        let mut work = self.shared.work.lock().unwrap();
        work.edit = Some((world.clone(), revision));
        work.cells.extend_from_slice(cells);
        self.shared.pending.store(true, Ordering::Release);
        self.shared.wake.notify_one();
    }
    pub fn received(&self) {
        self.shared.waiting.fetch_sub(1, Ordering::AcqRel);
    }
    pub fn pending(&self) -> bool {
        self.shared.pending.load(Ordering::Acquire)
            || self.shared.waiting.load(Ordering::Acquire) > 0
    }
}
impl Drop for LocalLight {
    fn drop(&mut self) {
        let mut work = self.shared.work.lock().unwrap_or_else(|p| p.into_inner());
        work.stop = true;
        self.shared.wake.notify_one();
    }
}
fn run(preview: Preview, shared: Arc<Shared>, tx: mpsc::SyncSender<Update>) {
    let generator = Generator::new(preview.seed).expect("local light generator");
    let mut sampler = generator.sampler();
    let mut sky = Sky::default();
    let mut tiles = Tiles::default();
    let mut fields = HashMap::<Key, (LightField, u64)>::new();
    let mut applied = 0;
    loop {
        let mut work = shared.work.lock().unwrap();
        while !work.stop && work.edit.is_none() && work.seeds.is_empty() {
            work = shared.wake.wait(work).unwrap();
        }
        if work.stop {
            return;
        }
        for seed in work.seeds.drain(..) {
            if seed.revision < applied {
                continue;
            }
            fields.insert(seed.key, (seed.field, seed.revision));
            if fields.len() > 8 {
                let far = *fields
                    .keys()
                    .max_by(|a, b| {
                        terrain_region::center(**a)
                            .distance_squared(seed.focus)
                            .total_cmp(&terrain_region::center(**b).distance_squared(seed.focus))
                    })
                    .unwrap();
                fields.remove(&far);
            }
        }
        let edit = work.edit.take();
        let cells = std::mem::take(&mut work.cells);
        drop(work);
        let Some((world, revision)) = edit else {
            continue;
        };
        if cells.is_empty() {
            shared.pending.store(false, Ordering::Release);
            continue;
        }
        sky.invalidate(&cells);
        tiles.invalidate(&cells);
        let target = cells[0].map(|v| v.div_euclid(128));
        let mut keys: Vec<_> = fields.keys().copied().collect();
        keys.sort_by_key(|k| {
            k.iter()
                .zip(target)
                .map(|(a, b)| (a - b).abs())
                .sum::<i32>()
        });
        for key in keys {
            let (field, rev) = fields.get_mut(&key).unwrap();
            if *rev > revision {
                continue;
            }
            let changed = field
                .update_sampled(
                    &world,
                    &cells,
                    &mut Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false),
                )
                .is_some();
            *rev = revision;
            if changed {
                shared.waiting.fetch_add(1, Ordering::AcqRel);
            }
            if changed
                && tx
                    .send(Update {
                        key,
                        pixels: field.crop([31; 3], [34; 3]),
                        revision,
                    })
                    .is_err()
            {
                return;
            }
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = fields.entry(target) {
            let origin = target.map(|v| v * 128 - 128);
            if let Some(field) = LightField::build_sampled(
                &world,
                origin,
                [96; 3],
                &mut Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false),
            ) {
                let pixels = field.crop([31; 3], [34; 3]);
                entry.insert((field, revision));
                shared.waiting.fetch_add(1, Ordering::AcqRel);
                if tx
                    .send(Update {
                        key: target,
                        pixels,
                        revision,
                    })
                    .is_err()
                {
                    return;
                }
            }
        }
        while fields.len() > 8 {
            let far = *fields
                .keys()
                .max_by_key(|k| {
                    k.iter()
                        .zip(target)
                        .map(|(a, b)| (a - b).abs())
                        .sum::<i32>()
                })
                .unwrap();
            fields.remove(&far);
        }
        applied = revision;
        let work = shared.work.lock().unwrap();
        if work.edit.is_none() {
            shared.pending.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn local_lamp_edits_repair_cached_fields_while_terrain_is_busy() {
        let preview = Preview::default();
        let generator = Generator::new(preview.seed).unwrap();
        let mut world = World::with_height(384 * 16);
        world.infinite = true;
        world.noise_preview = Some(preview);
        let key = [5, 40, 5];
        let origin = key.map(|v| v * 128 - 128);
        let mut sampler = generator.sampler();
        let mut sky = Sky::default();
        let mut tiles = Tiles::default();
        let mut field = LightField::build_sampled(
            &world,
            origin,
            [96; 3],
            &mut Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false),
        )
        .unwrap();
        field.prepare_edits(&mut Source::new(
            &world,
            &mut sampler,
            &mut sky,
            &mut tiles,
            &|| false,
        ));
        let terrain = crate::terrain_stream::Stream::new(
            world.clone(),
            Sky::default(),
            terrain_region::center([0, 15, 2]),
            12.0,
            HashMap::new(),
        );
        let local = LocalLight::new(preview);
        local.seed(Seed {
            key,
            field,
            revision: 0,
            focus: terrain_region::center(key),
        });
        let cell = key.map(|v| v * 128 + 66);
        for (revision, material) in [
            (1, crate::world::LIGHT),
            (2, crate::world::AIR),
            (3, crate::world::STONE),
            (4, crate::world::GLASS),
        ] {
            world.set(cell[0], cell[1], cell[2], material);
            let started = Instant::now();
            local.edit(&world, &[cell], revision);
            let update = local
                .results
                .recv_timeout(Duration::from_secs(10))
                .expect("local edits must not wait for terrain results to be consumed");
            local.received();
            assert_eq!(update.key, key);
            assert_eq!(update.revision, revision);
            println!(
                "LOCAL_EDIT {revision} {:.3} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
            let expected = LightField::build_sampled(
                &world,
                origin,
                [96; 3],
                &mut Source::new(
                    &world,
                    &mut sampler,
                    &mut Sky::default(),
                    &mut Tiles::default(),
                    &|| false,
                ),
            )
            .unwrap();
            assert_eq!(update.pixels, expected.crop([31; 3], [34; 3]));
        }
        assert!(terrain.pending());
    }
}
