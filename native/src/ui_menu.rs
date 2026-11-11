use crate::{
    hud::Hud,
    overlay::{OverlayStats, ACCENT, MUTED, TEXT},
    ui::{Action, Layout, Panel, Rect, UiState},
};

pub fn draw(hud: &mut Hud, layout: &Layout, ui: &UiState, stats: &OverlayStats<'_>, vsync: bool) {
    hud.rect(
        0.0,
        0.0,
        layout.width,
        layout.height,
        [0.012, 0.018, 0.025, 0.66],
    );
    let p = layout.panel(ui.panel);
    hud.rounded_rect(
        Rect {
            x: p.x,
            y: p.y + 8.0,
            ..p
        },
        18.0,
        [0.0, 0.0, 0.0, 0.22],
    );
    hud.rounded_rect(p, 16.0, [0.27, 0.32, 0.33, 0.95]);
    hud.rounded_rect(
        Rect {
            x: p.x + 1.0,
            y: p.y + 1.0,
            w: p.w - 2.0,
            h: p.h - 2.0,
        },
        15.0,
        [0.045, 0.057, 0.064, 0.98],
    );
    match ui.panel {
        Panel::Pause => {
            crate::ui_icons::block(hud, p.x + 28.0, p.y + 30.0, 38.0, crate::world::GRASS);
            hud.styled_text("MiniCraft", p.x + 82.0, p.y + 29.0, 32.0, TEXT, true);
            hud.text("World menu", p.x + 84.0, p.y + 68.0, 13.0, MUTED);
        }
        Panel::Controls => {
            hud.styled_text("Controls", p.x + 26.0, p.y + 26.0, 24.0, TEXT, true);
            let rows = [
                ("W A S D", "Move"),
                ("Mouse", "Look around"),
                ("Space / Shift", "Jump / slow walk"),
                ("F / Space / Shift", "Toggle flight / up / down"),
                ("F5 / F6", "Spectator / night vision"),
                ("Ctrl / Scroll", "Boost / adjust speed"),
                ("Left / Right click", "Break / place a voxel"),
                ("Middle click / #", "Pick material"),
                ("E", "Open / close a door"),
                ("1 - 0 / Tab", "Hotbar / all materials"),
                ("B / N", "Glass / door handle"),
                ("- / =", "Earlier / later time"),
                ("F1 / F3", "Controls / debug info"),
                ("H / F11", "Hide UI / fullscreen"),
                ("R / Esc", "Reset view / world menu"),
            ];
            for (i, (key, description)) in rows.iter().enumerate() {
                let y = p.y + 76.0 + i as f32 * 24.0;
                hud.text(key, p.x + 28.0, y, 14.0, ACCENT);
                hud.text(description, p.x + 224.0, y, 14.0, TEXT);
            }
        }
        Panel::Materials => {
            hud.styled_text("Materials", p.x + 26.0, p.y + 24.0, 24.0, TEXT, true);
            hud.text(
                "Choose a material. Tab returns to the world.",
                p.x + 26.0,
                p.y + 54.0,
                12.0,
                MUTED,
            );
        }
        Panel::View => crate::ui_view::draw(hud, layout, stats.view),
        Panel::Settings => {
            hud.text(
                if stats.local_time {
                    "Adjusting time switches back to manual."
                } else {
                    "Sun and moon follow your computer's clock."
                },
                p.x + 26.0,
                p.y + 402.0,
                12.0,
                MUTED,
            );
            hud.styled_text("Settings", p.x + 26.0, p.y + 26.0, 24.0, TEXT, true);
            hud.centered(
                &format!("Time of day   {}", stats.time),
                p.x + p.w / 2.0,
                p.y + 432.0,
                14.0,
                TEXT,
                false,
            );
        }
    }
    let pointer = ui.pointer.map(|v| v / layout.scale);
    for b in layout.buttons(ui.panel) {
        let hovered = b.rect.contains(pointer);
        if let Action::Material(id) = b.action {
            let selected = id == stats.material;
            hud.rounded_rect(
                b.rect,
                6.0,
                if selected {
                    ACCENT
                } else if hovered {
                    [0.20, 0.25, 0.26, 1.0]
                } else {
                    [0.095, 0.12, 0.135, 1.0]
                },
            );
            crate::ui_icons::block(hud, b.rect.x + 40.0, b.rect.y + 3.0, 20.0, id);
            let split = if b.label.len() > 17 {
                b.label[..17].rfind(' ')
            } else {
                None
            };
            let fg = if selected {
                [0.035, 0.075, 0.045, 1.0]
            } else {
                TEXT
            };
            if let Some(i) = split {
                hud.centered(
                    &b.label[..i],
                    b.rect.x + 50.0,
                    b.rect.y + 27.0,
                    10.0,
                    fg,
                    false,
                );
                hud.centered(
                    &b.label[i + 1..],
                    b.rect.x + 50.0,
                    b.rect.y + 38.0,
                    10.0,
                    fg,
                    false,
                );
            } else {
                hud.centered(b.label, b.rect.x + 50.0, b.rect.y + 31.0, 10.0, fg, false);
            }
            continue;
        }
        let bg = if b.primary {
            if hovered {
                [0.73, 0.91, 0.70, 1.0]
            } else {
                ACCENT
            }
        } else if hovered {
            [0.17, 0.21, 0.23, 1.0]
        } else {
            [0.095, 0.12, 0.135, 1.0]
        };
        hud.rounded_rect(b.rect, 8.0, bg);
        let fg = if b.primary {
            [0.035, 0.075, 0.045, 1.0]
        } else if b.action == Action::Title {
            MUTED
        } else {
            TEXT
        };
        let toggle = match b.action {
            Action::AmbientOcclusion => Some(stats.ao_enabled),
            Action::Vsync => Some(vsync),
            Action::Grid => Some(stats.grid),
            Action::NightVision => Some(stats.night_vision),
            Action::LocalTime => Some(stats.local_time),
            Action::ViewChange(crate::view_settings::Change::Fog) => Some(stats.view.fog),
            Action::ViewChange(crate::view_settings::Change::Lod) => Some(stats.view.lod),
            _ => None,
        };
        if let Some(enabled) = toggle {
            hud.text(b.label, b.rect.x + 14.0, b.rect.y + 14.0, 14.0, fg);
            let switch = Rect {
                x: b.rect.x + b.rect.w - 50.0,
                y: b.rect.y + 12.0,
                w: 36.0,
                h: 20.0,
            };
            hud.rounded_rect(
                switch,
                10.0,
                if enabled {
                    ACCENT
                } else {
                    [0.26, 0.30, 0.31, 1.0]
                },
            );
            hud.rounded_rect(
                Rect {
                    x: switch.x + if enabled { 18.0 } else { 2.0 },
                    y: switch.y + 2.0,
                    w: 16.0,
                    h: 16.0,
                },
                8.0,
                if enabled {
                    [0.08, 0.15, 0.10, 1.0]
                } else {
                    [0.65, 0.70, 0.70, 1.0]
                },
            );
        } else {
            let label = if b.action == Action::Resume && !ui.played {
                "Play"
            } else {
                b.label
            };
            hud.centered(
                label,
                b.rect.x + b.rect.w / 2.0,
                b.rect.y + 14.0,
                15.0,
                fg,
                b.primary,
            );
        }
    }
    if ui.panel == Panel::Pause {
        hud.centered(
            &format!("v{}", env!("CARGO_PKG_VERSION")),
            layout.width / 2.0,
            p.y + p.h + 16.0,
            12.0,
            [0.7, 0.74, 0.73, 0.7],
            false,
        );
    }
}
