use crate::{hud::Hud, world};

/// Isometric material previews use the same palette as world geometry.
pub fn block(hud: &mut Hud, x: f32, y: f32, size: f32, material: u8) {
    let color = world::color(material);
    let tint = |gain: f32, alpha: f32| [color[0] * gain, color[1] * gain, color[2] * gain, alpha];
    let p = |u: f32, v: f32| [x + u * size, y + v * size];
    let glass = material == world::GLASS;
    let top = if material == world::GRASS {
        [0.43, 0.68, 0.32, 1.0]
    } else {
        tint(1.3, if glass { 0.42 } else { 1.0 })
    };
    let left = if material == world::GRASS {
        [0.40, 0.29, 0.19, 1.0]
    } else {
        tint(0.9, if glass { 0.27 } else { 1.0 })
    };
    hud.polygon(&[p(0.5, 0.0), p(1.0, 0.25), p(0.5, 0.5), p(0.0, 0.25)], top);
    hud.polygon(
        &[p(0.0, 0.25), p(0.5, 0.5), p(0.5, 1.0), p(0.0, 0.75)],
        left,
    );
    hud.polygon(
        &[p(0.5, 0.5), p(1.0, 0.25), p(1.0, 0.75), p(0.5, 1.0)],
        tint(0.65, if glass { 0.35 } else { 1.0 }),
    );
    if glass {
        let shine = [0.74, 0.94, 0.98, 0.9];
        hud.polygon(
            &[p(0.06, 0.29), p(0.1, 0.31), p(0.1, 0.72), p(0.06, 0.70)],
            shine,
        );
        hud.polygon(
            &[p(0.53, 0.51), p(0.57, 0.49), p(0.57, 0.94), p(0.53, 0.96)],
            shine,
        );
        hud.polygon(
            &[p(0.93, 0.32), p(0.97, 0.30), p(0.97, 0.72), p(0.93, 0.74)],
            shine,
        );
        hud.polygon(
            &[p(0.15, 0.43), p(0.2, 0.45), p(0.38, 0.72), p(0.33, 0.70)],
            [0.9, 1.0, 1.0, 0.65],
        );
    } else if material == world::GRASS {
        hud.polygon(
            &[p(0.0, 0.25), p(0.5, 0.5), p(0.5, 0.63), p(0.0, 0.38)],
            [0.32, 0.52, 0.25, 1.0],
        );
    } else if material == world::WOOD {
        hud.polygon(
            &[p(0.12, 0.39), p(0.16, 0.41), p(0.16, 0.76), p(0.12, 0.74)],
            tint(0.55, 1.0),
        );
        hud.polygon(
            &[p(0.31, 0.47), p(0.35, 0.49), p(0.35, 0.86), p(0.31, 0.84)],
            tint(0.55, 1.0),
        );
    } else if material == world::LIGHT {
        hud.polygon(
            &[p(0.5, 0.07), p(0.87, 0.25), p(0.5, 0.43), p(0.13, 0.25)],
            [1.0, 0.96, 0.71, 1.0],
        );
    }
}
pub fn clock(hud: &mut Hud, x: f32, y: f32, night: bool) {
    let points: Vec<_> = (0..24)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 24.0;
            [x + angle.cos() * 6.0, y + angle.sin() * 6.0]
        })
        .collect();
    hud.polygon(
        &points,
        if night {
            [0.69, 0.80, 0.94, 1.0]
        } else {
            [0.96, 0.77, 0.42, 1.0]
        },
    );
    if night {
        let cut: Vec<_> = (0..24)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 24.0;
                [x + 3.0 + angle.cos() * 5.0, y - 2.0 + angle.sin() * 5.0]
            })
            .collect();
        hud.polygon(&cut, [0.055, 0.067, 0.078, 1.0]);
    }
}
