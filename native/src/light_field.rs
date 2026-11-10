use crate::{
    ray::VoxelRay,
    world::{self, World, VOXEL},
};
use glam::Vec3;

#[path = "light_updates.rs"]
mod updates;

pub const STEP: i32 = 4;
pub const SIZE: [usize; 3] = [
    (world::WIDTH / STEP) as usize,
    (world::HEIGHT / STEP) as usize,
    (world::DEPTH / STEP) as usize,
];
const STRIDES: [usize; 3] = [1, SIZE[0], SIZE[0] * SIZE[1]];
const BLOCKED: u8 = 255;
const BLOCK_FALLOFF: u8 = 8;
const SKY_FALLOFF: u8 = 14;
type Buckets = [Vec<usize>; 256];

/// Connectivity and source values stay resident in the worker between edits.
pub struct LightField {
    pub pixels: Vec<u8>,
    edges: Vec<[u8; 6]>,
    seeds: Vec<[u8; 2]>,
}
/// A tightly packed subvolume, ready for a partial GPU texture upload.
pub struct LightPatch {
    pub origin: [usize; 3],
    pub size: [usize; 3],
    pub pixels: Vec<u8>,
}
fn index(x: usize, y: usize, z: usize) -> usize {
    x + SIZE[0] * (y + SIZE[1] * z)
}
fn position(i: usize) -> [usize; 3] {
    [i % SIZE[0], i / SIZE[0] % SIZE[1], i / STRIDES[2]]
}
fn center(x: usize, y: usize, z: usize) -> Vec3 {
    World::meters(
        x as i32 * STEP + STEP / 2,
        y as i32 * STEP + STEP / 2,
        z as i32 * STEP + STEP / 2,
    ) + Vec3::splat(VOXEL * 0.5)
}
fn loss(material: u8) -> u8 {
    match material {
        world::AIR | world::LIGHT => 0,
        world::GLASS => 3,
        world::WATER => 6,
        _ => BLOCKED,
    }
}
pub fn segment_loss(world: &World, from: Vec3, to: Vec3) -> u8 {
    let direction = to - from;
    let mut total = 0_u8;
    for (cell, _) in VoxelRay::new(from, direction, direction.length()) {
        let attenuation = loss(world.get(cell[0], cell[1], cell[2]));
        if attenuation == BLOCKED {
            return BLOCKED;
        }
        total = total.saturating_add(attenuation);
    }
    total
}
/// Neighbor connections are axis aligned; integer traversal visits the same fine voxels.
fn edge_loss(world: &World, p: [usize; 3], axis: usize) -> u8 {
    let mut cell = p.map(|v| v as i32 * STEP + STEP / 2);
    let mut total = 0_u8;
    for _ in 0..=STEP {
        let attenuation = loss(world.get(cell[0], cell[1], cell[2]));
        if attenuation == BLOCKED {
            return BLOCKED;
        }
        total = total.saturating_add(attenuation);
        cell[axis] += 1;
    }
    total
}
fn sky_column(world: &World, x: usize, z: usize) -> [u8; SIZE[1]] {
    let mut seeds = [0; SIZE[1]];
    let mut value = 255_u8;
    for y in (0..world::HEIGHT).rev() {
        let material = world.get(x as i32 * STEP + STEP / 2, y, z as i32 * STEP + STEP / 2);
        if loss(material) == BLOCKED || material == world::LIGHT {
            break;
        }
        value = value.saturating_sub(loss(material));
        if y % STEP == STEP / 2 {
            seeds[y as usize / STEP as usize] = value;
        }
    }
    seeds
}
fn emitter_seeds(world: &World, cell: [i32; 3], mut visit: impl FnMut(usize, u8)) {
    let [x, y, z] = cell;
    let source = World::meters(x, y, z) + Vec3::splat(VOXEL * 0.5);
    let base = cell.map(|v| v / STEP);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = [base[0] + dx, base[1] + dy, base[2] + dz];
                if p.iter()
                    .enumerate()
                    .any(|(axis, v)| *v < 0 || *v >= SIZE[axis] as i32)
                {
                    continue;
                }
                let p = p.map(|v| v as usize);
                let target = center(p[0], p[1], p[2]);
                let cost = segment_loss(world, source, target);
                if cost == BLOCKED {
                    continue;
                }
                let distance = (target - source).abs().element_sum() / (STEP as f32 * VOXEL);
                let value = 255_u8
                    .saturating_sub((distance * BLOCK_FALLOFF as f32) as u8)
                    .saturating_sub(cost);
                visit(index(p[0], p[1], p[2]), value);
            }
        }
    }
}
impl LightField {
    pub fn build(world: &World) -> Self {
        let count = SIZE.iter().product::<usize>();
        let mut edges = vec![[BLOCKED; 6]; count];
        for z in 0..SIZE[2] {
            for y in 0..SIZE[1] {
                for x in 0..SIZE[0] {
                    let p = [x, y, z];
                    let i = index(x, y, z);
                    for axis in 0..3 {
                        if p[axis] + 1 < SIZE[axis] {
                            let cost = edge_loss(world, p, axis);
                            edges[i][axis * 2 + 1] = cost;
                            edges[i + STRIDES[axis]][axis * 2] = cost;
                        }
                    }
                }
            }
        }
        let mut seeds = vec![[0; 2]; count];
        for z in 0..SIZE[2] {
            for x in 0..SIZE[0] {
                for (y, value) in sky_column(world, x, z).into_iter().enumerate() {
                    seeds[index(x, y, z)][1] = value;
                }
            }
        }
        for &cell in &world.emitters {
            emitter_seeds(world, cell, |i, value| seeds[i][0] = seeds[i][0].max(value));
        }
        let pixels = seeds.iter().flatten().copied().collect();
        let mut field = Self {
            pixels,
            edges,
            seeds,
        };
        for (channel, decay) in [BLOCK_FALLOFF, SKY_FALLOFF].into_iter().enumerate() {
            let mut buckets: Buckets = std::array::from_fn(|_| Vec::new());
            for (i, seed) in field.seeds.iter().enumerate() {
                if seed[channel] > decay {
                    buckets[seed[channel] as usize].push(i);
                }
            }
            field.propagate(channel, decay, &mut buckets, |_| {});
        }
        field
    }
    fn propagate(
        &mut self,
        channel: usize,
        decay: u8,
        buckets: &mut Buckets,
        mut changed: impl FnMut(usize),
    ) {
        // Brightest-first buckets visit a node only after its strongest path is known.
        for brightness in (1..256).rev() {
            while let Some(i) = buckets[brightness].pop() {
                if self.pixels[i * 2 + channel] as usize != brightness {
                    continue;
                }
                for direction in 0..6 {
                    let cost = self.edges[i][direction];
                    if cost == BLOCKED {
                        continue;
                    }
                    let candidate = (brightness as u8).saturating_sub(decay.saturating_add(cost));
                    let next = neighbor(i, direction);
                    let value = &mut self.pixels[next * 2 + channel];
                    if candidate > *value {
                        *value = candidate;
                        changed(next);
                        if candidate > decay {
                            buckets[candidate as usize].push(next);
                        }
                    }
                }
            }
        }
    }
    #[cfg(test)]
    pub fn at(&self, cell: [i32; 3]) -> [u8; 2] {
        let p = cell.map(|v| (v / STEP) as usize);
        let i = index(p[0], p[1], p[2]) * 2;
        [self.pixels[i], self.pixels[i + 1]]
    }
}
fn neighbor(i: usize, direction: usize) -> usize {
    if direction.is_multiple_of(2) {
        i - STRIDES[direction / 2]
    } else {
        i + STRIDES[direction / 2]
    }
}
#[cfg(test)]
#[path = "light_tests.rs"]
mod tests;
