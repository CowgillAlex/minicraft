//! Workers share immutable deterministic recipes. Never hold a cache lock while
//! sampling terrain, and let independent keys generate concurrently.
use super::*;
use std::sync::Mutex;
type Plans<const N: usize> = Mutex<crate::cache::Cache<[i32; 3], Arc<Vec<Primitive>>, N>>;
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
    if let Some(value) = cache.lock().unwrap().get(&key) {
        return value.clone();
    }
    let value = Arc::new(build());
    cache.lock().unwrap().insert(key, value.clone());
    value
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
