#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app_startup;
mod app_ui;
mod bloom;
mod camera;
mod capture;
mod capture_views;
mod daylight;
mod door_gpu;
mod doors;
mod editing;
mod gpu;
mod hud;
mod light_field;
mod light_volume;
mod mesh;
mod occlusion;
mod overlay;
mod physics;
mod picking;
mod pipelines;
mod ray;
mod renderer;
mod scene_mesh;
mod simulation;
mod startup;
mod startup_ui;
mod ui;
mod ui_icons;
mod ui_menu;
mod water;
mod world;
mod worldgen;

use camera::Camera;
use renderer::{FrameInfo, Renderer};
use std::{collections::HashSet, error::Error, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

struct App {
    scene: worldgen::preview::Scene,
    instance: Arc<wgpu::Instance>,
    window: Option<Arc<Window>>,
    surface: Option<Arc<wgpu::Surface<'static>>>,
    renderer: Option<Renderer>,
    startup: Option<startup::Startup>,
    loading: Option<startup_ui::LoadingScreen>,
    config: Option<wgpu::SurfaceConfiguration>,
    camera: Camera,
    simulation: simulation::Simulation,
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
    fn new(scene: worldgen::preview::Scene) -> Self {
        Self {
            scene,
            instance: Arc::new(wgpu::Instance::default()),
            window: None,
            surface: None,
            renderer: None,
            startup: None,
            loading: None,
            config: None,
            camera: Camera::default(),
            simulation: simulation::Simulation::default(),
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
    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_startup(event_loop);
        if self.renderer.is_none() {
            self.draw_loading(event_loop);
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        if self.captured {
            if let Some(renderer) = &self.renderer {
                self.simulation.advance(
                    renderer.world(),
                    physics::Input::from_keys(&self.keys, self.camera.yaw, self.camera.speed),
                    dt,
                );
            }
        }
        self.camera.position = self.simulation.eye_position();
        let (Some(surface), Some(renderer), Some(config)) =
            (&self.surface, &mut self.renderer, &self.config)
        else {
            return;
        };
        renderer.ui.flying = self.simulation.player.mode == physics::Mode::Fly;
        renderer.ui.grounded = self.simulation.player.grounded;
        renderer.ui.ticks = self.simulation.ticks;
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
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.start_loading(event_loop);
    }
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            && self.startup.is_some()
            && !self.occluded
        {
            if let Some(window) = &self.window {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    window.request_redraw();
                }
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(startup) = &self.startup {
                    startup.progress.cancel();
                }
                event_loop.exit();
            }
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
                        self.click_menu(event_loop);
                    }
                } else if let Some(renderer) = &mut self.renderer {
                    match button {
                        MouseButton::Left => {
                            renderer.edit(&self.camera, false);
                        }
                        MouseButton::Right => {
                            if renderer
                                .target_cell(&self.camera, true)
                                .is_some_and(|cell| !self.simulation.blocks_placement(cell))
                            {
                                renderer.edit(&self.camera, true);
                            }
                        }
                        MouseButton::Middle => renderer.pick_material(&self.camera),
                        _ => {}
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } if !self.captured => {
                self.move_ui_cursor(position);
            }
            WindowEvent::MouseWheel { delta, .. } if self.captured => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.camera.speed = (self.camera.speed * 1.15_f32.powf(lines)).clamp(0.3, 40.0);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if let Some(startup) = &self.startup {
                        if key == KeyCode::Escape && event.state == ElementState::Pressed {
                            startup.progress.cancel();
                            event_loop.exit();
                        }
                        return;
                    }
                    if event.state == ElementState::Released {
                        self.keys.remove(&key);
                        return;
                    }
                    if self.captured {
                        self.keys.insert(key);
                    }
                    if matches!(key, KeyCode::Minus | KeyCode::Equal) {
                        if let Some(renderer) = &mut self.renderer {
                            renderer.adjust_time(
                                if key == KeyCode::Minus { -1.0 } else { 1.0 }
                                    * if event.repeat { 0.08 } else { 0.25 },
                            );
                        }
                        return;
                    }
                    if event.repeat {
                        return;
                    }
                    if self.captured && key == KeyCode::Space {
                        self.simulation.queue_jump();
                    }
                    if self.handle_ui_key(key) {
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
                        KeyCode::KeyB => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.selected_material = crate::world::GLASS;
                            }
                        }
                        KeyCode::KeyN => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.selected_material = world::HANDLE;
                            }
                        }
                        KeyCode::KeyE if self.captured => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.interact_door(
                                    &self.camera,
                                    self.simulation.player.bounds(),
                                    physics::Player::bounds_at(self.simulation.render_position()),
                                );
                            }
                        }
                        KeyCode::KeyR => self.reset_player(),
                        KeyCode::KeyF if self.captured => self.simulation.toggle_flight(),
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
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_startup(event_loop);
        if self.startup.is_some() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(33),
            ));
            return;
        }
        event_loop.set_control_flow(ControlFlow::Wait);
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
    if args.iter().any(|arg| arg == "--licenses") {
        println!("{}", include_str!("../assets/FONT-LICENSE.txt"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("MiniCraft v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let scene = worldgen::preview::Scene::from_args(&args)?;
    if let worldgen::preview::Scene::Noise(preview) = scene {
        if args.iter().any(|arg| arg == "--noise-benchmark") {
            return preview.benchmark().map_err(Into::into);
        }
        if let Some(index) = args.iter().position(|arg| arg == "--noise-sample") {
            let point: [f64; 3] = args
                .get(index + 1)
                .ok_or("--noise-sample needs X,Y,Z")?
                .split(',')
                .map(str::parse::<f64>)
                .collect::<std::result::Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "--noise-sample needs exactly three coordinates")?;
            if point
                .into_iter()
                .any(|v| !v.is_finite() || v.abs() > 30_000_000.0)
            {
                return Err(
                    "Sample coordinates must be finite and within 30 million metres".into(),
                );
            }
            let generator = worldgen::Generator::new(preview.seed)?;
            let mut sampler = generator.sampler();
            println!(
                "seed {}, point {:?}, density {:.17}, material {}",
                preview.seed,
                point,
                sampler.density(point),
                sampler.material(point)
            );
            return Ok(());
        }
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
        let hours = args
            .iter()
            .position(|arg| arg == "--time")
            .map(|index| args.get(index + 1).ok_or("--time needs an hour"))
            .transpose()?
            .map(|value| value.parse::<f32>())
            .transpose()?;
        return capture::capture(
            std::path::Path::new(path),
            capture::CaptureOptions {
                scene,
                frames,
                ui: args
                    .iter()
                    .position(|arg| arg == "--ui")
                    .and_then(|index| args.get(index + 1))
                    .map(String::as_str),
                size: args
                    .iter()
                    .position(|arg| arg == "--size")
                    .map(|index| {
                        let value = args.get(index + 1).ok_or("--size needs WIDTHxHEIGHT")?;
                        let (w, h) = value.split_once('x').ok_or("--size needs WIDTHxHEIGHT")?;
                        Ok::<_, Box<dyn Error>>((w.parse::<u32>()?, h.parse::<u32>()?))
                    })
                    .transpose()?,
                verify_edits: args.iter().any(|arg| arg == "--verify-edits"),
                verify_doors: args.iter().any(|arg| arg == "--verify-doors"),
                door: args
                    .iter()
                    .position(|arg| arg == "--door")
                    .and_then(|i| args.get(i + 1))
                    .map(String::as_str),
                closeup: args.iter().any(|arg| arg == "--ao-closeup"),
                ao_enabled: !args.iter().any(|arg| arg == "--no-ao"),
                view: args
                    .iter()
                    .position(|arg| arg == "--shadow-view" || arg == "--view")
                    .and_then(|index| args.get(index + 1))
                    .map(String::as_str),
                hours,
            },
        );
    }
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(scene);
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
        .and_then(|p| p.parent().map(|p| p.join("minicraft-error.txt")))
        .unwrap_or_else(|| "minicraft-error.txt".into());
    let _ = std::fs::write(
        &path,
        format!("MiniCraft v{}\n{}\n", env!("CARGO_PKG_VERSION"), error),
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
        let title: Vec<u16> = std::ffi::OsStr::new("MiniCraft")
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
}
