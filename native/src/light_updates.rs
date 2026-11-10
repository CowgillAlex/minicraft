use super::*;
use std::collections::HashSet;

#[derive(Default)]
struct Bounds(Option<([usize; 3], [usize; 3])>);
impl Bounds {
    fn include(&mut self, i: usize) {
        let p = position(i);
        let (min, max) = self.0.get_or_insert((p, p));
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
}
impl LightField {
    /// Repair changed connections and sources, then remove/refill only affected light paths.
    pub fn update(&mut self, world: &World, edits: &[[i32; 3]]) -> Option<LightPatch> {
        let mut dirty = [Vec::new(), Vec::new()];
        let mut targets = HashSet::new();
        let mut columns = HashSet::new();
        let bases: HashSet<_> = edits.iter().map(|cell| cell.map(|v| v / STEP)).collect();
        for base in bases {
            columns.insert([base[0] as usize, base[2] as usize]);
            for p in around(base, 1) {
                let i = index(p[0], p[1], p[2]);
                for axis in 0..3 {
                    if p[axis] + 1 >= SIZE[axis] {
                        continue;
                    }
                    let cost = edge_loss(world, p, axis);
                    if cost != self.edges[i][axis * 2 + 1] {
                        let next = i + STRIDES[axis];
                        self.edges[i][axis * 2 + 1] = cost;
                        self.edges[next][axis * 2] = cost;
                        for channel in &mut dirty {
                            channel.extend([i, next]);
                        }
                    }
                }
            }
            // Source-to-grid rays span at most two cells. Three cells cover every
            // seed that can depend on a changed voxel, including removed emitters.
            targets.extend(around(base, 3).map(|p| index(p[0], p[1], p[2])));
        }
        for [x, z] in columns {
            for (y, value) in sky_column(world, x, z).into_iter().enumerate() {
                let i = index(x, y, z);
                if self.seeds[i][1] != value {
                    self.seeds[i][1] = value;
                    dirty[1].push(i);
                }
            }
        }
        self.update_emitters(world, &targets, &mut dirty[0]);
        let mut bounds = Bounds::default();
        for (channel, mut nodes) in dirty.into_iter().enumerate() {
            nodes.sort_unstable();
            nodes.dedup();
            self.repair(channel, &nodes, &mut bounds);
        }
        let (origin, max) = bounds.0?;
        let size = std::array::from_fn(|axis| max[axis] - origin[axis] + 1);
        let mut pixels = Vec::with_capacity(size.iter().product::<usize>() * 2);
        for z in origin[2]..=max[2] {
            for y in origin[1]..=max[1] {
                let start = index(origin[0], y, z) * 2;
                pixels.extend_from_slice(&self.pixels[start..start + size[0] * 2]);
            }
        }
        Some(LightPatch {
            origin,
            size,
            pixels,
        })
    }
    fn update_emitters(&mut self, world: &World, targets: &HashSet<usize>, dirty: &mut Vec<usize>) {
        if targets.is_empty() {
            return;
        }
        let mut bounds = Bounds::default();
        let previous: Vec<_> = targets.iter().map(|&i| (i, self.seeds[i][0])).collect();
        for &i in targets {
            bounds.include(i);
            self.seeds[i][0] = 0;
        }
        let (min, max) = bounds.0.unwrap();
        for &cell in &world.emitters {
            let base = cell.map(|v| (v / STEP) as usize);
            if (0..3).any(|axis| base[axis] + 1 < min[axis] || base[axis] > max[axis] + 1) {
                continue;
            }
            emitter_seeds(world, cell, |i, value| {
                if targets.contains(&i) {
                    self.seeds[i][0] = self.seeds[i][0].max(value);
                }
            });
        }
        for (i, old) in previous {
            if self.seeds[i][0] != old {
                dirty.push(i);
            }
        }
    }
    fn repair(&mut self, channel: usize, dirty: &[usize], bounds: &mut Bounds) {
        if dirty.is_empty() {
            return;
        }
        let decay = [BLOCK_FALLOFF, SKY_FALLOFF][channel];
        let mut buckets: Buckets = std::array::from_fn(|_| Vec::new());
        let mut removed = Vec::with_capacity(dirty.len());
        for &i in dirty {
            let value = &mut self.pixels[i * 2 + channel];
            removed.push((i, *value));
            *value = 0;
        }
        let mut cursor = 0;
        while cursor < removed.len() {
            let (i, old) = removed[cursor];
            cursor += 1;
            for direction in 0..6 {
                if self.edges[i][direction] == BLOCKED {
                    continue;
                }
                let next = neighbor(i, direction);
                let value = &mut self.pixels[next * 2 + channel];
                if *value == 0 {
                    continue;
                }
                // A light path strictly decreases. Lower neighbors may depend on
                // this node; equal/brighter neighbors can supply the refill.
                if *value < old {
                    removed.push((next, *value));
                    *value = 0;
                } else if *value > decay {
                    buckets[*value as usize].push(next);
                }
            }
        }
        for &(i, _) in &removed {
            let value = self.seeds[i][channel];
            self.pixels[i * 2 + channel] = value;
            if value > decay {
                buckets[value as usize].push(i);
            }
        }
        self.propagate(channel, decay, &mut buckets, |i| bounds.include(i));
        for (i, old) in removed {
            if self.pixels[i * 2 + channel] != old {
                bounds.include(i);
            }
        }
    }
}
fn around(base: [i32; 3], radius: i32) -> impl Iterator<Item = [usize; 3]> {
    let min: [usize; 3] = std::array::from_fn(|axis| (base[axis] - radius).max(0) as usize);
    let max: [usize; 3] =
        std::array::from_fn(|axis| (base[axis] + radius).min(SIZE[axis] as i32 - 1) as usize);
    (min[2]..=max[2]).flat_map(move |z| {
        (min[1]..=max[1]).flat_map(move |y| (min[0]..=max[0]).map(move |x| [x, y, z]))
    })
}
