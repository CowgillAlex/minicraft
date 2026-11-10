use super::{Chunk, World, AIR};

#[derive(Clone, Copy)]
struct Entry<'a> {
    key: [i32; 3],
    page: Option<&'a Chunk>,
    overlay: Option<&'a [u8]>,
}
/// A small read-only page cache for dense voxel traversals. The world borrow
/// prevents edits from invalidating cached references during a mesh or light bake.
pub struct Reader<'a> {
    world: &'a World,
    entries: [Option<Entry<'a>>; 8],
}
impl<'a> Reader<'a> {
    pub(super) fn new(world: &'a World) -> Self {
        Self {
            world,
            entries: [None; 8],
        }
    }
    fn read(&mut self, p: [i32; 3], objects: bool) -> u8 {
        if !self.world.contains(p) {
            return AIR;
        }
        let key = p.map(|v| v.div_euclid(32));
        let slot = ((key[0].wrapping_mul(3) ^ key[1].wrapping_mul(5) ^ key[2].wrapping_mul(7))
            as usize)
            & 7;
        if self.entries[slot].is_none_or(|entry| entry.key != key) {
            self.entries[slot] = Some(Entry {
                key,
                page: self.world.chunks.get(&key),
                overlay: self.world.door_chunks.get(&key).map(|c| c.as_ref()),
            });
        }
        let entry = self.entries[slot].unwrap();
        let i = p[0].rem_euclid(32) as usize
            + 32 * (p[2].rem_euclid(32) as usize + 32 * p[1].rem_euclid(32) as usize);
        let material = entry.page.map_or(AIR, |c| c.get(i));
        if material != AIR || !objects {
            material
        } else {
            entry.overlay.map_or(AIR, |c| c[i])
        }
    }
    pub fn get(&mut self, p: [i32; 3]) -> u8 {
        self.read(p, true)
    }
    pub fn get_static(&mut self, p: [i32; 3]) -> u8 {
        self.read(p, false)
    }
}
