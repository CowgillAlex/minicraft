use super::App;
use crate::{
    startup::{Event, Startup},
    startup_ui::LoadingScreen,
};
use std::{
    sync::{mpsc::TryRecvError, Arc},
    time::Instant,
};
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

impl App {
    pub(super) fn start_loading(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| -> crate::renderer::Result<()> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(format!(
                            "MiniCraft v{} - Loading...",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .with_inner_size(PhysicalSize::new(1440, 900))
                        .with_min_inner_size(PhysicalSize::new(800, 600)),
                )?,
            );
            let surface = Arc::new(self.instance.create_surface(window.clone())?);
            self.startup = Some(Startup::start(
                self.instance.clone(),
                surface.clone(),
                self.scene,
            )?);
            self.window = Some(window);
            self.surface = Some(surface);
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error.to_string());
            event_loop.exit();
        }
    }

    pub(super) fn poll_startup(&mut self, event_loop: &ActiveEventLoop) {
        loop {
            let Some(startup) = &self.startup else {
                return;
            };
            let event = match startup.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    Event::Failed("World loader stopped unexpectedly".into())
                }
            };
            match event {
                Event::Graphics(screen) => {
                    self.configure_loading(&screen);
                    self.loading = Some(*screen);
                }
                Event::Ready(renderer) => {
                    println!("Startup ready in {:.2}s", startup.progress.elapsed());
                    self.renderer = Some(*renderer);
                    self.loading = None;
                    self.startup = None;
                    if let Some(window) = &self.window {
                        window.set_title(&format!("MiniCraft v{}", env!("CARGO_PKG_VERSION")));
                    }
                    self.reset_player();
                    self.last_frame = Instant::now();
                    self.stats_since = self.last_frame;
                    return;
                }
                Event::Failed(error) => {
                    self.error = Some(error);
                    self.startup = None;
                    self.loading = None;
                    event_loop.exit();
                    return;
                }
            }
        }
    }

    fn configure_loading(&mut self, screen: &LoadingScreen) {
        let size = self.window.as_ref().unwrap().inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: screen.format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        self.surface
            .as_ref()
            .unwrap()
            .configure(&screen.device, &config);
        self.config = Some(config);
    }

    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        let device = self
            .renderer
            .as_ref()
            .map(|r| &r.device)
            .or_else(|| self.loading.as_ref().map(|s| &s.device));
        if let (Some(device), Some(surface), Some(config)) =
            (device, &self.surface, &mut self.config)
        {
            config.width = size.width;
            config.height = size.height;
            config.present_mode = if self.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            };
            surface.configure(device, config);
        }
    }

    pub(super) fn draw_loading(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(surface), Some(screen), Some(config), Some(startup)) = (
            &self.surface,
            &mut self.loading,
            &self.config,
            &self.startup,
        ) else {
            return;
        };
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&screen.device, config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                self.error = Some("GPU ran out of memory".into());
                event_loop.exit();
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
        };
        screen.render(
            &frame.texture.create_view(&Default::default()),
            (config.width, config.height),
            &startup.progress,
        );
        self.window.as_ref().unwrap().pre_present_notify();
        frame.present();
        if !screen.presented {
            println!("First loading frame in {:.3}s", startup.progress.elapsed());
            screen.presented = true;
        }
    }
}
