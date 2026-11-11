use super::{
    ecology::Habitat,
    geometry::{Primitive as P, Replace},
    plants::stem,
    Rng,
};
use crate::materials::*;
use glam::DVec3 as V;
pub fn grow(out: &mut Vec<P>, base: V, depth: f64, kind: Habitat, rng: &mut Rng) {
    if kind == Habitat::Reef && rng.unit() < 0.6 {
        let color = [CORAL_PINK, CORAL_BLUE, CORAL_GOLD][rng.index(3)];
        let top = base + V::Y * (0.3 + rng.unit() * 0.5);
        out.push(P::branch(base, top, 0.10, 0.07, color, Replace::Water, 26));
        for i in 0..6 {
            let a = i as f64 * 2.4;
            let tip = top + V::new(a.cos() * 0.3, rng.unit() * 0.3, a.sin() * 0.3);
            out.push(P::branch(top, tip, 0.075, 0.05, color, Replace::Water, 26));
            out.push(P::ellipsoid(tip, V::splat(0.1), color, Replace::Water, 27));
        }
        return;
    }
    let kelp = depth > 2. && !matches!(kind, Habitat::River | Habitat::FrozenRiver);
    let h = if kelp {
        (1.5 + rng.unit() * 4.).min(depth - 0.15)
    } else {
        (0.35 + rng.unit() * 0.65).min(depth - 0.1)
    };
    if h < 0.12 {
        return;
    }
    for n in 0..if kelp { 2 } else { 5 } {
        let root = base + V::new(rng.signed() * 0.18, 0., rng.signed() * 0.18);
        let mut prev = root;
        for j in 1..=8 {
            let t = j as f64 / 8.;
            let p = root
                + V::new(
                    (t * 3. + n as f64).sin() * 0.18,
                    h * t,
                    (t * 4.).sin() * 0.16,
                );
            stem(out, prev, p, if kelp { KELP } else { SEAGRASS }, true);
            if kelp && j > 1 {
                out.push(P::ellipsoid(
                    p + V::X * if j % 2 == 0 { 0.13 } else { -0.13 },
                    V::new(0.22, 0.055, 0.09),
                    KELP,
                    Replace::Water,
                    25,
                ));
            }
            prev = p;
        }
    }
}
