//! Bounded wall-clock frame history and measured fixed-step simulation work.
use std::collections::VecDeque;
const HISTORY: usize = 240;
#[derive(Default)]
pub struct History {
    pub frames: VecDeque<(f32, f32)>,
    pub ticks: VecDeque<f32>,
    pub fps: f32,
    pub tps: f32,
    frame_seconds: f64,
    frame_count: u32,
    tick_seconds: f64,
    tick_count: usize,
}
impl History {
    pub fn frame(&mut self, seconds: f64, cpu_ms: f32) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        if self.frames.len() == HISTORY {
            self.frames.pop_front();
        }
        self.frames.push_back((seconds as f32 * 1000.0, cpu_ms));
        self.frame_seconds += seconds;
        self.frame_count += 1;
        if self.frame_seconds >= 0.5 {
            self.fps = self.frame_count as f32 / self.frame_seconds as f32;
            self.frame_count = 0;
            self.frame_seconds = 0.0;
        }
    }
    pub fn simulation(&mut self, seconds: f64, tick_ms: &[f32]) {
        for &ms in tick_ms {
            if self.ticks.len() == HISTORY {
                self.ticks.pop_front();
            }
            self.ticks.push_back(ms);
        }
        self.tick_seconds += seconds.max(0.0);
        self.tick_count += tick_ms.len();
        if self.tick_seconds >= 1.0 {
            self.tps = self.tick_count as f32 / self.tick_seconds as f32;
            self.tick_seconds = 0.0;
            self.tick_count = 0;
        }
    }
    pub fn resume(&mut self) {
        self.tick_seconds = 0.0;
        self.tick_count = 0;
        self.tps = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_real_frames_and_ticks_including_stalls() {
        let mut h = History::default();
        for _ in 0..120 {
            h.frame(1.0 / 60.0, 2.0);
        }
        assert!((h.fps - 60.0).abs() < 0.01);
        for _ in 0..20 {
            h.simulation(0.05, &[0.3]);
        }
        assert!((h.tps - 20.0).abs() < 0.01);
        h.resume();
        h.simulation(1.0, &[1.0; 5]);
        assert_eq!(h.tps, 5.0);
        for _ in 0..1000 {
            h.frame(0.016, 2.0);
            h.simulation(0.05, &[0.2]);
        }
        assert_eq!(h.frames.len(), HISTORY);
        assert_eq!(h.ticks.len(), HISTORY);
    }
}
