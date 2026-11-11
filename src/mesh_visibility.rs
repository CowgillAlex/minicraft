use crate::world::{self, AIR, CHUNK, WATER};
/// Find exposed faces with bitwise neighbor tests. Each output row is a mask of
/// visible U positions; solid interiors never enter the AO or greedy loops.
pub fn rows(halo: &[u8], axis: usize) -> [Vec<u32>; 2] {
    let side = CHUNK + 2;
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let mut rows = [vec![0; CHUNK * CHUNK], vec![0; CHUNK * CHUNK]];
    for j in 0..CHUNK {
        for i in 0..CHUNK {
            let mut p = [0; 3];
            p[u] = i + 1;
            p[v] = j + 1;
            let (mut solid, mut water, mut glass) = (0_u64, 0_u64, 0_u64);
            for k in 0..side {
                p[axis] = k;
                let m = halo[p[0] + side * (p[2] + side * p[1])];
                let bit = 1_u64 << k;
                match m {
                    AIR => {}
                    WATER => water |= bit,
                    world::GLASS => glass |= bit,
                    _ => solid |= bit,
                }
            }
            for (sign, rows) in rows.iter_mut().enumerate() {
                let shift = |bits: u64| if sign == 0 { bits << 1 } else { bits >> 1 };
                let mut visible = ((solid & !shift(solid))
                    | (water & !shift(solid | water))
                    | (glass & !shift(solid | glass)))
                    & 0x1fffffffe;
                while visible != 0 {
                    let slice = visible.trailing_zeros() as usize - 1;
                    rows[slice * CHUNK + j] |= 1_u32 << i;
                    visible &= visible - 1;
                }
            }
        }
    }
    rows
}
