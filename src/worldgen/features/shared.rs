//! Workers share immutable deterministic recipes. Never hold a cache lock while
//! sampling terrain, and let independent keys generate concurrently.
use super::*;
use std::sync::{Mutex, OnceLock};
type Slot = Arc<OnceLock<Arc<Vec<Primitive>>>>;
type Plans<const N: usize> = Mutex<crate::cache::Cache<[i32; 3], Slot, N>>;
#[derive(Default)]
pub(crate) struct Shared {
    areas: Plans<512>,
    caves: Plans<2048>,
    ores: Plans<1024>,
}
fn cached<const N: usize>(
    cache: &Plans<N>,
    key: [i32; 3],
    build: impl FnOnce() -> Vec<Primitive>,
) -> Arc<Vec<Primitive>> {
    let slot = {
        let mut cache = cache.lock().unwrap();
        if let Some(slot) = cache.get(&key) {
            slot.clone()
        } else {
            let slot = Arc::new(OnceLock::new());
            cache.insert(key, slot.clone());
            slot
        }
    };
    // Workers often request the same neighbouring feature area together.
    // Coordinate that one build without serializing independent areas.
    slot.get_or_init(|| Arc::new(build())).clone()
}

#[cfg(test)]
#[test]
fn concurrent_requests_build_a_feature_plan_once() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    };
    let cache = Plans::<16>::default();
    let builds = AtomicUsize::new(0);
    let barrier = Barrier::new(4);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                barrier.wait();
                assert!(cached(&cache, [1, -2, 3], || {
                    builds.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    Vec::new()
                })
                .is_empty());
            });
        }
    });
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}
impl Shared {
    pub fn area(&self, sampler: &mut TerrainSampler<'_>, key: [i32; 2]) -> Arc<Vec<Primitive>> {
        cached(&self.areas, [key[0], 0, key[1]], || {
            surface_area(sampler, key)
        })
    }
    pub fn cave(&self, sampler: &mut TerrainSampler<'_>, key: [i32; 3]) -> Arc<Vec<Primitive>> {
        cached(&self.caves, key, || cave_area(sampler, key))
    }
    pub fn ore(
        &self,
        sampler: &mut TerrainSampler<'_>,
        key: [i32; 3],
        seed: i64,
    ) -> Arc<Vec<Primitive>> {
        cached(&self.ores, key, || ores::column(sampler, key, seed))
    }
}
