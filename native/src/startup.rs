use crate::{gpu::Gpu, renderer::Renderer, startup_ui::LoadingScreen, worldgen::preview::Scene};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Graphics,
    Terrain,
    Mesh,
    Lighting,
    Preparing,
    Ready,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Graphics => "Starting graphics",
            Self::Terrain => "Generating terrain",
            Self::Mesh => "Building meshes",
            Self::Lighting => "Baking lighting",
            Self::Preparing => "Preparing the world",
            Self::Ready => "Ready",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Snapshot {
    pub stage: Stage,
    pub completed: usize,
    pub total: usize,
}

/// A bounded shared snapshot: workers never queue individual progress events.
#[derive(Clone)]
pub struct Progress {
    state: Arc<Mutex<Snapshot>>,
    cancelled: Arc<AtomicBool>,
    started: Instant,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(Snapshot {
                stage: Stage::Graphics,
                completed: 0,
                total: 0,
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
        }
    }
}

impl Progress {
    pub fn begin(&self, stage: Stage, total: usize) {
        *self.state.lock().unwrap() = Snapshot {
            stage,
            completed: 0,
            total,
        };
        println!("Loading: {}", stage.label());
    }
    pub fn advance(&self) {
        let mut state = self.state.lock().unwrap();
        state.completed = (state.completed + 1).min(state.total);
    }
    pub fn snapshot(&self) -> Snapshot {
        *self.state.lock().unwrap()
    }
    pub fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("Loading cancelled".into())
        } else {
            Ok(())
        }
    }
}

pub enum Event {
    Graphics(Box<LoadingScreen>, Gpu),
    Ready(Box<Renderer>),
    Failed(String),
}

pub struct Startup {
    pub progress: Progress,
    pub events: mpsc::Receiver<Event>,
}

impl Startup {
    pub fn start(
        instance: Arc<wgpu::Instance>,
        surface: Arc<wgpu::Surface<'static>>,
    ) -> std::io::Result<Self> {
        Self::spawn(move |work, sender| {
            let gpu = pollster::block_on(Gpu::new(&instance, Some(&surface)))
                .map_err(|e| e.to_string())?;
            work.check()?;
            let screen = LoadingScreen::new(&gpu).map_err(|e| e.to_string())?;
            sender
                .send(Event::Graphics(Box::new(screen), gpu))
                .map_err(|_| "Window closed".to_string())
        })
    }
    pub fn generate(gpu: Gpu, source: Scene, previous: Option<Renderer>) -> std::io::Result<Self> {
        Self::spawn(move |work, sender| {
            // Release old world data and lighting workers away from the window event loop.
            drop(previous);
            let renderer = Renderer::from_gpu(gpu, source, &work).map_err(|e| e.to_string())?;
            work.check()?;
            sender
                .send(Event::Ready(Box::new(renderer)))
                .map_err(|_| "Window closed".to_string())
        })
    }
    fn spawn(
        job: impl FnOnce(Progress, mpsc::Sender<Event>) -> Result<(), String> + Send + 'static,
    ) -> std::io::Result<Self> {
        let progress = Progress::default();
        let work = progress.clone();
        let (sender, events) = mpsc::channel();
        std::thread::Builder::new()
            .name("world-startup".into())
            .spawn(move || {
                let failure = sender.clone();
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(work, sender)));
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(payload) => Some(
                        payload
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "World loading failed".into()),
                    ),
                };
                if let Some(error) = error {
                    let _ = failure.send(Event::Failed(error));
                }
            })?;
        Ok(Self { progress, events })
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        self.progress.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_counts_actual_completions_and_cancellation_survives_stage_changes() {
        let progress = Progress::default();
        let worker = progress.clone();
        worker.begin(Stage::Terrain, 3);
        for _ in 0..5 {
            worker.advance();
        }
        assert_eq!(progress.snapshot().completed, 3);
        progress.cancel();
        worker.begin(Stage::Mesh, 12);
        assert_eq!(progress.snapshot().completed, 0);
        assert_eq!(progress.snapshot().stage, Stage::Mesh);
        assert!(worker.check().is_err());
    }
    #[test]
    fn cancelled_generation_and_meshing_stop_before_doing_work() {
        let progress = Progress::default();
        progress.cancel();
        assert!(Scene::Noise(Default::default())
            .build_with_progress(&progress)
            .is_err());
        assert!(
            crate::mesh::Mesh::build_with_progress(&crate::world::World::empty(), &progress)
                .is_err()
        );
    }
}
