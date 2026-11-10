#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod camera;
mod capture;
mod capture_views;
mod editing;
mod hud;
mod mesh;
mod occlusion;
mod pipelines;
mod renderer;
mod scene_mesh;
mod world;

use camera::Camera;
use renderer::{FrameInfo, Renderer};
use std::{collections::HashSet, error::Error, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Fullscreen, Window, WindowId},
};

struct App {
    instance: wgpu::Instance,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    renderer: Option<Renderer>,
    config: Option<wgpu::SurfaceConfiguration>,
    camera: Camera,
    keys: HashSet<KeyCode>,
    captured: bool,
    occluded: bool,
    vsync: bool,
    last_frame: Instant,
    stats_since: Instant,
    frames: u32,
    fps: f32,
    error: Option<String>,
}
impl App {
    fn new() -> Self {
        Self {
            instance: wgpu::Instance::default(),
            window: None,
            surface: None,
            renderer: None,
            config: None,
            camera: Camera::default(),
            keys: HashSet::new(),
            captured: false,
            occluded: false,
            vsync: true,
            last_frame: Instant::now(),
            stats_since: Instant::now(),
            frames: 0,
            fps: 0.0,
            error: None,
        }
    }
    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if let (Some(renderer), Some(surface), Some(config)) =
            (&self.renderer, &self.surface, &mut self.config)
        {
            config.width = size.width;
            config.height = size.height;
            config.present_mode = if self.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            };
            surface.configure(&renderer.device, config);
        }
    }
    fn capture_cursor(&mut self, capture: bool) {
        if let Some(window) = &self.window {
            let result = if capture {
                window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            } else {
                window.set_cursor_grab(CursorGrabMode::None)
            };
            self.captured = capture && result.is_ok();
            window.set_cursor_visible(!self.captured);
            if !self.captured {
                self.keys.clear();
            }
        }
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        if self.captured {
            self.camera.update(&self.keys, dt);
        }
        let (Some(surface), Some(renderer), Some(config)) =
            (&self.surface, &mut self.renderer, &self.config)
        else {
            return;
        };
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&renderer.device, config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                self.error = Some("GPU ran out of memory".into());
                event_loop.exit();
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
        };
        let view = frame.texture.create_view(&Default::default());
        renderer.render(
            &view,
            &self.camera,
            FrameInfo {
                size: (config.width, config.height),
                fps: self.fps,
                captured: self.captured,
                vsync: self.vsync,
            },
        );
        if let Some(window) = &self.window {
            window.pre_present_notify();
        }
        frame.present();
        self.frames += 1;
        let interval = self.stats_since.elapsed().as_secs_f32();
        if interval >= 0.5 {
            self.fps = self.frames as f32 / interval;
            self.frames = 0;
            self.stats_since = now;
            if let Some(window) = &self.window {
                window.set_title(&format!(
                    "Cedar Voxel Lab v{} | {:.0} FPS | {} | 6.25cm voxels",
                    env!("CARGO_PKG_VERSION"),
                    self.fps,
                    renderer.adapter_name
                ));
            }
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> renderer::Result<()> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Cedar Voxel Lab - building world...")
                        .with_inner_size(PhysicalSize::new(1440, 900))
                        .with_min_inner_size(PhysicalSize::new(800, 600)),
                )?,
            );
            let surface = self.instance.create_surface(window.clone())?;
            let renderer = pollster::block_on(Renderer::new(&self.instance, Some(&surface)))?;
            let size = window.inner_size();
            self.config = Some(wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: renderer.format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            });
            surface.configure(&renderer.device, self.config.as_ref().unwrap());
            self.window = Some(window);
            self.surface = Some(surface);
            self.renderer = Some(renderer);
            self.last_frame = Instant::now();
            self.stats_since = Instant::now();
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error.to_string());
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size),
            WindowEvent::Focused(false) => self.capture_cursor(false),
            WindowEvent::Occluded(value) => {
                self.occluded = value;
                self.keys.clear();
            }
            WindowEvent::RedrawRequested => self.draw(event_loop),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if !self.captured {
                    if button == MouseButton::Left {
                        self.capture_cursor(true);
                    }
                } else if let Some(renderer) = &mut self.renderer {
                    match button {
                        MouseButton::Left => {
                            renderer.edit(&self.camera, false);
                        }
                        MouseButton::Right => {
                            renderer.edit(&self.camera, true);
                        }
                        MouseButton::Middle => renderer.pick_material(&self.camera),
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.camera.speed = (self.camera.speed * 1.15_f32.powf(lines)).clamp(0.3, 40.0);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Released {
                        self.keys.remove(&key);
                        return;
                    }
                    if self.captured {
                        self.keys.insert(key);
                    }
                    if event.repeat {
                        return;
                    }
                    match key {
                        KeyCode::Digit1
                        | KeyCode::Digit2
                        | KeyCode::Digit3
                        | KeyCode::Digit4
                        | KeyCode::Digit5
                        | KeyCode::Digit6
                        | KeyCode::Digit7
                        | KeyCode::Digit8
                        | KeyCode::Digit9
                        | KeyCode::Digit0 => {
                            let index = match key {
                                KeyCode::Digit1 => 0,
                                KeyCode::Digit2 => 1,
                                KeyCode::Digit3 => 2,
                                KeyCode::Digit4 => 3,
                                KeyCode::Digit5 => 4,
                                KeyCode::Digit6 => 5,
                                KeyCode::Digit7 => 6,
                                KeyCode::Digit8 => 7,
                                KeyCode::Digit9 => 8,
                                _ => 9,
                            };
                            if let Some(renderer) = &mut self.renderer {
                                renderer.selected_material = editing::PALETTE[index];
                            }
                        }
                        KeyCode::Escape => self.capture_cursor(false),
                        KeyCode::KeyR => self.camera = Camera::default(),
                        KeyCode::KeyH => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.hud_enabled = !renderer.hud_enabled;
                            }
                        }
                        KeyCode::KeyO => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.ao_enabled = !renderer.ao_enabled;
                            }
                        }
                        KeyCode::KeyG => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.grid = !renderer.grid;
                            }
                        }
                        KeyCode::KeyV => {
                            self.vsync = !self.vsync;
                            if let Some(window) = &self.window {
                                self.resize(window.inner_size());
                            }
                        }
                        KeyCode::F11 => {
                            if let Some(window) = &self.window {
                                window.set_fullscreen(if window.fullscreen().is_some() {
                                    None
                                } else {
                                    Some(Fullscreen::Borderless(None))
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if self.captured {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.camera.look(delta.0, delta.1);
            }
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if !self.occluded {
            if let Some(window) = &self.window {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    window.request_redraw();
                }
            }
        }
    }
}
fn run() -> std::result::Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("Cedar Voxel Lab v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if let Some(index) = args.iter().position(|arg| arg == "--screenshot") {
        let path = args.get(index + 1).ok_or("--screenshot needs a PNG path")?;
        let frames = args
            .iter()
            .position(|arg| arg == "--frames")
            .and_then(|index| args.get(index + 1))
            .map(|n| n.parse::<u32>())
            .transpose()?
            .unwrap_or(1)
            .clamp(1, 1000);
        return capture::capture(
            std::path::Path::new(path),
            frames,
            args.iter().any(|arg| arg == "--verify-edits"),
            args.iter().any(|arg| arg == "--ao-closeup"),
            !args.iter().any(|arg| arg == "--no-ao"),
            args.iter()
                .position(|arg| arg == "--shadow-view")
                .and_then(|index| args.get(index + 1))
                .map(String::as_str),
        );
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}
fn main() {
    std::panic::set_hook(Box::new(|info| report_error(&info.to_string())));
    if let Err(error) = run() {
        report_error(&error.to_string());
        std::process::exit(1);
    }
}
fn report_error(error: &str) {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("voxel-drive-error.txt")))
        .unwrap_or_else(|| "voxel-drive-error.txt".into());
    let _ = std::fs::write(
        &path,
        format!(
            "Cedar Voxel Lab v{}\n{}\n",
            env!("CARGO_PKG_VERSION"),
            error
        ),
    );
    eprintln!("{error}\nDetails saved to {}", path.display());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                kind: u32,
            ) -> i32;
        }
        let text: Vec<u16> = std::ffi::OsStr::new(&format!(
            "Could not start the renderer:\n\n{error}\n\nDetails were saved beside the executable."
        ))
        .encode_wide()
        .chain(Some(0))
        .collect();
        let title: Vec<u16> = std::ffi::OsStr::new("Cedar Voxel Lab")
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
}
