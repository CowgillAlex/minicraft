use super::{
    ecology::Habitat,
    geometry::{Primitive as P, Replace},
    Rng,
};
use crate::materials::*;
use glam::DVec3 as V;
use std::f64::consts::TAU;
pub(super) fn stem(out: &mut Vec<P>, a: V, b: V, material: u8, water: bool) {
    out.push(P::branch(
        a,
        b,
        0.047,
        0.042,
        material,
        if water { Replace::Water } else { Replace::Air },
        24,
    ));
}
pub fn grass(out: &mut Vec<P>, base: V, rng: &mut Rng, dry: bool) {
    for i in 0..7 {
        let a = i as f64 * 2.39996 + rng.unit();
        let root = base + V::new(a.cos(), 0., a.sin()) * rng.unit() * 0.22;
        let h = 0.22 + rng.unit() * 0.48;
        let bend = V::new(a.cos(), 0., a.sin()) * (0.08 + rng.unit() * 0.2);
        let mid = root + V::Y * h * 0.65 + bend * 0.3;
        let tip = root + V::Y * h + bend;
        stem(
            out,
            root,
            mid,
            if dry { WHEAT_STEM } else { GRASS_BLADE },
            false,
        );
        stem(
            out,
            mid,
            tip,
            if dry { WHEAT_STEM } else { GRASS_BLADE },
            false,
        );
        if rng.unit() < 0.24 {
            for k in 0..3 {
                out.push(P::ellipsoid(
                    tip + V::Y * k as f64 * 0.055,
                    V::new(0.055, 0.075, 0.05),
                    SEED,
                    Replace::Air,
                    32,
                ));
            }
        }
    }
}
pub fn flower(out: &mut Vec<P>, base: V, rng: &mut Rng, color: u8) {
    let head = base
        + V::new(
            rng.signed() * 0.12,
            0.5 + rng.unit() * 0.4,
            rng.signed() * 0.12,
        );
    stem(out, base, head, STEM, false);
    for side in [-1., 1.] {
        out.push(P::ellipsoid(
            base.lerp(head, 0.45) + V::X * side * 0.11,
            V::new(0.16, 0.055, 0.075),
            FERN,
            Replace::Air,
            25,
        ));
    }
    for i in 0..6 {
        let a = i as f64 * TAU / 6.;
        out.push(P::ellipsoid(
            head + V::new(a.cos() * 0.125, 0., a.sin() * 0.125),
            V::new(0.10, 0.055, 0.10),
            color,
            Replace::Air,
            30,
        ));
    }
    out.push(P::ellipsoid(
        head + V::Y * 0.025,
        V::new(0.075, 0.065, 0.075),
        POLLEN,
        Replace::Air,
        33,
    ));
}
pub fn wheat(out: &mut Vec<P>, base: V, rng: &mut Rng) {
    for i in 0..6 {
        let a = i as f64 * 2.4;
        let root = base + V::new(a.cos(), 0., a.sin()) * 0.18;
        let head = root
            + V::new(
                rng.signed() * 0.1,
                0.85 + rng.unit() * 0.4,
                rng.signed() * 0.1,
            );
        stem(out, root, head + V::Y * 0.3, WHEAT_STEM, false);
        for j in 0..5 {
            let sign = if j % 2 == 0 { 1. } else { -1. };
            let p = head + V::new(sign * 0.055, j as f64 * 0.065, 0.);
            out.push(P::ellipsoid(
                p,
                V::new(0.075, 0.065, 0.055),
                GRAIN,
                Replace::Air,
                35,
            ));
        }
        stem(
            out,
            root.lerp(head, 0.45),
            root.lerp(head, 0.65) + V::new(0.22, 0., 0.10),
            GRASS_BLADE,
            false,
        );
    }
}
pub fn fern(out: &mut Vec<P>, base: V, rng: &mut Rng) {
    for i in 0..6 {
        let a = i as f64 * TAU / 6.;
        let dir = V::new(a.cos(), 0., a.sin());
        let tip = base + dir * 0.6 + V::Y * (0.4 + rng.unit() * 0.35);
        stem(out, base, tip, STEM, false);
        for j in 1..6 {
            let t = j as f64 / 6.;
            let p = base.lerp(tip, t);
            let width = 0.17 * (1. - t) + 0.045;
            for sign in [-1., 1.] {
                let side = V::new(-a.sin(), 0.1, a.cos()) * sign;
                stem(out, p, p + side * width, FERN, false);
            }
        }
    }
}
pub fn mushroom(out: &mut Vec<P>, base: V, rng: &mut Rng, large: bool) {
    let h = if large {
        2.2 + rng.unit() * 1.5
    } else {
        0.25 + rng.unit() * 0.3
    };
    let width = if large { 1.3 } else { 0.2 };
    out.push(P::branch(
        base,
        base + V::Y * h,
        if large { 0.2 } else { 0.055 },
        if large { 0.16 } else { 0.05 },
        MUSHROOM_STEM,
        Replace::Air,
        24,
    ));
    let top = base + V::Y * h;
    out.push(P::ellipsoid(
        top,
        V::new(width, width * 0.3, width),
        MUSHROOM_CAP,
        Replace::Air,
        30,
    ));
    for i in 0..8 {
        let a = i as f64 * 2.4;
        let r = width * (0.2 + rng.unit() * 0.55);
        out.push(P::ellipsoid(
            top + V::new(a.cos() * r, width * 0.25, a.sin() * r),
            V::splat(if large { 0.12 } else { 0.045 }),
            PETAL_WHITE,
            Replace::Air,
            32,
        ));
    }
}
pub fn cactus(out: &mut Vec<P>, base: V, rng: &mut Rng) {
    let h = 1.3 + rng.unit() * 1.7;
    out.push(P::branch(
        base,
        base + V::Y * h,
        0.18,
        0.14,
        CACTUS,
        Replace::Air,
        30,
    ));
    for sign in [-1., 1.] {
        let p = base + V::Y * (h * 0.45);
        let q = p + V::X * sign * 0.45;
        out.push(P::branch(p, q, 0.12, 0.11, CACTUS, Replace::Air, 30));
        out.push(P::branch(
            q,
            q + V::Y * 0.55,
            0.11,
            0.09,
            CACTUS,
            Replace::Air,
            30,
        ));
    }
    if rng.unit() < 0.5 {
        flower(out, base + V::Y * (h - 0.45), rng, PETAL_PINK);
    }
}
pub fn detail(out: &mut Vec<P>, base: V, kind: Habitat, rng: &mut Rng) {
    use Habitat::*;
    match kind {
        Sunflower => {
            grass(out, base, rng, false);
            if rng.unit() < 0.45 {
                sunflower(out, base, rng);
            }
        }
        Bamboo => {
            if rng.unit() < 0.3 {
                bamboo(out, base, rng);
            } else {
                fern(out, base, rng);
            }
        }
        Beach => grass(out, base, rng, true),
        Desert | Badlands => {
            if rng.unit() < 0.25 {
                cactus(out, base, rng)
            } else {
                grass(out, base, rng, true)
            }
        }
        Alpine | Snow | FrozenRiver => {
            out.push(P::ellipsoid(
                base + V::Y * 0.03,
                V::new(0.22, 0.05, 0.18),
                LICHEN,
                Replace::Air,
                22,
            ));
            if rng.unit() < 0.25 {
                grass(out, base, rng, true)
            }
        }
        Mushroom => mushroom(out, base, rng, true),
        Pine | Dark | Pale | Jungle | Wetland | Lush => {
            if rng.unit() < 0.18 {
                mushroom(out, base, rng, false)
            } else {
                fern(out, base, rng)
            }
        }
        Savanna => {
            grass(out, base, rng, true);
            if rng.unit() < 0.2 {
                wheat(out, base, rng)
            }
        }
        River => {
            for i in 0..4 {
                let root = base + V::X * i as f64 * 0.12;
                let tip = root + V::Y * (0.8 + rng.unit() * 0.6);
                stem(out, root, tip, REED, false);
                out.push(P::ellipsoid(
                    tip,
                    V::new(0.065, 0.17, 0.065),
                    SEED,
                    Replace::Air,
                    30,
                ));
            }
        }
        DeepDark => {
            out.push(P::ellipsoid(
                base + V::Y * 0.03,
                V::new(0.35, 0.05, 0.28),
                SCULK,
                Replace::Air,
                22,
            ));
        }
        Sulfur => {
            out.push(P::branch(
                base,
                base + V::new(0.12, 0.5, 0.03),
                0.12,
                0.025,
                CRYSTAL,
                Replace::Air,
                25,
            ));
        }
        Dripstone => {
            out.push(P::branch(
                base,
                base + V::Y * (0.3 + rng.unit()),
                0.17,
                0.025,
                SANDSTONE,
                Replace::Air,
                25,
            ));
        }
        _ => {
            grass(out, base, rng, false);
            let roll = rng.unit();
            if roll < 0.28 {
                let color = [PETAL_WHITE, PETAL_RED, PETAL_BLUE, PETAL_PINK][rng.index(4)];
                flower(out, base, rng, color)
            } else if roll < 0.36 {
                wheat(out, base, rng)
            }
        }
    }
}

pub fn sunflower(out: &mut Vec<P>, base: V, rng: &mut Rng) {
    let head = base + V::Y * (1.3 + rng.unit() * 0.5);
    stem(out, base, head, STEM, false);
    for i in 0..10 {
        let a = i as f64 * TAU / 10.;
        out.push(P::ellipsoid(
            head + V::new(a.cos() * 0.24, a.sin() * 0.24, 0.),
            V::splat(0.105),
            POLLEN,
            Replace::Air,
            30,
        ));
    }
    out.push(P::ellipsoid(
        head,
        V::new(0.17, 0.17, 0.06),
        BARK,
        Replace::Air,
        31,
    ));
    for y in -1..=1 {
        for x in -1..=1 {
            out.push(P::ellipsoid(
                head + V::new(x as f64 * 0.075, y as f64 * 0.075, 0.06),
                V::splat(0.035),
                SEED,
                Replace::Air,
                32,
            ));
        }
    }
    for s in [-1., 1.] {
        out.push(P::ellipsoid(
            base + V::new(s * 0.16, 0.6, 0.),
            V::new(0.23, 0.075, 0.11),
            FERN,
            Replace::Air,
            25,
        ));
    }
}
pub fn bamboo(out: &mut Vec<P>, base: V, rng: &mut Rng) {
    let h = 4. + rng.unit() * 3.;
    out.push(P::branch(
        base,
        base + V::Y * h,
        0.10,
        0.065,
        BAMBOO,
        Replace::Air,
        35,
    ));
    for i in 1..(h / 0.45) as i32 {
        let p = base + V::Y * i as f64 * 0.45;
        out.push(P::branch(
            p,
            p + V::Y * 0.045,
            0.112,
            0.112,
            BARK,
            Replace::Air,
            36,
        ));
        if i % 2 == 0 {
            for sign in [-1., 1.] {
                stem(out, p, p + V::new(sign * 0.5, 0.15, 0.2), LEAF, false);
            }
        }
    }
}
