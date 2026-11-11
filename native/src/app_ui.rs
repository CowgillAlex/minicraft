use super::App;
use crate::ui;
use winit::{
    dpi::PhysicalPosition,
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::{CursorGrabMode, CursorIcon},
};

impl App {
    pub(super) fn reset_player(&mut self) {
        if let Some(renderer) = &self.renderer {
            self.simulation.reset(renderer.world());
            let position = self.simulation.eye_position();
            let direction = if renderer.world().noise_preview.is_some() {
                glam::Vec3::new(1.0, -0.12, -0.7).normalize()
            } else {
                (crate::world::World::meters(155, 40, 205) - position).normalize()
            };
            self.camera = crate::camera::Camera {
                position,
                yaw: direction.z.atan2(direction.x),
                pitch: direction.y.asin(),
                ..Default::default()
            };
        }
    }
    pub(super) fn capture_cursor(&mut self, capture: bool) {
        self.last_frame = std::time::Instant::now();
        if !capture {
            self.simulation.clear_input();
            if let Some(renderer) = &mut self.renderer {
                renderer.hud_enabled = true;
            }
        }
        if let Some(window) = &self.window {
            let result = if capture {
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            } else {
                window.set_cursor_grab(CursorGrabMode::None)
            };
            self.captured = capture && result.is_ok();
            if self.captured {
                if let Some(renderer) = &mut self.renderer {
                    renderer.ui.played = true;
                    renderer.ui.panel = ui::Panel::Pause;
                }
            }
            window.set_cursor_visible(!self.captured);
            if !self.captured {
                self.keys.clear();
            }
        }
    }
    fn ui_action(&mut self, action: ui::Action) {
        use ui::{Action, Panel};
        match action {
            Action::Resume => self.capture_cursor(true),
            Action::Material(id) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.selected_material = id;
                }
                self.capture_cursor(true);
            }
            Action::Title => self.return_to_title(),
            Action::Vsync => {
                self.vsync = !self.vsync;
                if let Some(window) = &self.window {
                    self.resize(window.inner_size());
                }
            }
            _ => {
                if let Some(renderer) = &mut self.renderer {
                    match action {
                        Action::Controls => renderer.ui.panel = Panel::Controls,
                        Action::Settings | Action::BackToSettings => {
                            renderer.ui.panel = Panel::Settings
                        }
                        Action::View => renderer.ui.panel = Panel::View,
                        Action::MaterialPage(page) => renderer.ui.panel = Panel::Materials(page),
                        Action::ViewChange(change) => renderer.view.change(change),
                        Action::Back => renderer.ui.panel = Panel::Pause,
                        Action::AmbientOcclusion => renderer.ao_enabled = !renderer.ao_enabled,
                        Action::Grid => renderer.grid = !renderer.grid,
                        Action::NightVision => renderer.night_vision = !renderer.night_vision,
                        Action::LocalTime => renderer.toggle_local_time(),
                        Action::Earlier => renderer.adjust_time(-0.5),
                        Action::Later => renderer.adjust_time(0.5),
                        _ => {}
                    }
                }
            }
        }
    }
    pub(super) fn handle_ui_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Tab => {
                if !self.captured
                    && self
                        .renderer
                        .as_ref()
                        .is_some_and(|r| matches!(r.ui.panel, ui::Panel::Materials(_)))
                {
                    self.capture_cursor(true);
                } else {
                    self.capture_cursor(false);
                    if let Some(r) = &mut self.renderer {
                        r.ui.panel = ui::Panel::Materials(0);
                    }
                }
            }
            KeyCode::Escape => {
                if self.captured {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.ui.panel = ui::Panel::Pause;
                    }
                    self.capture_cursor(false);
                } else if self
                    .renderer
                    .as_ref()
                    .is_some_and(|r| r.ui.panel != ui::Panel::Pause)
                {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.ui.panel = if renderer.ui.panel == ui::Panel::View {
                            ui::Panel::Settings
                        } else {
                            ui::Panel::Pause
                        };
                    }
                } else {
                    self.capture_cursor(true);
                }
            }
            KeyCode::Enter if !self.captured => self.capture_cursor(true),
            KeyCode::F1 => {
                if !self.captured
                    && self
                        .renderer
                        .as_ref()
                        .is_some_and(|r| r.ui.panel == ui::Panel::Controls)
                {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.ui.panel = ui::Panel::Pause;
                    }
                } else {
                    self.capture_cursor(false);
                    if let Some(renderer) = &mut self.renderer {
                        renderer.ui.panel = ui::Panel::Controls;
                    }
                }
            }
            KeyCode::F6 => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.night_vision = !renderer.night_vision;
                }
            }
            KeyCode::F3 => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.ui.debug = !renderer.ui.debug;
                }
            }
            KeyCode::KeyH if self.captured => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.hud_enabled = !renderer.hud_enabled;
                }
            }
            _ => return false,
        }
        true
    }
    pub(super) fn click_menu(&mut self, event_loop: &ActiveEventLoop) {
        if let (Some(title), Some(window)) = (&mut self.title, &self.window) {
            let size = window.inner_size();
            match crate::title::action_at((size.width, size.height), title.pointer) {
                Some(crate::title::Action::Seed) => {
                    title.focused = true;
                    title.selected = true;
                }
                Some(crate::title::Action::Create) => self.begin_world(event_loop),
                Some(crate::title::Action::Quit) => event_loop.exit(),
                None => title.focused = false,
            }
            return;
        }
        let action =
            self.window
                .as_ref()
                .zip(self.renderer.as_ref())
                .and_then(|(window, renderer)| {
                    let size = window.inner_size();
                    ui::Layout::new((size.width, size.height))
                        .action_at(renderer.ui.panel, renderer.ui.pointer)
                });
        if let Some(action) = action {
            self.ui_action(action);
        }
    }
    pub(super) fn move_ui_cursor(&mut self, position: PhysicalPosition<f64>) {
        self.pointer = [position.x as f32, position.y as f32];
        if let (Some(window), Some(title)) = (&self.window, &mut self.title) {
            title.pointer = [position.x as f32, position.y as f32];
            let size = window.inner_size();
            window.set_cursor(
                match crate::title::action_at((size.width, size.height), title.pointer) {
                    Some(crate::title::Action::Seed) => CursorIcon::Text,
                    Some(_) => CursorIcon::Pointer,
                    None => CursorIcon::Default,
                },
            );
            return;
        }
        if let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer) {
            renderer.ui.pointer = [position.x as f32, position.y as f32];
            let size = window.inner_size();
            let hover = ui::Layout::new((size.width, size.height))
                .action_at(renderer.ui.panel, renderer.ui.pointer)
                .is_some();
            window.set_cursor(if hover {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            });
        }
    }
}
