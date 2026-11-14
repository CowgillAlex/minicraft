pub use crate::ui_style::{ACCENT, MUTED, TEXT};
use crate::{
    camera::Camera,
    editing::{self, Hit},
    hud::{Hud, HudVertex},
    ui::{Layout, Rect, UiState, HOTBAR_GAP, HOTBAR_SLOT},
    ui_style as style,
};
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
    pub hotbar: editing::Hotbar,
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
    if let Some(preview) = stats.noise_preview {
        style::tracked(
            &mut hud,
            &format!(
                "{} {}",
                if preview.infinite {
                    "SEED"
                } else {
                    "NOISE PREVIEW"
                },
                preview.seed
            ),
            24.0,
            22.0,
            11.0,
            2.0,
            MUTED,
        );
    }
    let hour = stats
        .time
        .get(..2)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(12);
    let right = layout.width - 24.0;
    let time_w = Hud::text_width(stats.time, 26.0, true);
    style::text(&mut hud, stats.time, right - time_w, 18.0, 26.0, TEXT, true);
    crate::ui_icons::clock(
        &mut hud,
        right - time_w - 16.0,
        34.0,
        !(6..19).contains(&hour),
    );
    let mode = format!(
        "{}  {:.1} M/S",
        if ui.spectating {
            "SPECTATOR"
        } else if ui.flying {
            "FLY"
        } else {
            "WALK"
        },
        camera.speed
    );
    let mut status_y = 52.0;
    let mut status = |hud: &mut Hud, text: &str, color| {
        style::tracked_right(hud, text, right, status_y, 10.0, 2.0, color);
        status_y += 17.0;
    };
    status(&mut hud, &mode, ACCENT);
    if stats.night_vision {
        status(&mut hud, "NIGHT VISION", ACCENT);
    }
    if stats.lighting_pending {
        status(&mut hud, "UPDATING LIGHT", MUTED);
    }
    if stats.meshes_pending {
        status(&mut hud, "LOADING TERRAIN", MUTED);
    }
    if captured {
        hotbar(&mut hud, &layout, stats.material, stats.hotbar);
    }
    if captured {
        if let Some(hint) = stats.interaction {
            let (key, label) = hint.split_once("  ").unwrap_or(("", hint));
            let key_w = if key.is_empty() {
                0.0
            } else {
                style::keycap_width(key) + 8.0
            };
            let w = key_w + Hud::text_width(label, 14.0, true);
            let x = layout.width / 2.0 - w / 2.0;
            let y = layout.height / 2.0 + 24.0;
            if !key.is_empty() {
                style::keycap(&mut hud, key, x, y);
            }
            style::text(&mut hud, label, x + key_w, y + 3.0, 14.0, TEXT, true);
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
fn hotbar(hud: &mut Hud, layout: &Layout, material: u8, bar_state: editing::Hotbar) {
    let bar = layout.hotbar();
    let name = editing::material_name(material).to_ascii_uppercase();
    style::tracked_centered(
        hud,
        &name,
        layout.width / 2.0,
        bar.y - 26.0,
        14.0,
        4.0,
        TEXT,
    );
    for (i, &m) in bar_state.slots.iter().enumerate() {
        let slot = Rect {
            x: bar.x + i as f32 * (HOTBAR_SLOT + HOTBAR_GAP),
            y: bar.y,
            w: HOTBAR_SLOT,
            h: HOTBAR_SLOT,
        };
        let selected = i == bar_state.selected && m == material;
        style::chamfer(
            hud,
            slot,
            8.0,
            if selected {
                [0.36, 0.52, 0.27, 0.72]
            } else {
                [0.03, 0.045, 0.055, 0.6]
            },
        );
        if selected {
            hud.polygon(
                &[
                    [slot.x + 8.0, slot.y + slot.h - 3.0],
                    [slot.x + slot.w, slot.y + slot.h - 3.0],
                    [slot.x + slot.w, slot.y + slot.h],
                    [slot.x + 8.0, slot.y + slot.h],
                ],
                ACCENT,
            );
            hud.polygon(
                &[
                    [slot.x + 5.0, slot.y + slot.h - 3.0],
                    [slot.x + 8.0, slot.y + slot.h - 3.0],
                    [slot.x + 8.0, slot.y + slot.h],
                ],
                ACCENT,
            );
        }
        crate::ui_icons::block(hud, slot.x + 11.0, slot.y + 10.0, 28.0, m);
        let key = match i {
            10 => "B".to_string(),
            11 => "N".to_string(),
            _ => ((i + 1) % 10).to_string(),
        };
        style::text(
            hud,
            &key,
            slot.x + 5.0,
            slot.y + 3.0,
            10.0,
            if selected {
                TEXT
            } else {
                [0.85, 0.87, 0.84, 0.7]
            },
            true,
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
