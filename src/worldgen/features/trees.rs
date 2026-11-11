use super::{
    ecology::Habitat,
    geometry::{Primitive as P, Replace},
    Rng,
};
use crate::materials::*;
use glam::DVec3 as V;
use std::f64::consts::TAU;

/// Tapered, joined woody limbs carry finer shoots and separated foliage sprays.
pub fn tree(out: &mut Vec<P>, base: V, kind: Habitat, rng: &mut Rng) {
    use Habitat::*;
    let pine = matches!(kind, Pine | Snow);
    let birch = kind == Birch;
    let acacia = kind == Savanna;
    let tall = matches!(kind, Jungle | Dark);
    let height = if pine {
        8.0 + rng.unit() * 5.0
    } else if tall {
        9.0 + rng.unit() * 5.0
    } else if birch {
        7.0 + rng.unit() * 3.0
    } else {
        5.0 + rng.unit() * 3.0
    };
    let bark = if birch { BIRCH_BARK } else { BARK };
    let leaf = match kind {
        Pine | Snow => NEEDLES,
        Cherry => CHERRY_LEAF,
        Pale => PALE_LEAF,
        _ => LEAF,
    };
    let radius = if tall {
        0.43
    } else if birch {
        0.18
    } else {
        0.25
    };
    let lean = V::new(rng.signed() * 0.6, height, rng.signed() * 0.6);
    let mut previous = base - V::Y * 0.15;
    for i in 1..=6 {
        let t = i as f64 / 6.;
        let next = base + lean * t + V::new((t * 4.).sin() * 0.13, 0., (t * 3.).sin() * 0.16);
        let r0 = radius * (1. - (t - 1. / 6.) * 0.85);
        let r1 = radius * (1. - t * 0.85);
        out.push(P::branch(previous, next, r0, r1, bark, Replace::Any, 40));
        out.push(P::branch(
            previous,
            next,
            (r0 - 0.065).max(0.018),
            (r1 - 0.065).max(0.015),
            HEARTWOOD,
            Replace::Any,
            41,
        ));
        if birch && i % 2 == 0 {
            out.push(P::branch(
                next - V::Y * 0.06,
                next + V::Y * 0.05,
                r1 + 0.012,
                r1 + 0.012,
                BARK,
                Replace::Any,
                42,
            ));
        }
        previous = next;
    }
    for i in 0..6 {
        let angle = i as f64 * TAU / 6. + rng.unit() * 0.4;
        let dir = V::new(angle.cos(), 0., angle.sin());
        out.push(P::branch(
            base + V::Y * 0.45,
            base + dir * (0.8 + rng.unit() * 0.5) - V::Y * 0.12,
            radius * 0.7,
            0.055,
            bark,
            Replace::Any,
            40,
        ));
    }
    if kind == Wetland {
        for i in 0..7 {
            let a = i as f64 * TAU / 7.;
            out.push(P::branch(
                base + V::Y * 1.8,
                base + V::new(a.cos() * 1.8, -0.2, a.sin() * 1.8),
                0.18,
                0.06,
                bark,
                Replace::Any,
                40,
            ));
        }
    }
    let limbs = if pine {
        22
    } else if tall {
        16
    } else {
        11
    };
    for i in 0..limbs {
        let t = if pine {
            0.22 + i as f64 / limbs as f64 * 0.72
        } else {
            0.38 + rng.unit() * 0.5
        };
        let a = base + lean * t;
        let angle = i as f64 * 2.39996 + rng.unit() * 0.55;
        let reach = if pine {
            (1. - t) * 5.0 + 0.4
        } else if acacia {
            2.8 + rng.unit() * 1.2
        } else {
            1.3 + rng.unit() * 2.0
        };
        let dir = V::new(angle.cos(), 0., angle.sin());
        let b = a + dir * reach + V::Y * if pine { -0.15 } else { 0.55 + rng.unit() * 0.8 };
        let r = radius * (1. - t) * 0.75 + 0.035;
        out.push(P::branch(a, b, r, 0.045, bark, Replace::Any, 40));
        for j in 0..4 {
            let fork = angle + (j as f64 - 1.5) * 0.65;
            let start = a.lerp(b, 0.45 + j as f64 * 0.13);
            let tip = start
                + V::new(
                    fork.cos(),
                    if pine { 0.18 } else { 0.7 + rng.unit() * 0.6 },
                    fork.sin(),
                ) * (0.55 + rng.unit() * 0.5);
            out.push(P::branch(start, tip, 0.055, 0.032, bark, Replace::Any, 40));
            for k in 0..3 {
                let at = tip + V::new(rng.signed() * 0.45, k as f64 * 0.16, rng.signed() * 0.45);
                if pine {
                    out.push(P::ellipsoid(
                        at,
                        V::new(0.62, 0.17, 0.62) * (0.8 + rng.unit() * 0.5),
                        leaf,
                        Replace::Air,
                        15,
                    ));
                    if kind == Snow {
                        out.push(P::ellipsoid(
                            at + V::Y * 0.16,
                            V::new(0.36, 0.06, 0.36),
                            SNOW,
                            Replace::Air,
                            16,
                        ));
                    }
                } else {
                    let direction = V::new(
                        (fork + k as f64 * 0.7).cos(),
                        0.12,
                        (fork + k as f64 * 0.7).sin(),
                    );
                    let shoot_start = at - direction * 0.28;
                    let shoot_end = at + direction * 0.28;
                    out.push(P::branch(
                        tip,
                        shoot_end,
                        0.045,
                        0.035,
                        bark,
                        Replace::Air,
                        39,
                    ));
                    for l in 0..7 {
                        let t = l as f64 / 6.;
                        let side = if l % 2 == 0 { 1. } else { -1. };
                        let c = shoot_start.lerp(shoot_end, t)
                            + V::new(-direction.z, 0., direction.x) * side * 0.11;
                        out.push(P::ellipsoid(
                            c,
                            V::new(0.15, 0.045, 0.11) * (0.8 + rng.unit() * 0.4),
                            leaf,
                            Replace::Air,
                            15,
                        ));
                    }
                }
            }
            if matches!(kind, Jungle | Wetland | Pale) && j % 2 == 0 {
                let end = tip - V::Y * (0.8 + rng.unit() * 1.6);
                out.push(P::branch(
                    tip,
                    end,
                    0.035,
                    0.032,
                    if kind == Pale { LICHEN } else { FERN },
                    Replace::Air,
                    16,
                ));
            }
        }
    }
    out.push(P::ellipsoid(
        previous,
        V::new(0.30, 0.15, 0.30),
        leaf,
        Replace::Air,
        15,
    ));
}
