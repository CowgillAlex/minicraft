use crate::{
    camera::Camera,
    editing::{self, Hit},
    hud::{Hud, HudVertex},
    ui::{Layout, Rect, UiState},
};
pub const TEXT: [f32; 4] = [0.94, 0.95, 0.93, 1.0];
pub const MUTED: [f32; 4] = [0.60, 0.65, 0.65, 1.0];
pub const ACCENT: [f32; 4] = [0.65, 0.84, 0.66, 1.0];
pub struct OverlayStats<'a> {
    pub noise_preview: Option<crate::worldgen::preview::Preview>,
    pub voxels: usize,
    pub triangles: usize,
    pub chunks: usize,
    pub ao_enabled: bool,
    pub night_vision: bool,
    pub view: crate::view_settings::ViewSettings,
    pub graphics: crate::graphics::Settings,
    pub gpu_ms: Option<[f32; 8]>,
    pub gpu_supported: bool,
    pub grid: bool,
    pub local_time: bool,
    pub interaction: Option<&'a str>,
    pub material: u8,
    pub target: Option<Hit>,
    pub target_material: Option<u8>,
    pub biome: Option<&'a str>,
    pub time: &'a str,
    pub lighting_pending: bool,
    pub meshes_pending: bool,
    pub terrain_full: bool,
    pub streaming: Option<crate::stream_metrics::Snapshot>,
}
pub fn vertices(
    camera: &Camera,
    size: (u32, u32),
    fps: f32,
    captured: bool,
    vsync: bool,
    stats: OverlayStats<'_>,
    ui: &UiState,
) -> Vec<HudVertex> {
    let layout = Layout::new(size);
    let mut hud = Hud::new(layout.width, layout.height);
    let shadow = [0.0, 0.0, 0.0, 0.65];
    hud.styled_text("MiniCraft", 25.0, 25.0, 16.0, shadow, true);
    hud.styled_text("MiniCraft", 24.0, 24.0, 16.0, [0.94, 0.95, 0.93, 0.8], true);
    if let Some(preview) = stats.noise_preview {
        hud.text(
            &format!(
                "{} / Seed {}",
                if preview.infinite {
                    "World"
                } else {
                    "Noise preview"
                },
                preview.seed
            ),
            24.0,
            46.0,
            11.0,
            MUTED,
        );
    }
    let clock = Rect {
        x: layout.width - 124.0,
        y: 20.0,
        w: 104.0,
        h: 36.0,
    };
    hud.rounded_rect(clock, 10.0, [0.055, 0.067, 0.078, 0.88]);
    let hour = stats
        .time
        .get(..2)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(12);
    crate::ui_icons::clock(
        &mut hud,
        clock.x + 20.0,
        clock.y + 18.0,
        !(6..19).contains(&hour),
    );
    hud.text(stats.time, clock.x + 38.0, clock.y + 10.0, 16.0, TEXT);
    hotbar(&mut hud, &layout, stats.material);
    hud.text("F1  Controls", 25.0, layout.height - 37.0, 13.0, shadow);
    hud.text(
        "F1  Controls",
        24.0,
        layout.height - 38.0,
        13.0,
        [0.85, 0.89, 0.86, 0.8],
    );
    if layout.width >= 1100.0 {
        let speed = format!(
            "{}   {:.1} m/s",
            if ui.spectating {
                "Spectator"
            } else if ui.flying {
                "Fly"
            } else {
                "Walk"
            },
            camera.speed
        );
        let speed_x = layout.width - 24.0 - Hud::text_width(&speed, 13.0, false);
        hud.text(&speed, speed_x + 1.0, layout.height - 37.0, 13.0, shadow);
        hud.text(
            &speed,
            speed_x,
            layout.height - 38.0,
            13.0,
            [0.85, 0.89, 0.86, 0.8],
        );
    }
    if stats.night_vision {
        hud.text("Night vision", 24.0, layout.height - 60.0, 12.0, ACCENT);
    }
    if stats.lighting_pending {
        hud.text("Updating light", layout.width - 124.0, 66.0, 12.0, MUTED);
    }
    if stats.meshes_pending {
        hud.text(
            "Loading terrain",
            layout.width - 124.0,
            if stats.lighting_pending { 84.0 } else { 66.0 },
            12.0,
            MUTED,
        );
    }
    if captured {
        if let Some(hint) = stats.interaction {
            let width = Hud::text_width(hint, 13.0, false) + 24.0;
            hud.rounded_rect(
                Rect {
                    x: layout.width / 2.0 - width / 2.0,
                    y: layout.height / 2.0 + 22.0,
                    w: width,
                    h: 28.0,
                },
                7.0,
                [0.035, 0.045, 0.05, 0.9],
            );
            hud.centered(
                hint,
                layout.width / 2.0,
                layout.height / 2.0 + 29.0,
                13.0,
                TEXT,
                false,
            );
        }
        let (x, y) = (layout.width / 2.0, layout.height / 2.0);
        for (dx, dy, w, h) in [
            (-6.0, -0.5, 4.0, 1.0),
            (2.0, -0.5, 4.0, 1.0),
            (-0.5, -6.0, 1.0, 4.0),
            (-0.5, 2.0, 1.0, 4.0),
        ] {
            hud.rect(
                x + dx - 1.0,
                y + dy - 1.0,
                w + 2.0,
                h + 2.0,
                [0.0, 0.0, 0.0, 0.25],
            );
            hud.rect(x + dx, y + dy, w, h, [1.0, 1.0, 1.0, 0.8]);
        }
    }
    if ui.debug && captured {
        debug(&mut hud, camera, &stats, fps, vsync, ui);
        crate::lagometer::draw(&mut hud, &layout, ui, &stats);
    }
    if !captured {
        crate::ui_menu::draw(&mut hud, &layout, ui, &stats, vsync);
    }
    hud.vertices
}
fn hotbar(hud: &mut Hud, layout: &Layout, material: u8) {
    let bar = layout.hotbar();
    hud.rounded_rect(
        Rect {
            y: bar.y + 3.0,
            ..bar
        },
        13.0,
        [0.0, 0.0, 0.0, 0.22],
    );
    hud.rounded_rect(bar, 13.0, [0.65, 0.71, 0.72, 0.25]);
    hud.rounded_rect(
        Rect {
            x: bar.x + 1.0,
            y: bar.y + 1.0,
            w: bar.w - 2.0,
            h: bar.h - 2.0,
        },
        12.0,
        [0.035, 0.045, 0.050, 0.92],
    );
    let name = editing::material_name(material).to_ascii_lowercase();
    let name = format!("{}{}", name[..1].to_ascii_uppercase(), &name[1..]);
    let text_width = Hud::text_width(&name, 15.0, true) + 24.0;
    hud.rounded_rect(
        Rect {
            x: layout.width / 2.0 - text_width / 2.0,
            y: bar.y - 34.0,
            w: text_width,
            h: 26.0,
        },
        7.0,
        [0.035, 0.045, 0.050, 0.82],
    );
    hud.centered(&name, layout.width / 2.0, bar.y - 29.0, 15.0, TEXT, true);
    for (i, &m) in editing::PALETTE.iter().enumerate() {
        let x = bar.x + 8.0 + i as f32 * 46.0;
        let slot = Rect {
            x,
            y: bar.y + 7.0,
            w: 44.0,
            h: 50.0,
        };
        if m == material {
            hud.rounded_rect(slot, 7.0, [0.80, 0.91, 0.79, 1.0]);
            hud.rounded_rect(
                Rect {
                    x: x + 1.5,
                    y: slot.y + 1.5,
                    w: 41.0,
                    h: 47.0,
                },
                5.5,
                [0.14, 0.20, 0.17, 1.0],
            );
        } else {
            hud.rounded_rect(slot, 7.0, [0.18, 0.22, 0.23, 0.42]);
        }
        crate::ui_icons::block(hud, x + 10.0, bar.y + 13.0, 26.0, m);
        let key = if i == 10 {
            "B".to_string()
        } else if i == 11 {
            "N".to_string()
        } else {
            ((i + 1) % 10).to_string()
        };
        hud.text(
            &key,
            x + 5.0,
            bar.y + 43.0,
            10.0,
            if m == material { ACCENT } else { MUTED },
        );
    }
}
fn debug(
    hud: &mut Hud,
    camera: &Camera,
    stats: &OverlayStats<'_>,
    fps: f32,
    vsync: bool,
    ui: &UiState,
) {
    let position = if let Some(preview) = stats.noise_preview {
        camera.position
            + glam::Vec3::new(
                preview.origin[0] as f32 + crate::world::WIDTH as f32 / 32.0,
                preview.origin[1] as f32,
                preview.origin[2] as f32 + crate::world::DEPTH as f32 / 32.0,
            )
    } else {
        camera.position
    };
    let mut rows = vec![
        format!("MiniCraft v{}", env!("CARGO_PKG_VERSION")),
        format!(
            "{:.1} TPS / tick {} / {}",
            ui.performance.tps,
            ui.ticks,
            if ui.grounded {
                "grounded"
            } else if ui.spectating {
                "spectating"
            } else if ui.flying {
                "flying"
            } else {
                "airborne"
            }
        ),
        format!(
            "{fps:.0} FPS  /  {:.1} ms",
            if fps > 0.0 { 1000.0 / fps } else { 0.0 }
        ),
        format!("{} triangles / {} chunks", stats.triangles, stats.chunks),
        format!("{:.2}M voxels", stats.voxels as f32 / 1e6),
        format!(
            "XYZ  {:.1}  {:.1}  {:.1}",
            position.x, position.y, position.z
        ),
        format!(
            "AO {}  /  VSync {}",
            if stats.ao_enabled { "on" } else { "off" },
            if vsync { "on" } else { "off" }
        ),
        format!(
            "Terrain {}",
            if stats.terrain_full {
                "view prioritized / memory limit"
            } else if stats.meshes_pending {
                "streaming"
            } else {
                "ready"
            }
        ),
        format!("Biome {}", stats.biome.unwrap_or("Village scene")),
        stats
            .target_material
            .map(|id| format!("Block {}", crate::editing::material_name(id)))
            .unwrap_or_else(|| "Block none".into()),
        stats
            .target
            .map(|h| format!("Target {:?} / {:.2}m", h.cell, h.distance))
            .unwrap_or_else(|| "No target within 8m".into()),
    ];
    if let Some(s) = stats.streaming {
        rows.push(format!(
            "Queue {} / gen {} / light {}+{} / stage {}",
            s.queued,
            usize::from(s.geometry_active),
            s.light_queued,
            usize::from(s.light_active),
            s.staging
        ));
        rows.push(format!("Oldest pending request {:.0} ms", s.oldest_ms));
        if let Some(l) = s.last {
            rows.push(format!(
                "Last R: queue {:.0} / light {:.0} ms",
                l.queue_ms,
                l.light_queue_ms + l.light_ms
            ));
            rows.push(format!(
                "Ready: first {:.0} / all {:.0} ms",
                l.first_ready_ms, l.ready_ms
            ));
        }
    }
    hud.rounded_rect(
        Rect {
            x: 20.0,
            y: 60.0,
            w: 420.0,
            h: rows.len() as f32 * 23.0 + 30.0,
        },
        10.0,
        [0.025, 0.035, 0.04, 0.9],
    );
    for (i, row) in rows.iter().enumerate() {
        hud.text(
            row,
            34.0,
            74.0 + i as f32 * 23.0,
            13.0,
            if i == 0 { ACCENT } else { TEXT },
        );
    }
}
