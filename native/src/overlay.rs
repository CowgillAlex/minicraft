use crate::{
    camera::Camera,
    editing::{self, Hit},
    hud::{Hud, HudVertex},
};
pub struct OverlayStats<'a> {
    pub voxels: usize,
    pub triangles: usize,
    pub chunks: usize,
    pub ao_enabled: bool,
    pub material: u8,
    pub target: Option<Hit>,
    pub time: &'a str,
    pub lighting_pending: bool,
}
pub fn vertices(
    camera: &Camera,
    size: (u32, u32),
    fps: f32,
    captured: bool,
    vsync: bool,
    stats: OverlayStats<'_>,
) -> Vec<HudVertex> {
    let mut hud = Hud::new(size.0, size.1);
    let text = [0.78, 0.87, 0.83, 1.0];
    let muted = [0.43, 0.58, 0.54, 1.0];
    let accent = [0.31, 0.84, 0.62, 1.0];
    hud.rect(16.0, 16.0, 350.0, 151.0, [0.015, 0.028, 0.027, 0.88]);
    hud.rect(16.0, 16.0, 3.0, 151.0, accent);
    hud.text("CEDAR / VOXEL LAB", 31.0, 29.0, 2.0, text);
    hud.text(
        &format!(
            "V{} / {} / AO {}",
            env!("CARGO_PKG_VERSION"),
            stats.time,
            if stats.ao_enabled { "ON" } else { "OFF" }
        ),
        31.0,
        52.0,
        1.5,
        muted,
    );
    hud.text(
        &format!(
            "{:3.0} FPS   {:4.1} MS   {}",
            fps,
            if fps > 0.0 { 1000.0 / fps } else { 0.0 },
            if vsync { "VSYNC" } else { "UNCAPPED" }
        ),
        31.0,
        76.0,
        1.5,
        accent,
    );
    hud.text(
        &format!("{:.2}M VOXELS  /  6.25 CM", stats.voxels as f32 / 1e6),
        31.0,
        97.0,
        1.5,
        text,
    );
    hud.text(
        &format!(
            "{}K TRIS  /  {} CHUNKS",
            stats.triangles / 1000,
            stats.chunks
        ),
        31.0,
        116.0,
        1.5,
        text,
    );
    hud.text(
        &format!(
            "XYZ {:.1} {:.1} {:.1}",
            camera.position.x, camera.position.y, camera.position.z
        ),
        31.0,
        139.0,
        1.5,
        muted,
    );
    let palette_width = editing::PALETTE.len() as f32 * 33.0 + 28.0;
    let palette_x = size.0 as f32 - palette_width - 16.0;
    hud.rect(
        palette_x,
        16.0,
        palette_width,
        66.0,
        [0.015, 0.028, 0.027, 0.88],
    );
    hud.text(
        "1-0 MATERIAL / B GLASS / MMB PICK",
        palette_x + 12.0,
        26.0,
        1.0,
        muted,
    );
    for (index, material) in editing::PALETTE.iter().enumerate() {
        let x = palette_x + 13.0 + index as f32 * 33.0;
        if *material == stats.material {
            hud.rect(x - 2.0, 42.0, 28.0, 28.0, accent);
        }
        let color = crate::world::color(*material);
        hud.rect(x, 44.0, 24.0, 24.0, [color[0], color[1], color[2], 1.0]);
        hud.text(
            &if index == 10 {
                "B".to_owned()
            } else {
                ((index + 1) % 10).to_string()
            },
            x + 3.0,
            47.0,
            1.0,
            [0.015, 0.028, 0.027, 1.0],
        );
    }
    let y = size.1 as f32 - 118.0;
    hud.rect(
        16.0,
        y,
        if size.0 < 1000 { 610.0 } else { 830.0 },
        102.0,
        [0.015, 0.028, 0.027, 0.87],
    );
    hud.text(
        if captured {
            "WASD MOVE / SPACE UP / SHIFT DOWN / CTRL BOOST"
        } else {
            "CLICK TO EXPLORE / SPACE UP / SHIFT DOWN / CTRL BOOST"
        },
        29.0,
        y + 12.0,
        1.5,
        text,
    );
    hud.text(
        "LMB BREAK / RMB PLACE / 1-0 MATERIAL / B GLASS / SCROLL SPEED",
        29.0,
        y + 34.0,
        if size.0 < 1000 { 1.0 } else { 1.5 },
        muted,
    );
    hud.text(
        "- / = TIME / ESC RELEASE / R RESET / H HUD / G GRID / O AO / V VSYNC",
        29.0,
        y + 55.0,
        if size.0 < 1000 { 1.0 } else { 1.5 },
        muted,
    );
    let material = format!(
        "PLACE {} / {}",
        editing::material_name(stats.material),
        stats
            .target
            .map(|hit| format!(
                "TARGET {} {} {} / {:.2}M",
                hit.cell[0], hit.cell[1], hit.cell[2], hit.distance
            ))
            .unwrap_or_else(|| "REACH 8M / NO TARGET".into())
    );
    hud.text(
        &format!(
            "{}{}",
            material,
            if stats.lighting_pending {
                " / LIGHT BAKE"
            } else {
                ""
            }
        ),
        29.0,
        y + 77.0,
        1.5,
        accent,
    );
    if captured {
        let x = size.0 as f32 / 2.0;
        let y = size.1 as f32 / 2.0;
        hud.rect(x - 4.0, y, 8.0, 1.0, [0.8, 0.95, 0.87, 0.7]);
        hud.rect(x, y - 4.0, 1.0, 8.0, [0.8, 0.95, 0.87, 0.7]);
    }
    hud.vertices
}
