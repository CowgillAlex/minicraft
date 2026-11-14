use crate::{
    hud::Hud,
    overlay::OverlayStats,
    ui::{Action, Button, Layout, Panel, Rect, UiState},
    ui_style::{self as style, ACCENT, KEY, MUTED, TEXT},
};

fn heading(hud: &mut Hud, p: Rect, title: &str) {
    style::centered(hud, title, p.x + p.w / 2.0, p.y + 22.0, 24.0, TEXT, true);
}

fn note(hud: &mut Hud, text: &str, x: f32, y: f32) {
    style::text(hud, text, x, y, 12.0, MUTED, false);
}

/// Split "Render scale: 100%" into its name and value.
fn split_label(label: &str) -> (&str, &str) {
    label.split_once(": ").unwrap_or((label, ""))
}

pub fn draw(hud: &mut Hud, layout: &Layout, ui: &UiState, stats: &OverlayStats<'_>, vsync: bool) {
    hud.rect(
        0.0,
        0.0,
        layout.width,
        layout.height,
        [0.02, 0.025, 0.035, 0.62],
    );
    let p = layout.panel(ui.panel);
    style::panel(hud, p);
    match ui.panel {
        Panel::Pause => {
            style::centered(
                hud,
                "MiniCraft",
                p.x + p.w / 2.0,
                p.y + 30.0,
                36.0,
                TEXT,
                true,
            );
            style::tracked_centered(
                hud,
                "WORLD MENU",
                p.x + p.w / 2.0,
                p.y + 78.0,
                10.0,
                3.0,
                MUTED,
            );
            style::centered(
                hud,
                &format!("v{}", env!("CARGO_PKG_VERSION")),
                p.x + p.w / 2.0,
                p.y + p.h - 26.0,
                11.0,
                MUTED,
                false,
            );
        }
        Panel::Controls => {
            heading(hud, p, "Controls");
            let rows: [(&[&str], &str); 15] = [
                (&["W", "A", "S", "D"], "Move"),
                (&["Mouse"], "Look around"),
                (&["Space", "Shift"], "Jump / slow walk"),
                (&["F", "Space", "Shift"], "Toggle flight / up / down"),
                (&["F5", "F6"], "Spectator / night vision"),
                (&["Ctrl", "Scroll"], "Boost / adjust speed"),
                (&["LMB", "RMB"], "Break / place a voxel"),
                (&["MMB", "#"], "Pick material"),
                (&["E"], "Open / close a door"),
                (&["1-0", "Tab"], "Hotbar / inventory"),
                (&["B", "N"], "Hotbar slots 11 / 12"),
                (&["-", "="], "Earlier / later time"),
                (&["F1", "F3", "F7"], "Controls / debug / chunk timings"),
                (&["H", "F11"], "Hide UI / fullscreen"),
                (&["R", "Esc"], "Reset view / world menu"),
            ];
            for (i, (keys, description)) in rows.iter().enumerate() {
                let y = p.y + 70.0 + i as f32 * 24.5;
                let mut x = p.x + 28.0;
                for key in keys.iter() {
                    x += style::keycap(hud, key, x, y) + 6.0;
                }
                style::text(hud, description, p.x + 214.0, y + 3.0, 14.0, TEXT, false);
            }
        }
        Panel::Materials(_) => {
            heading(hud, p, "Inventory");
            style::centered(
                hud,
                "Click a block to put it in the selected hotbar slot.",
                p.x + p.w / 2.0,
                p.y + 52.0,
                12.0,
                MUTED,
                false,
            );
            let bar_x = p.x + (p.w - (12.0 * 51.0 - 4.0)) / 2.0;
            style::tracked(hud, "HOTBAR", bar_x, p.y + 422.0, 11.0, 3.0, TEXT);
            style::right(
                hud,
                "Click a slot to choose which one to fill",
                p.x + p.w - bar_x + p.x,
                p.y + 422.0,
                11.0,
                MUTED,
                false,
            );
            if let Panel::Materials(page) = ui.panel {
                style::centered(
                    hud,
                    &format!("Page {} / {}", page + 1, crate::ui::material_pages()),
                    p.x + 138.0,
                    p.y + 523.0,
                    14.0,
                    TEXT,
                    true,
                );
            }
        }
        Panel::View => {
            heading(hud, p, "Distance & fog");
            crate::ui_view::draw(hud, layout, stats.view);
        }
        Panel::Graphics => {
            heading(hud, p, "Graphics & performance");
            note(
                hud,
                "Menus: 30 FPS. Background: 10 FPS.",
                p.x + 26.0,
                p.y + 480.0,
            );
        }
        Panel::Settings => {
            heading(hud, p, "Settings");
            note(
                hud,
                if stats.local_time {
                    "Adjusting time switches back to manual."
                } else {
                    "Sun and moon follow your computer's clock."
                },
                p.x + 26.0,
                p.y + 396.0,
            );
            let well = Rect {
                x: p.x + 76.0,
                y: p.y + 420.0,
                w: p.w - 152.0,
                h: 40.0,
            };
            style::well(hud, well);
            style::centered(
                hud,
                &format!("Time of day  {}", stats.time),
                well.x + well.w / 2.0,
                well.y + 11.0,
                15.0,
                TEXT,
                false,
            );
        }
    }
    let pointer = ui.pointer.map(|v| v / layout.scale);
    let mut tooltip = None;
    for b in layout.buttons(ui.panel) {
        let hovered = b.rect.contains(pointer);
        if hovered {
            tooltip = match b.action {
                Action::Material(_) => Some(b.label),
                Action::HotbarSlot(i) => Some(crate::editing::material_name(
                    stats.hotbar.slots[i as usize],
                )),
                _ => None,
            };
        }
        draw_button(hud, &b, hovered, ui, stats, vsync);
    }
    if let Some(name) = tooltip {
        draw_tooltip(hud, layout, pointer, name);
    }
}

fn draw_tooltip(hud: &mut Hud, layout: &Layout, pointer: [f32; 2], name: &str) {
    let w = Hud::text_width(name, 14.0, true) + 20.0;
    let r = Rect {
        x: (pointer[0] + 14.0).min(layout.width - w - 6.0),
        y: (pointer[1] + 18.0).min(layout.height - 34.0),
        w,
        h: 28.0,
    };
    style::block(hud, r, style::PANEL, 2.0, 2.0, 4.0);
    style::text(hud, name, r.x + 10.0, r.y + 6.0, 14.0, TEXT, true);
}

/// A recessed inventory slot with a large block preview.
fn draw_slot(hud: &mut Hud, r: Rect, material: u8, hovered: bool, selected: bool) {
    if selected {
        hud.rect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, TEXT);
    }
    style::bevel(
        hud,
        r,
        if selected {
            style::BUTTON
        } else if hovered {
            style::PANEL
        } else {
            style::WELL
        },
        2.0,
    );
    let icon = (r.w * 0.6).round();
    crate::ui_icons::block(
        hud,
        r.x + (r.w - icon) / 2.0,
        r.y + (r.h - icon) / 2.0 + 1.0,
        icon,
        material,
    );
}

fn draw_button(
    hud: &mut Hud,
    b: &Button,
    hovered: bool,
    ui: &UiState,
    stats: &OverlayStats<'_>,
    vsync: bool,
) {
    let r = b.rect;
    if let Action::Material(id) = b.action {
        draw_slot(hud, r, id, hovered, id == stats.material);
        // Blocks already on the hotbar show which key selects them.
        if let Some(slot) = stats.hotbar.slots.iter().position(|&m| m == id) {
            style::text(
                hud,
                crate::editing::Hotbar::key(slot),
                r.x + 5.0,
                r.y + 3.0,
                10.0,
                ACCENT,
                true,
            );
        }
        return;
    }
    if let Action::HotbarSlot(i) = b.action {
        let selected = i as usize == stats.hotbar.selected;
        draw_slot(hud, r, stats.hotbar.slots[i as usize], hovered, selected);
        if selected {
            hud.rect(r.x, r.y + r.h - 3.0, r.w, 3.0, ACCENT);
        }
        style::text(hud, b.label, r.x + 4.0, r.y + 2.0, 10.0, TEXT, true);
        return;
    }
    if let Action::MaterialPage(target) = b.action {
        let enabled = matches!(ui.panel, Panel::Materials(page) if page != target);
        style::button(hud, style::inset(r, 3.0), false, hovered && enabled);
        let color = if enabled { TEXT } else { [0.6, 0.62, 0.6, 0.5] };
        let (x, y) = (r.x + r.w / 2.0 - 4.0, r.y + r.h / 2.0 - 6.0);
        if b.label == "Next" {
            style::chevron(hud, x, y, color);
        } else {
            style::chevron_left(hud, x, y, color);
        }
        return;
    }
    let toggle = match b.action {
        Action::AmbientOcclusion => Some(stats.ao_enabled),
        Action::Vsync => Some(vsync),
        Action::Grid => Some(stats.grid),
        Action::NightVision => Some(stats.night_vision),
        Action::LocalTime => Some(stats.local_time),
        Action::ViewChange(crate::view_settings::Change::Fog) => Some(stats.view.fog),
        Action::ViewChange(crate::view_settings::Change::Lod) => Some(stats.view.lod),
        Action::GraphicsChange(crate::graphics::Change::Bloom) => Some(stats.graphics.bloom),
        Action::GraphicsChange(crate::graphics::Change::Occlusion) => {
            Some(stats.graphics.occlusion)
        }
        _ => None,
    };
    let cycle = match b.action {
        Action::GraphicsChange(
            change @ (crate::graphics::Change::Scale
            | crate::graphics::Change::Ao
            | crate::graphics::Change::Shadows
            | crate::graphics::Change::Limit),
        ) => Some(stats.graphics.label(change)),
        _ => None,
    };
    if toggle.is_some() || cycle.is_some() {
        // Option rows are recessed so they read differently from buttons.
        let row = style::inset(r, 1.0);
        style::bevel(
            hud,
            row,
            if hovered { style::PANEL } else { style::WELL },
            2.0,
        );
        let full;
        let name = match (&cycle, b.action) {
            (Some(label), _) => split_label(label).0,
            (None, Action::GraphicsChange(change)) => {
                full = stats.graphics.label(change);
                split_label(&full).0
            }
            _ => b.label,
        };
        style::text(hud, name, r.x + 14.0, r.y + 13.0, 14.0, TEXT, false);
        if let Some(enabled) = toggle {
            style::switch(hud, r.x + r.w - 62.0, r.y + 10.0, enabled);
        } else if let Some(label) = &cycle {
            let value = split_label(label).1;
            style::right(hud, value, r.x + r.w - 32.0, r.y + 13.0, 14.0, KEY, true);
            style::chevron(hud, r.x + r.w - 22.0, r.y + 16.0, ACCENT);
        }
        return;
    }
    style::button(hud, style::inset(r, 3.0), b.primary, hovered);
    let label = match b.action {
        Action::GraphicsChange(change) => stats.graphics.label(change),
        Action::Resume if !ui.played && ui.panel == Panel::Pause => "Play".into(),
        _ => b.label.into(),
    };
    let size = if r.h >= 44.0 && r.w > 100.0 {
        17.0
    } else {
        18.0
    };
    style::centered(
        hud,
        &label,
        r.x + r.w / 2.0,
        r.y + (r.h - size) / 2.0 - 1.0,
        size,
        TEXT,
        true,
    );
    if matches!(b.action, Action::Graphics | Action::View) {
        style::chevron(hud, r.x + r.w - 26.0, r.y + r.h / 2.0 - 6.0, TEXT);
    }
}
