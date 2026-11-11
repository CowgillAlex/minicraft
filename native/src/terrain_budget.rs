//! Admission uses the current view, so a full cache can replace less useful
//! regions instead of permanently rejecting the cave the player is facing.
use crate::{
    terrain_lod::Level,
    terrain_region::{self, Key},
};
use glam::Vec3;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug)]
pub struct Loaded {
    pub bytes: usize,
    pub level: Level,
}
#[derive(Clone, Copy)]
pub struct View {
    pub position: Vec3,
    pub direction: Vec3,
    pub travel: Vec3,
    pub radius: f32,
}
impl View {
    pub fn priority(self, key: Key) -> f32 {
        let offset = terrain_region::center(key) - self.position;
        let distance = offset.length();
        if distance < 16.0 {
            return distance - 1000.0;
        }
        let facing = offset.normalize_or_zero().dot(self.direction);
        let ahead = offset.normalize_or_zero().dot(self.travel);
        let behind = if self.direction.length_squared() > 0.5 && facing < -0.1 {
            self.radius * 4.0
        } else {
            0.0
        };
        behind + distance * (1.3 - facing * 0.55 - ahead * 0.25)
    }
}
pub fn victims(
    loaded: &HashMap<Key, Loaded>,
    key: Key,
    bytes: usize,
    view: View,
    budget: usize,
) -> Option<Vec<Key>> {
    let mut total = loaded.values().map(|r| r.bytes).sum::<usize>()
        - loaded.get(&key).map_or(0, |r| r.bytes)
        + bytes;
    if total <= budget {
        return Some(Vec::new());
    }
    let near = terrain_region::center(key).distance(view.position) < 16.0;
    let mut candidates: Vec<_> = loaded
        .iter()
        .filter(|(k, _)| {
            **k != key
                && terrain_region::center(**k).distance(view.position) >= 16.0
                && (near || view.priority(**k) > view.priority(key))
        })
        .map(|(key, r)| (*key, r.bytes))
        .collect();
    candidates.sort_by(|(a, _), (b, _)| {
        view.priority(*b)
            .total_cmp(&view.priority(*a))
            .then_with(|| a.cmp(b))
    });
    let mut evicted = Vec::new();
    for (key, bytes) in candidates {
        if total <= budget {
            break;
        }
        total -= bytes;
        evicted.push(key);
    }
    (total <= budget || near).then_some(evicted)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_cache_changes_with_the_view_and_keeps_nearby_regions() {
        let eye = terrain_region::center([0, 10, 0]);
        let behind = [-5, 10, 0];
        let front = [5, 10, 0];
        let near = [0, 10, 0];
        let view = View {
            position: eye,
            direction: Vec3::X,
            travel: Vec3::ZERO,
            radius: 64.0,
        };
        let loaded = HashMap::from([
            (
                behind,
                Loaded {
                    bytes: 80,
                    level: Level::Full,
                },
            ),
            (
                near,
                Loaded {
                    bytes: 20,
                    level: Level::Full,
                },
            ),
        ]);
        assert_eq!(victims(&loaded, front, 80, view, 100), Some(vec![behind]));
        let loaded = HashMap::from([
            (
                front,
                Loaded {
                    bytes: 80,
                    level: Level::Full,
                },
            ),
            (
                near,
                Loaded {
                    bytes: 20,
                    level: Level::Full,
                },
            ),
        ]);
        assert!(victims(&loaded, behind, 80, view, 100).is_none());
        assert_eq!(
            victims(
                &loaded,
                behind,
                80,
                View {
                    direction: -Vec3::X,
                    ..view
                },
                100
            ),
            Some(vec![front])
        );
    }
}
