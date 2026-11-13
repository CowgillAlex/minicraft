use crate::{
    hud::Hud,
    overlay::{OverlayStats, ACCENT, MUTED, TEXT},
    ui::{Layout, Rect, UiState},
};
pub fn draw(hud: &mut Hud, layout: &Layout, ui: &UiState, stats: &OverlayStats<'_>) {
    let x = layout.width - 300.0;
    hud.rounded_rect(
        Rect {
            x,
            y: 94.0,
            w: 280.0,
            h: 206.0,
        },
        8.0,
        [0.025, 0.035, 0.04, 0.9],
    );
    hud.text("GPU sample / milliseconds", x + 12.0, 104.0, 12.0, ACCENT);
    if let Some(ms) = stats.gpu_ms {
        for (i, label) in crate::gpu_timing::LABELS.iter().enumerate() {
            hud.text(label, x + 12.0, 128.0 + i as f32 * 17.0, 11.0, MUTED);
            hud.text(
                &format!("{:.2}", ms[i]),
                x + 208.0,
                128.0 + i as f32 * 17.0,
                11.0,
                TEXT,
            );
        }
        hud.text(
            &format!("Total {:.2} ms", ms.iter().sum::<f32>()),
            x + 12.0,
            275.0,
            12.0,
            TEXT,
        );
    } else {
        hud.text(
            if stats.gpu_supported {
                "Waiting for GPU sample..."
            } else {
                "GPU timestamps unavailable"
            },
            x + 12.0,
            132.0,
            12.0,
            MUTED,
        );
    }
    let base = layout.height - 274.0;
    let h = &ui.performance;
    for (row, label, values, budget) in [
        (
            0,
            format!("Frames {:.1} FPS / cyan: CPU render", h.fps),
            h.frames
                .iter()
                .map(|&(ms, cpu)| (ms, cpu))
                .collect::<Vec<_>>(),
            16.667_f32,
        ),
        (
            1,
            format!(
                "Ticks {:.1} TPS / {:.2} ms latest",
                h.tps,
                h.ticks.back().copied().unwrap_or(0.0)
            ),
            h.ticks.iter().map(|&ms| (ms, 0.0)).collect::<Vec<_>>(),
            50.0_f32,
        ),
    ] {
        let y = base + row as f32 * 98.0;
        hud.rounded_rect(
            Rect {
                x,
                y,
                w: 280.0,
                h: 92.0,
            },
            8.0,
            [0.025, 0.035, 0.04, 0.9],
        );
        hud.text(&label, x + 10.0, y + 7.0, 11.0, TEXT);
        let top = y + 28.0;
        let height = 52.0;
        let range = if row == 0 { 50.0 } else { 60.0 };
        for (i, (ms, cpu)) in values.iter().rev().take(240).rev().enumerate() {
            let bar = (*ms / range * height).clamp(1.0, height);
            let color = if *ms <= budget + 0.3 {
                [0.40, 0.80, 0.45, 1.0]
            } else if *ms <= budget * 2.0 {
                [0.95, 0.73, 0.25, 1.0]
            } else {
                [0.95, 0.30, 0.25, 1.0]
            };
            hud.rect(x + 10.0 + i as f32, top + height - bar, 1.0, bar, color);
            if *cpu > 0.0 {
                let work = (*cpu / range * height).clamp(0.0, height);
                hud.rect(
                    x + 10.0 + i as f32,
                    top + height - work,
                    1.0,
                    work,
                    [0.25, 0.80, 0.90, 1.0],
                );
            }
        }
        let target = top + height - budget / range * height;
        hud.rect(x + 10.0, target, 240.0, 1.0, [0.8, 0.82, 0.85, 0.5]);
        hud.text(
            if row == 0 { "16.7ms" } else { "50ms" },
            x + 239.0,
            target - 11.0,
            9.0,
            MUTED,
        );
    }
}
