use super::geometry::Primitive as P;
use crate::materials::*;
use glam::DVec3 as V;
/// A hollow timber cottage. The door aperture is reserved for the existing
/// editable door object, which is installed once its generated pages arrive.
pub fn cabin(out: &mut Vec<P>, base: V) {
    let mut block =
        |lo: V, hi: V, m: u8, priority| out.push(P::cuboid(base + lo, base + hi, m, priority));
    block(V::new(-2.5, -1., -2.1), V::new(2.5, 0.125, 2.1), STONE, 80);
    block(V::new(-2.2, 0.125, -1.8), V::new(2.2, 2.9, 1.8), WALL, 90);
    block(
        V::new(-2.05, 0.25, -1.65),
        V::new(2.05, 2.85, 1.65),
        AIR,
        95,
    );
    block(
        V::new(-2.05, 0.125, -1.65),
        V::new(2.05, 0.25, 1.65),
        WOOD,
        96,
    );
    for y in [0.5, 1.25, 2.25, 2.8] {
        for z in [-1.81, 1.68] {
            block(
                V::new(-2.2, y, z),
                V::new(2.2, y + 0.10, z + 0.13),
                WOOD,
                96,
            );
        }
    }
    for x in [-1.35, 1.35] {
        block(
            V::new(x - 0.45, 1.2, -1.82),
            V::new(x + 0.45, 2.25, -1.64),
            WOOD,
            97,
        );
        block(
            V::new(x - 0.34, 1.30, -1.83),
            V::new(x + 0.34, 2.14, -1.63),
            GLASS,
            98,
        );
        block(
            V::new(x - 0.45, 1.2, 1.64),
            V::new(x + 0.45, 2.25, 1.82),
            WOOD,
            97,
        );
        block(
            V::new(x - 0.34, 1.30, 1.63),
            V::new(x + 0.34, 2.14, 1.83),
            GLASS,
            98,
        );
    }
    // Full-height entrance, facing the positive Z side.
    block(
        V::new(-0.625, 0.25, 1.60),
        V::new(0.625, 2.45, 1.84),
        WOOD,
        99,
    );
    block(V::new(-0.5, 0.25, 1.59), V::new(0.5, 2.25, 1.90), AIR, 100);
    for i in 0..40 {
        let x = -2.5 + i as f64 * 0.125;
        let y = 2.8 + (2.5 - x.abs()) * 0.65;
        block(
            V::new(x, y, -2.05),
            V::new(x + 0.125, y + 0.16, 2.05),
            ROOF,
            100,
        );
    }
    for i in 0..36 {
        let x = -2.2 + i as f64 * 0.125;
        let top = 2.8 + (2.5 - x.abs()) * 0.65;
        for z in [-1.8, 1.68] {
            block(
                V::new(x, 2.8, z),
                V::new(x + 0.125, top, z + 0.12),
                WALL,
                90,
            );
        }
    }
    block(
        V::new(-0.15, 2.52, -0.15),
        V::new(0.15, 2.72, 0.15),
        LIGHT,
        101,
    );
    block(V::new(1.1, 2.9, -0.8), V::new(1.45, 4.4, -0.45), STONE, 101);
    block(V::new(-0.55, 0.10, 1.8), V::new(0.55, 0.24, 3.4), WOOD, 96);
    let door = crate::doors::Door::prefab([0; 3]);
    let origin = base + V::new(-0.5, 0.25, 1.625);
    for z in 0..4 {
        for y in 0..32 {
            let mut x = 0;
            while x < 16 {
                let m = door.get([x, y, z]);
                let start = x;
                x += 1;
                while x < 16 && door.get([x, y, z]) == m {
                    x += 1;
                }
                if m != AIR {
                    out.push(P::cuboid(
                        origin + V::new(start as f64, y as f64, z as f64) / 16.,
                        origin + V::new(x as f64, (y + 1) as f64, (z + 1) as f64) / 16.,
                        m,
                        110,
                    ));
                }
            }
        }
    }
}
