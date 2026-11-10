use crate::editing;

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Panel {
    #[default]
    Pause,
    Controls,
    Settings,
}
#[derive(Default)]
pub struct UiState {
    pub panel: Panel,
    pub pointer: [f32; 2],
    pub debug: bool,
    pub played: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Resume,
    Controls,
    Settings,
    Back,
    Quit,
    AmbientOcclusion,
    Vsync,
    Grid,
    Earlier,
    Later,
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
            Panel::Controls => (520.0, 478.0),
            Panel::Settings => (440.0, 382.0),
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
            Panel::Pause => {
                add(108.0, "Back to world", Action::Resume, true);
                add(162.0, "Settings", Action::Settings, false);
                add(216.0, "Controls", Action::Controls, false);
                add(270.0, "Quit game", Action::Quit, false);
            }
            Panel::Controls => add(410.0, "Back", Action::Back, false),
            Panel::Settings => {
                add(80.0, "Ambient occlusion", Action::AmbientOcclusion, false);
                add(134.0, "VSync", Action::Vsync, false);
                add(188.0, "Voxel outlines", Action::Grid, false);
                add(314.0, "Back", Action::Back, false);
                for (offset, label, action) in [
                    (0.0, "-", Action::Earlier),
                    (p.w - 92.0, "+", Action::Later),
                ] {
                    buttons.push(Button {
                        rect: Rect {
                            x: p.x + 24.0 + offset,
                            y: p.y + 252.0,
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
        let w = editing::PALETTE.len() as f32 * 50.0 + 14.0;
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
            layout.action_at(Panel::Settings, [720.0, 361.0]),
            Some(Action::AmbientOcclusion)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [720.0, 415.0]),
            Some(Action::Vsync)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [546.0, 531.0]),
            Some(Action::Earlier)
        );
        assert_eq!(
            layout.action_at(Panel::Settings, [894.0, 531.0]),
            Some(Action::Later)
        );
        assert_eq!(
            layout.action_at(Panel::Controls, [720.0, 643.0]),
            Some(Action::Back)
        );
    }
}
