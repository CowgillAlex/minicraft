//! Bounded second-chance caches retain recently used work without global resets.
use rustc_hash::FxHashMap;
use std::hash::Hash;
struct Entry<K, V> {
    key: K,
    value: V,
    used: bool,
}
pub struct Cache<K, V, const N: usize> {
    // Keys are internal coordinates/IDs, so use integer hashing rather than
    // paying for cryptographic hashing in every terrain and lighting query.
    index: FxHashMap<K, usize>,
    slots: Vec<Option<Entry<K, V>>>,
    free: Vec<usize>,
    hand: usize,
}
impl<K, V, const N: usize> Default for Cache<K, V, N> {
    fn default() -> Self {
        Self {
            index: FxHashMap::default(),
            slots: Vec::new(),
            free: Vec::new(),
            hand: 0,
        }
    }
}
impl<K: Copy + Eq + Hash, V, const N: usize> Cache<K, V, N> {
    pub fn contains_key(&self, key: &K) -> bool {
        self.index.contains_key(key)
    }
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let slot = *self.index.get(key)?;
        let entry = self.slots[slot].as_mut().unwrap();
        entry.used = true;
        Some(&entry.value)
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let slot = self.index.remove(key)?;
        self.free.push(slot);
        Some(self.slots[slot].take().unwrap().value)
    }
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(&slot) = self.index.get(&key) {
            self.slots[slot] = Some(Entry {
                key,
                value,
                used: true,
            });
            return;
        }
        let slot = if let Some(slot) = self.free.pop() {
            slot
        } else if self.slots.len() < N {
            self.slots.push(None);
            self.slots.len() - 1
        } else {
            loop {
                let slot = self.hand;
                self.hand = (self.hand + 1) % N;
                let entry = self.slots[slot].as_mut().unwrap();
                if entry.used {
                    entry.used = false;
                } else {
                    self.index.remove(&entry.key);
                    break slot;
                }
            }
        };
        self.slots[slot] = Some(Entry {
            key,
            value,
            used: true,
        });
        self.index.insert(key, slot);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reuse_survives_pressure_and_removal_reuses_slots() {
        let mut c = Cache::<i32, i32, 3>::default();
        for i in 0..3 {
            c.insert(i, i);
        }
        c.insert(3, 3); // first pass gives old entries a second chance
        assert_eq!(c.get(&2), Some(&2));
        c.insert(4, 4);
        assert_eq!(c.get(&2), Some(&2));
        for _ in 0..100 {
            let v = c.remove(&2).unwrap();
            c.insert(2, v);
        }
        assert_eq!(c.slots.len(), 3);
        assert_eq!(c.index.len(), 3);
    }
}
