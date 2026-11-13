use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct WorkTiming {
    pub requested: Instant,
    pub started: Instant,
    pub geometry_ready: Instant,
    pub light_started: Option<Instant>,
    pub light_ready: Option<Instant>,
    pub data_ms: f32,
    pub mesh_ms: f32,
}
impl WorkTiming {
    pub fn new(started: Instant) -> Self {
        Self {
            requested: started,
            started,
            geometry_ready: started,
            light_started: None,
            light_ready: None,
            data_ms: 0.0,
            mesh_ms: 0.0,
        }
    }
    pub fn latency(self, first_ready: Instant, complete: Instant, upload_ms: f32) -> Latency {
        let ms = |end: Instant, start: Instant| {
            end.saturating_duration_since(start).as_secs_f32() * 1000.0
        };
        let light_start = self.light_started.unwrap_or(self.geometry_ready);
        let light_ready = self.light_ready.unwrap_or(self.geometry_ready);
        Latency {
            queue_ms: ms(self.started, self.requested),
            geometry_ms: ms(self.geometry_ready, self.started),
            light_queue_ms: ms(light_start, self.geometry_ready),
            light_ms: ms(light_ready, light_start),
            publish_ms: ms(complete, light_ready),
            upload_ms,
            first_ready_ms: ms(first_ready, self.requested),
            ready_ms: ms(complete, self.requested),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Latency {
    pub queue_ms: f32,
    pub geometry_ms: f32,
    pub light_queue_ms: f32,
    pub light_ms: f32,
    pub publish_ms: f32,
    pub upload_ms: f32,
    pub first_ready_ms: f32,
    pub ready_ms: f32,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub queued: usize,
    pub geometry_active: bool,
    pub light_queued: usize,
    pub light_active: bool,
    pub staging: usize,
    pub oldest_ms: f32,
    pub last: Option<Latency>,
}
#[cfg(test)]
#[test]
fn overlapping_upload_work_is_not_added_twice_to_latency() {
    use std::time::Duration;
    let t = Instant::now();
    let mut timing = WorkTiming::new(t + Duration::from_millis(20));
    timing.requested = t;
    timing.geometry_ready = t + Duration::from_millis(60);
    timing.light_started = Some(t + Duration::from_millis(70));
    timing.light_ready = Some(t + Duration::from_millis(100));
    let l = timing.latency(
        t + Duration::from_millis(110),
        t + Duration::from_millis(120),
        5.0,
    );
    for (actual, expected) in [
        (l.queue_ms, 20.0),
        (l.geometry_ms, 40.0),
        (l.light_queue_ms, 10.0),
        (l.light_ms, 30.0),
        (l.first_ready_ms, 110.0),
        (l.ready_ms, 120.0),
        (l.publish_ms, 20.0),
        (l.upload_ms, 5.0),
    ] {
        assert!((actual - expected).abs() < 0.01);
    }
}
