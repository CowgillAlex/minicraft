use crate::editing;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Panel {
    #[default]
    Pause,
    Controls,
    Settings,
    View,
    Graphics,
    Materials(u8),
}
#[derive(Default)]
pub struct UiState {
    pub panel: Panel,
    pub pointer: [f32; 2],
    pub debug: bool,
    pub chunk_timings: bool,
    pub played: bool,
    pub flying: bool,
    pub spectating: bool,
    pub grounded: bool,
    pub ticks: u64,
    pub performance: crate::performance::History,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Resume,
    Controls,
    Settings,
    Back,
    Title,
    NightVision,
    AmbientOcclusion,
    Vsync,
    Grid,
    LocalTime,
    View,
    Graphics,
    GraphicsChange(crate::graphics::Change),
    ViewChange(crate::view_settings::Change),
    BackToSettings,
    Earlier,
    Later,
    Material(u8),
    MaterialPage(u8),
}
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(self, p: [f32; 2]) -> bool {
        p[0] >= self.x && p[0] < self.x + self.w && p[1] >= self.y && p[1] < self.y + self.h
    }
}
pub struct Button {
    pub rect: Rect,
    pub label: &'static str,
    pub action: Action,
    pub primary: bool,
}
pub struct Layout {
    pub scale: f32,
    pub width: f32,
    pub height: f32,
}
impl Layout {
    pub fn new(size: (u32, u32)) -> Self {
        let scale = (size.1 as f32 / 900.0)
            .clamp(1.0, 2.0)
            .min(size.0 as f32 / 800.0)
            .max(0.5);
        Self {
            scale,
            width: size.0 as f32 / scale,
            height: size.1 as f32 / scale,
        }
    }
    pub fn panel(&self, panel: Panel) -> Rect {
        let (w, h) = match panel {
            Panel::Pause => (360.0, 348.0),
            Panel::Controls => (520.0, 516.0),
            Panel::Settings => (440.0, 550.0),
            Panel::View => (480.0, 588.0),
            Panel::Graphics => (480.0, 590.0),
            Panel::Materials(_) => (680.0, 562.0),
        };
        Rect {
            x: (self.width - w) / 2.0,
            y: (self.height - h) / 2.0,
            w,
            h,
        }
    }
    pub fn buttons(&self, panel: Panel) -> Vec<Button> {
        let p = self.panel(panel);
        let mut buttons = Vec::new();
        let mut add = |y, label, action, primary| {
            buttons.push(Button {
                rect: Rect {
                    x: p.x + 24.0,
                    y: p.y + y,
                    w: p.w - 48.0,
                    h: 44.0,
                },
                label,
                action,
                primary,
            })
        };
        match panel {
            Panel::Graphics => {
                use crate::graphics::Change;
                for (i, change) in [
                    Change::Scale,
                    Change::Ao,
                    Change::Shadows,
                    Change::Bloom,
                    Change::Limit,
                    Change::Occlusion,
                    Change::Preset,
                    Change::Reset,
                ]
                .into_iter()
                .enumerate()
                {
                    add(
                        76.0 + i as f32 * 50.0,
                        "",
                        Action::GraphicsChange(change),
                        false,
                    );
                }
                add(520.0, "Back", Action::BackToSettings, false);
            }
            Panel::Pause => {
                add(108.0, "Back to world", Action::Resume, true);
                add(162.0, "Settings", Action::Settings, false);
                add(216.0, "Controls", Action::Controls, false);
                add(270.0, "Title screen", Action::Title, false);
            }
            Panel::View => return crate::ui_view::buttons(self),
            Panel::Controls => add(448.0, "Back", Action::Back, false),
            Panel::Materials(page) => {
                for (x, w, label, action) in [
                    (
                        24.,
                        130.,
                        "Previous",
                        Action::MaterialPage(page.saturating_sub(1)),
                    ),
                    (164., 320., "Back to world", Action::Resume),
                    (
                        494.,
                        162.,
                        "Next",
                        Action::MaterialPage(
                            (page + 1).min(((crate::materials::ALL.len() - 2) / 42) as u8),
                        ),
                    ),
                ] {
                    buttons.push(Button {
                        rect: Rect {
                            x: p.x + x,
                            y: p.y + 494.,
                            w,
                            h: 44.,
                        },
                        label,
                        action,
                        primary: false,
                    });
                }
                for (i, m) in crate::materials::ALL
                    .iter()
                    .filter(|m| m.id != crate::world::AIR)
                    .skip(page as usize * 42)
                    .take(42)
                    .enumerate()
                {
                    buttons.push(Button {
                        rect: Rect {
                            x: p.x + 24.0 + (i % 6) as f32 * 106.0,
                            y: p.y + 76.0 + (i / 6) as f32 * 58.0,
                            w: 100.0,
                            h: 52.0,
                        },
                        label: m.name,
                        action: Action::Material(m.id),
                        primary: false,
                    });
                }
            }
            Panel::Settings => {
                add(64.0, "Graphics & performance", Action::Graphics, false);
                add(110.0, "Distance & fog", Action::View, false);
                add(156.0, "Ambient occlusion", Action::AmbientOcclusion, false);
                add(202.0, "VSync", Action::Vsync, false);
                add(248.0, "Voxel outlines", Action::Grid, false);
                add(294.0, "Night vision", Action::NightVision, false);
                add(340.0, "Use local time", Action::LocalTime, false);
                add(482.0, "Back", Action::Back, false);
                for (offset, label, action) in [
                    (0.0, "-", Action::Earlier),
                    (p.w - 92.0, "+", Action::Later),
                ] {
                    buttons.push(Button {
                        rect: Rect {
                            x: p.x + 24.0 + offset,
                            y: p.y + 420.0,
                            w: 44.0,
                            h: 40.0,
                        },
                        label,
                        action,
                        primary: false,
                    });
                }
            }
        }
        buttons
    }
    pub fn action_at(&self, panel: Panel, physical: [f32; 2]) -> Option<Action> {
        let p = physical.map(|v| v / self.scale);
        self.buttons(panel)
            .into_iter()
            .find(|b| b.rect.contains(p))
            .map(|b| b.action)
    }
    pub fn hotbar(&self) -> Rect {
        let w = editing::PALETTE.len() as f32 * 46.0 + 14.0;
        Rect {
            x: (self.width - w) / 2.0,
            y: self.height - 86.0,
            w,
            h: 64.0,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_buttons_remain_clickable_after_resize_and_dpi_scaling() {
        for (size, play, settings) in [
            ((800, 600), [400.0, 256.0], [400.0, 310.0]),
            ((1440, 900), [720.0, 406.0], [720.0, 460.0]),
            ((3840, 2160), [1920.0, 992.0], [1920.0, 1100.0]),
        ] {
            let layout = Layout::new(size);
            assert_eq!(layout.action_at(Panel::Pause, play), Some(Action::Resume));
            assert_eq!(
                layout.action_at(Panel::Pause, settings),
                Some(Action::Settings)
            );
            assert_eq!(layout.action_at(Panel::Pause, [10.0, 10.0]), None);
            let bar = layout.hotbar();
            assert!(bar.x >= 0.0 && bar.x + bar.w <= layout.width);
        }
        let layout = Layout::new((1440, 900));
        assert_eq!(
            layout.action_at(Panel::Settings, [720.0, 353.0]),
            Some(Action::AmbientOcclusion)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [720.0, 399.0]),
            Some(Action::Vsync)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [546.0, 615.0]),
            Some(Action::Earlier)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [894.0, 615.0]),
            Some(Action::Later)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [720.0, 537.0]),
            Some(Action::LocalTime)
        );
        assert_eq!(
            layout.action_at(Panel::Controls, [720.0, 662.0]),
            Some(Action::Back)
        );
        for size in [(800, 600), (1440, 900), (3840, 2160)] {
            let layout = Layout::new(size);
            for panel in [
                Panel::Settings,
                Panel::Graphics,
                Panel::View,
                Panel::Materials(0),
            ] {
                let bounds = layout.panel(panel);
                for button in layout.buttons(panel) {
                    assert!(button.rect.y + button.rect.h <= layout.height);
                    assert!(
                        button.rect.x >= bounds.x
                            && button.rect.x + button.rect.w <= bounds.x + bounds.w
                    );
                    assert_eq!(
                        layout.action_at(
                            panel,
                            [
                                (button.rect.x + button.rect.w / 2.0) * layout.scale,
                                (button.rect.y + button.rect.h / 2.0) * layout.scale
                            ]
                        ),
                        Some(button.action)
                    );
                }
            }
        }
    }
}
