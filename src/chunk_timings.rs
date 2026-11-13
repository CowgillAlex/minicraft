//! Shared, bounded measurements survive worker snapshots without copying voxel data.
use std::sync::{Arc, Mutex};
pub type Key = [i32; 3];
pub type Shared = Arc<Mutex<Store>>;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    pub generation_ms: Option<f32>,
    pub mesh_ms: Option<f32>,
    pub source_cached: bool,
    pub mesh_cached: bool,
    pub pipeline: Option<crate::stream_metrics::Latency>,
}
#[derive(Default)]
pub struct Store {
    entries: crate::cache::Cache<Key, Timing, 8192>,
}
impl Store {
    pub fn get(&mut self, key: Key) -> Timing {
        self.entries.get(&key).copied().unwrap_or_default()
    }
    pub fn generation(&mut self, key: Key, ms: f32) {
        self.entries.insert(
            key,
            Timing {
                generation_ms: Some(ms),
                ..Default::default()
            },
        );
    }
    pub fn mesh(&mut self, key: Key, ms: f32, source_cached: bool) {
        let mut timing = self.get(key);
        timing.mesh_ms = Some(ms);
        timing.source_cached = source_cached;
        timing.mesh_cached = false;
        timing.pipeline = None;
        self.entries.insert(key, timing);
    }
    pub fn reuse(&mut self, key: Key, mut timing: Timing) {
        timing.generation_ms = self.get(key).generation_ms.or(timing.generation_ms);
        timing.mesh_cached = true;
        timing.pipeline = None;
        self.entries.insert(key, timing);
    }
    pub fn region(&mut self, key: Key, latency: crate::stream_metrics::Latency) {
        for y in 0..4 {
            for z in 0..4 {
                for x in 0..4 {
                    let page = [key[0] * 4 + x, key[1] * 4 + y, key[2] * 4 + z];
                    let mut timing = self.get(page);
                    timing.pipeline = Some(latency);
                    self.entries.insert(page, timing);
                }
            }
        }
    }
}

#[cfg(test)]
#[test]
fn cache_reuse_keeps_build_cost_and_regeneration_clears_stale_mesh_data() {
    let mut store = Store::default();
    let key = [-12, 20, 8];
    assert_eq!(store.get(key).generation_ms, None);
    store.generation(key, 12.0);
    store.mesh(key, 3.0, false);
    let built = store.get(key);
    store.mesh(key, 0.3, true);
    store.reuse(key, built);
    let reused = store.get(key);
    assert_eq!(reused.mesh_ms, Some(3.0));
    assert_eq!(reused.generation_ms, Some(12.0));
    assert!(reused.mesh_cached);
    store.generation(key, 5.0);
    assert_eq!(store.get(key).mesh_ms, None);
    assert!(!store.get(key).mesh_cached);
}
