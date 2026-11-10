//! Quarter-metre transport tiles sample only the fine-voxel rays lighting uses.
//! They retain the same endpoint attenuation and thin-wall tests as full bakes.
use crate::{
    light_field::{self, LightSource},
    world::{self, World},
    worldgen::{sky::Sky, TerrainSampler},
};
use std::{collections::HashMap, sync::Arc};
type Key = [i32; 3];
const COUNT: usize = 8 * 8 * 8 * 3;
#[derive(Default)]
pub struct Tiles {
    entries: HashMap<Key, Arc<[u8; COUNT]>>,
}
impl Tiles {
    pub fn invalidate(&mut self, cells: &[Key]) {
        for p in cells {
            // A positive edge reaches two fine voxels into the next page.
            for x in -1..=0 {
                for y in -1..=0 {
                    for z in -1..=0 {
                        self.entries.remove(&[
                            p[0].div_euclid(32) + x,
                            p[1].div_euclid(32) + y,
                            p[2].div_euclid(32) + z,
                        ]);
                    }
                }
            }
        }
    }
}
pub struct Source<'a, 'b, 'g> {
    pub world: &'a World,
    pub sampler: &'a mut TerrainSampler<'g>,
    pub sky: &'a mut Sky,
    pub tiles: &'a mut Tiles,
    pub cancel: &'b dyn Fn() -> bool,
    last: Option<(Key, Arc<[u8; COUNT]>)>,
    pool: Option<&'a crate::terrain_pages::Pages>,
}
impl<'a, 'b, 'g> Source<'a, 'b, 'g> {
    pub fn new(
        world: &'a World,
        sampler: &'a mut TerrainSampler<'g>,
        sky: &'a mut Sky,
        tiles: &'a mut Tiles,
        cancel: &'b dyn Fn() -> bool,
    ) -> Self {
        Self {
            world,
            sampler,
            sky,
            tiles,
            cancel,
            last: None,
            pool: None,
        }
    }
    pub fn parallel(mut self, pool: &'a crate::terrain_pages::Pages) -> Self {
        self.pool = Some(pool);
        self
    }
    fn tile(&mut self, key: Key) -> Arc<[u8; COUNT]> {
        if self.tiles.entries.len() >= 16384 {
            self.tiles.entries.clear();
        }
        let tile = if let Some(tile) = self.tiles.entries.get(&key) {
            tile.clone()
        } else {
            let tile = generate(self.world, self.sampler, key);
            self.tiles.entries.insert(key, tile.clone());
            tile
        };
        self.last = Some((key, tile.clone()));
        tile
    }
}
impl LightSource for Source<'_, '_, '_> {
    fn prepare_sky(&mut self, origin: Key, size: [usize; 3]) {
        if let Some(pool) = self.pool {
            let points = self.sky.missing(origin, size);
            for (point, column) in pool.sky(self.world, points, self.cancel) {
                self.sky.insert(point, column);
            }
        }
    }
    fn prepare_edges(&mut self, origin: Key, size: [usize; 3]) {
        if let Some(pool) = self.pool {
            let lo = origin.map(|v| v.div_euclid(32));
            let hi = std::array::from_fn::<_, 3, _>(|i| {
                (origin[i] + size[i] as i32 * 4 - 1).div_euclid(32)
            });
            let mut keys = Vec::new();
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        let k = [x, y, z];
                        if !self.tiles.entries.contains_key(&k) {
                            keys.push(k);
                        }
                    }
                }
            }
            if self.tiles.entries.len() + keys.len() > 16384 {
                self.tiles.entries.clear();
            }
            for (key, tile) in pool.tiles(self.world, keys, self.cancel) {
                self.tiles.entries.insert(key, tile);
            }
        }
    }
    fn material(&mut self, p: Key) -> u8 {
        if !self.world.contains(p) {
            world::AIR
        } else if self.world.is_loaded(p) {
            self.world.get(p[0], p[1], p[2])
        } else if let Some(m) = self.world.override_at(p) {
            m
        } else {
            self.sampler
                .material_fast(self.world.noise_preview.unwrap().point(p))
        }
    }
    fn sky(&mut self, p: [i32; 2], min_y: i32, len: usize) -> Vec<u8> {
        self.sky.values(self.world, self.sampler, p, min_y, len)
    }
    fn edge(&mut self, p: Key, axis: usize) -> u8 {
        let key = p.map(|v| v.div_euclid(32));
        let local = p.map(|v| v.rem_euclid(32) as usize / 4);
        let index = (local[0] + 8 * (local[1] + 8 * local[2])) * 3 + axis;
        if let Some((k, tile)) = &self.last {
            if *k == key {
                return tile[index];
            }
        }
        self.tile(key)[index]
    }
    fn cancelled(&self) -> bool {
        (self.cancel)()
    }
}

pub(crate) fn generate(
    world: &World,
    sampler: &mut TerrainSampler<'_>,
    key: Key,
) -> Arc<[u8; COUNT]> {
    if key[1] * 32 + 34 < 0 || key[1] * 32 + 2 >= world.height() {
        return Arc::new([0; COUNT]);
    }
    let origin = key.map(|v| v * 32);
    let preview = world.noise_preview.expect("source lighting");
    let mut edges = [255; COUNT];
    let min = origin.map(|v| v + 2);
    let max = origin.map(|v| v + 34);
    let uniform = if !world.contains(min) || !world.contains(max) || world.has_changes(min, max) {
        None
    } else {
        sampler.uniform_material(preview.point(min), preview.point(max))
    };
    let core_max = origin.map(|v| v + 30);
    let core =
        if !world.contains(min) || !world.contains(core_max) || world.has_changes(min, core_max) {
            None
        } else {
            sampler.uniform_material(preview.point(min), preview.point(core_max))
        };
    if let Some(material) = uniform {
        edges.fill(light_field::loss(material).saturating_mul(5));
    } else if core.is_some_and(|m| light_field::loss(m) == 255) {
        edges.fill(255);
    } else {
        let mut values = vec![255; 35_usize.pow(3)];
        let mut reader = world.reader();
        for y in 0..8 {
            for z in 0..8 {
                for x in 0..8 {
                    for axis in 0..3 {
                        let mut local = [x * 4 + 2, y * 4 + 2, z * 4 + 2];
                        if let Some(material) = core {
                            if [x, y, z][axis] < 7 {
                                edges[(x + 8 * (y + 8 * z)) * 3 + axis] =
                                    light_field::loss(material).saturating_mul(5);
                                continue;
                            }
                        }
                        let mut cost = 0_u8;
                        for _ in 0..=4 {
                            let index = local[0] + 35 * (local[2] + 35 * local[1]);
                            let material = if values[index] != 255 {
                                values[index]
                            } else {
                                let p = std::array::from_fn(|i| origin[i] + local[i] as i32);
                                let m = if !world.contains(p) {
                                    world::AIR
                                } else if world.is_loaded(p) {
                                    reader.get(p)
                                } else if let Some(m) = world.override_at(p) {
                                    m
                                } else {
                                    sampler.material_fast(preview.point(p))
                                };
                                values[index] = m;
                                m
                            };
                            cost = cost.saturating_add(light_field::loss(material));
                            if cost == 255 {
                                break;
                            }
                            local[axis] += 1;
                        }
                        edges[(x + 8 * (y + 8 * z)) * 3 + axis] = cost;
                    }
                }
            }
        }
    }
    Arc::new(edges)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::{preview::Preview, Generator};
    #[test]
    fn sparse_transport_matches_fine_rays_with_edits_and_missing_pages() {
        let preview = Preview {
            origin: [134, 20, -37],
            height_m: 48,
            ..Default::default()
        };
        let generator = Generator::new(1).unwrap();
        let mut sampler = generator.sampler();
        let mut reference = generator.sampler();
        let mut world = World::with_height(48 * 16);
        world.infinite = true;
        world.noise_preview = Some(preview);
        let page = crate::worldgen::page::generate(preview, &mut sampler, [-32, 96, 64]).data;
        world.insert_page([-1, 3, 2], page);
        world.set(-30, 101, 66, world::GLASS);
        world.set(-30, 102, 66, world::STONE);
        world.set(-30, 103, 66, world::LIGHT);
        let mut sky = Sky::default();
        let mut tiles = Tiles::default();
        let mut source = Source::new(&world, &mut sampler, &mut sky, &mut tiles, &|| false);
        for key in [[-1, 3, 2], [0, 0, 2], [2, 20, -3], [3, 23, 5]] {
            for y in 0..8 {
                for z in 0..8 {
                    for x in 0..8 {
                        let p = [
                            key[0] * 32 + x * 4 + 2,
                            key[1] * 32 + y * 4 + 2,
                            key[2] * 32 + z * 4 + 2,
                        ];
                        for axis in 0..3 {
                            let mut cost = 0_u8;
                            for i in 0..=4 {
                                let mut q = p;
                                q[axis] += i;
                                let m = if !world.contains(q) {
                                    world::AIR
                                } else {
                                    world
                                        .override_at(q)
                                        .unwrap_or_else(|| reference.material(preview.point(q)))
                                };
                                cost = cost.saturating_add(light_field::loss(m));
                            }
                            assert_eq!(source.edge(p, axis), cost, "{p:?}/{axis}");
                        }
                    }
                }
            }
        }
    }
}
