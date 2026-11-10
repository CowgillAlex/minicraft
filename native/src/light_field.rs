use crate::{
    ray::VoxelRay,
    world::{self, World, VOXEL},
};
use glam::Vec3;

#[path = "light_updates.rs"]
mod updates;

pub const STEP: i32 = 4;
const BLOCKED: u8 = 255;
const BLOCK_FALLOFF: u8 = 8;
const SKY_FALLOFF: u8 = 14;
type Buckets = [Vec<usize>; 256];

/// Connectivity and source values stay resident in the worker between edits.
pub struct LightField {
    grid: Grid,
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
#[derive(Clone, Copy)]
struct Grid {
    size: [usize; 3],
    strides: [usize; 3],
    origin: [i32; 3],
}
impl Grid {
    fn new(height: i32) -> Self {
        let size = [
            (world::WIDTH / STEP) as usize,
            (height / STEP) as usize,
            (world::DEPTH / STEP) as usize,
        ];
        Self::region([0; 3], size)
    }
    fn region(origin: [i32; 3], size: [usize; 3]) -> Self {
        Self {
            origin,
            size,
            strides: [1, size[0], size[0] * size[1]],
        }
    }
    fn center(self, p: [usize; 3]) -> Vec3 {
        World::meters(
            self.origin[0] + p[0] as i32 * STEP + STEP / 2,
            self.origin[1] + p[1] as i32 * STEP + STEP / 2,
            self.origin[2] + p[2] as i32 * STEP + STEP / 2,
        ) + Vec3::splat(VOXEL * 0.5)
    }
    fn index(self, x: usize, y: usize, z: usize) -> usize {
        x + self.size[0] * (y + self.size[1] * z)
    }
    fn position(self, i: usize) -> [usize; 3] {
        [
            i % self.size[0],
            i / self.size[0] % self.size[1],
            i / self.strides[2],
        ]
    }
    fn neighbor(self, i: usize, direction: usize) -> usize {
        if direction.is_multiple_of(2) {
            i - self.strides[direction / 2]
        } else {
            i + self.strides[direction / 2]
        }
    }
}
pub(crate) fn loss(material: u8) -> u8 {
    match material {
        world::AIR | world::LIGHT => 0,
        world::GLASS => 3,
        world::WATER => 1,
        _ => BLOCKED,
    }
}
#[cfg(test)]
pub fn segment_loss(world: &World, from: Vec3, to: Vec3) -> u8 {
    let direction = to - from;
    let mut total = 0_u8;
    let mut reader = world.reader();
    for (cell, _) in VoxelRay::new(from, direction, direction.length()) {
        let attenuation = loss(reader.get(cell));
        if attenuation == BLOCKED {
            return BLOCKED;
        }
        total = total.saturating_add(attenuation);
    }
    total
}
/// Neighbor connections are axis aligned; integer traversal visits the same fine voxels.
fn edge_loss(world: &World, grid: Grid, p: [usize; 3], axis: usize) -> u8 {
    edge_loss_read(&mut world.reader(), grid, p, axis)
}
fn edge_loss_read(
    reader: &mut crate::world::Reader<'_>,
    grid: Grid,
    p: [usize; 3],
    axis: usize,
) -> u8 {
    let mut cell =
        std::array::from_fn::<_, 3, _>(|i| grid.origin[i] + p[i] as i32 * STEP + STEP / 2);
    let mut total = 0_u8;
    for _ in 0..=STEP {
        let attenuation = loss(reader.get(cell));
        if attenuation == BLOCKED {
            return BLOCKED;
        }
        total = total.saturating_add(attenuation);
        cell[axis] += 1;
    }
    total
}
fn sky_column(world: &World, x: usize, z: usize) -> Vec<u8> {
    let grid = Grid::new(world.height());
    let mut seeds = vec![0; grid.size[1]];
    let mut value = 255_u8;
    let mut reader = world.reader();
    for y in (0..world.height()).rev() {
        let material = reader.get([x as i32 * STEP + STEP / 2, y, z as i32 * STEP + STEP / 2]);
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
fn emitter_seeds(world: &World, grid: Grid, cell: [i32; 3], visit: impl FnMut(usize, u8)) {
    emitter_seeds_read(grid, cell, |p| world.get(p[0], p[1], p[2]), visit);
}
fn emitter_seeds_read(
    grid: Grid,
    cell: [i32; 3],
    mut read: impl FnMut([i32; 3]) -> u8,
    mut visit: impl FnMut(usize, u8),
) {
    let [x, y, z] = cell;
    let source = World::meters(x, y, z) + Vec3::splat(VOXEL * 0.5);
    let base = std::array::from_fn::<_, 3, _>(|i| (cell[i] - grid.origin[i]).div_euclid(STEP));
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = [base[0] + dx, base[1] + dy, base[2] + dz];
                if p.iter()
                    .enumerate()
                    .any(|(axis, v)| *v < 0 || *v >= grid.size[axis] as i32)
                {
                    continue;
                }
                let p = p.map(|v| v as usize);
                let target = grid.center(p);
                let direction = target - source;
                let mut cost = 0_u8;
                for (p, _) in VoxelRay::new(source, direction, direction.length()) {
                    cost = cost.saturating_add(loss(read(p)));
                    if cost == BLOCKED {
                        break;
                    }
                }
                if cost == BLOCKED {
                    continue;
                }
                let distance = (target - source).abs().element_sum() / (STEP as f32 * VOXEL);
                let value = 255_u8
                    .saturating_sub((distance * BLOCK_FALLOFF as f32) as u8)
                    .saturating_sub(cost);
                visit(grid.index(p[0], p[1], p[2]), value);
            }
        }
    }
}
pub trait LightSource {
    fn prepare_sky(&mut self, _origin: [i32; 3], _size: [usize; 3]) {}
    fn prepare_edges(&mut self, _origin: [i32; 3], _size: [usize; 3]) {}
    fn cancelled(&self) -> bool;
    fn material(&mut self, cell: [i32; 3]) -> u8;
    fn sky(&mut self, point: [i32; 2], min_y: i32, len: usize) -> Vec<u8>;
    fn edge(&mut self, point: [i32; 3], axis: usize) -> u8;
}

impl LightField {
    /// Keep an editable graph ready even when uniform illumination needed no flood.
    pub fn prepare_edits(&mut self, source: &mut impl LightSource) {
        if !self.edges.is_empty() {
            return;
        }
        let grid = self.grid;
        source.prepare_edges(grid.origin, grid.size);
        self.edges = vec![[BLOCKED; 6]; self.seeds.len()];
        self.connect(|p, axis| {
            source.edge(
                std::array::from_fn(|i| grid.origin[i] + p[i] as i32 * STEP + STEP / 2),
                axis,
            )
        });
    }
    pub fn size(&self) -> [usize; 3] {
        self.grid.size
    }
    pub fn build(world: &World) -> Self {
        let grid = Grid::new(world.height());
        Self::build_grid(world, grid, false, |x, z| sky_column(world, x, z))
    }
    pub fn dummy() -> Self {
        Self {
            grid: Grid::region([0; 3], [1; 3]),
            pixels: vec![0; 2],
            edges: vec![[BLOCKED; 6]],
            seeds: vec![[0; 2]],
        }
    }
    #[cfg(test)]
    pub fn build_region(
        world: &World,
        origin: [i32; 3],
        size: [usize; 3],
        mut sky: impl FnMut([i32; 2]) -> Vec<u8>,
    ) -> Self {
        let grid = Grid::region(origin, size);
        Self::build_grid(world, grid, true, |x, z| {
            sky([
                origin[0] + x as i32 * STEP + STEP / 2,
                origin[2] + z as i32 * STEP + STEP / 2,
            ])
        })
    }
    pub fn crop(&self, origin: [usize; 3], size: [usize; 3]) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(size.iter().product::<usize>() * 2);
        for z in origin[2]..origin[2] + size[2] {
            for y in origin[1]..origin[1] + size[1] {
                let i = self.grid.index(origin[0], y, z) * 2;
                pixels.extend_from_slice(&self.pixels[i..i + size[0] * 2]);
            }
        }
        pixels
    }
    fn build_grid(
        world: &World,
        grid: Grid,
        quick: bool,
        mut sky: impl FnMut(usize, usize) -> Vec<u8>,
    ) -> Self {
        let count = grid.size.iter().product::<usize>();
        let mut seeds = vec![[0; 2]; count];
        for z in 0..grid.size[2] {
            for x in 0..grid.size[0] {
                for (y, value) in sky(x, z).into_iter().enumerate() {
                    seeds[grid.index(x, y, z)][1] = value;
                }
            }
        }
        for &cell in &world.emitters {
            emitter_seeds(world, grid, cell, |i, value| {
                seeds[i][0] = seeds[i][0].max(value)
            });
        }
        if quick && (seeds.iter().all(|s| *s == [0, 0]) || seeds.iter().all(|s| *s == [0, 255])) {
            return Self {
                grid,
                pixels: seeds.iter().flatten().copied().collect(),
                edges: Vec::new(),
                seeds: Vec::new(),
            };
        }
        let mut field = Self {
            grid,
            pixels: seeds.iter().flatten().copied().collect(),
            edges: vec![[BLOCKED; 6]; count],
            seeds,
        };
        let mut reader = world.reader();
        field.connect(|p, axis| edge_loss_read(&mut reader, grid, p, axis));
        field.fill();
        field
    }
    fn connect(&mut self, mut edge: impl FnMut([usize; 3], usize) -> u8) {
        let grid = self.grid;
        for z in 0..grid.size[2] {
            for y in 0..grid.size[1] {
                for x in 0..grid.size[0] {
                    let p = [x, y, z];
                    let i = grid.index(x, y, z);
                    for axis in 0..3 {
                        if p[axis] + 1 < grid.size[axis] {
                            let cost = edge(p, axis);
                            self.edges[i][axis * 2 + 1] = cost;
                            self.edges[i + grid.strides[axis]][axis * 2] = cost;
                        }
                    }
                }
            }
        }
    }
    fn fill(&mut self) {
        for (channel, decay) in [BLOCK_FALLOFF, SKY_FALLOFF].into_iter().enumerate() {
            let mut buckets: Buckets = std::array::from_fn(|_| Vec::new());
            for (i, seed) in self.seeds.iter().enumerate() {
                if seed[channel] > decay {
                    buckets[seed[channel] as usize].push(i);
                }
            }
            self.propagate(channel, decay, &mut buckets, |_| {});
        }
    }
    /// Reuse transport tiles shared by adjacent lighting regions.
    pub fn build_sampled(
        world: &World,
        origin: [i32; 3],
        size: [usize; 3],
        source: &mut impl LightSource,
    ) -> Option<Self> {
        let profile_start = std::time::Instant::now();
        let grid = Grid::region(origin, size);
        let count = size.iter().product::<usize>();
        let mut field = Self {
            grid,
            pixels: vec![0; count * 2],
            edges: Vec::new(),
            seeds: vec![[0; 2]; count],
        };
        source.prepare_sky(origin, size);
        for z in 0..size[2] {
            for x in 0..size[0] {
                if source.cancelled() {
                    return None;
                }
                let point = [
                    origin[0] + x as i32 * STEP + STEP / 2,
                    origin[2] + z as i32 * STEP + STEP / 2,
                ];
                for (y, value) in source
                    .sky(point, origin[1], size[1])
                    .into_iter()
                    .enumerate()
                {
                    field.seeds[grid.index(x, y, z)][1] = value;
                }
            }
        }
        for &cell in &world.emitters {
            emitter_seeds_read(
                grid,
                cell,
                |p| source.material(p),
                |i, value| field.seeds[i][0] = field.seeds[i][0].max(value),
            );
        }
        let sky_time = profile_start.elapsed().as_secs_f64();
        field.pixels = field.seeds.iter().flatten().copied().collect();
        if field.seeds.iter().all(|s| *s == [0, 0]) || field.seeds.iter().all(|s| *s == [0, 255]) {
            return Some(field);
        }
        source.prepare_edges(origin, size);
        field.edges = vec![[BLOCKED; 6]; count];
        for z in 0..size[2] {
            if source.cancelled() {
                return None;
            }
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let p = [x, y, z];
                    let i = grid.index(x, y, z);
                    for axis in 0..3 {
                        if p[axis] + 1 < size[axis] {
                            let cost = source.edge(
                                std::array::from_fn(|a| origin[a] + p[a] as i32 * STEP + STEP / 2),
                                axis,
                            );
                            field.edges[i][axis * 2 + 1] = cost;
                            field.edges[i + grid.strides[axis]][axis * 2] = cost;
                        }
                    }
                }
            }
        }
        if source.cancelled() {
            return None;
        }
        let edges_time = profile_start.elapsed().as_secs_f64() - sky_time;
        field.fill();
        if std::env::var_os("MINICRAFT_PROFILE_STREAM").is_some() {
            eprintln!(
                "LIGHT sky={sky_time:.4} edges={edges_time:.4} fill={:.4}",
                profile_start.elapsed().as_secs_f64() - sky_time - edges_time
            );
        }
        Some(field)
    }
    fn propagate(
        &mut self,
        channel: usize,
        decay: u8,
        buckets: &mut Buckets,
        mut changed: impl FnMut(usize),
    ) {
        let grid = self.grid;
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
                    let next = grid.neighbor(i, direction);
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
        let grid = self.grid;
        let p = cell.map(|v| (v / STEP) as usize);
        let i = grid.index(p[0], p[1], p[2]) * 2;
        [self.pixels[i], self.pixels[i + 1]]
    }
}
#[cfg(test)]
#[path = "light_tests.rs"]
mod tests;
