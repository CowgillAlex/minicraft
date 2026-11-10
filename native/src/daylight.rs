use chrono::{Local, Timelike};
use glam::{Mat4, Vec3};
use std::time::{Duration, Instant};
#[derive(Clone, Copy)]
pub struct Daylight {
    pub hours: f32,
    local_time: bool,
    last_clock_check: Option<Instant>,
}
impl Default for Daylight {
    fn default() -> Self {
        Self {
            hours: 17.5,
            local_time: false,
            last_clock_check: None,
        }
    }
}
impl Daylight {
    pub fn advance(&mut self, hours: f32) {
        self.set_local_time(false);
        self.hours = (self.hours + hours).rem_euclid(24.0);
    }
    pub fn follows_local_time(&self) -> bool {
        self.local_time
    }
    pub fn set_local_time(&mut self, enabled: bool) {
        self.local_time = enabled;
        self.last_clock_check = None;
    }
    /// Sample the OS clock at most once per second, including while the game is paused.
    pub fn sync_clock(&mut self) -> bool {
        self.update_clock(Instant::now(), || {
            Local::now().num_seconds_from_midnight() as f32 / 3600.0
        })
    }
    fn update_clock(&mut self, now: Instant, read: impl FnOnce() -> f32) -> bool {
        if !self.local_time
            || self
                .last_clock_check
                .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
        {
            return false;
        }
        self.last_clock_check = Some(now);
        let hours = read().rem_euclid(24.0);
        let changed = self.hours != hours;
        self.hours = hours;
        changed
    }
    pub fn parameters(&self) -> (Vec3, f32, Vec3, [f32; 4]) {
        let angle = (self.hours - 6.0) / 12.0 * std::f32::consts::PI;
        let elevation = angle.sin();
        let solar = Vec3::new(angle.cos(), elevation, 0.3).normalize();
        let day = smooth(-0.12, 0.18, elevation);
        let twilight = smooth(-0.3, 0.0, elevation) * (1.0 - smooth(0.0, 0.35, elevation));
        let (direction, strength, color) = if elevation >= 0.0 {
            (
                solar,
                smooth(0.0, 0.12, elevation) * 1.4,
                Vec3::new(
                    1.22,
                    0.68 + 0.4 * smooth(0.0, 0.5, elevation),
                    0.36 + 0.46 * smooth(0.0, 0.5, elevation),
                ),
            )
        } else {
            (
                -solar,
                0.055 * (1.0 - smooth(-0.15, 0.0, elevation)),
                Vec3::new(0.28, 0.44, 0.8),
            )
        };
        (
            direction,
            strength,
            color,
            [day, twilight, self.hours, elevation],
        )
    }
    pub fn light_matrix(direction: Vec3) -> Mat4 {
        Mat4::orthographic_rh(-32.0, 32.0, -32.0, 32.0, 0.1, 130.0)
            * Mat4::look_at_rh(direction * 65.0, Vec3::ZERO, Vec3::Y)
    }
    pub fn label(&self) -> String {
        let minutes = ((self.hours * 3600.0).round() as u32 / 60) % 1440;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_clock_sync_is_throttled_and_manual_adjustments_disable_it() {
        let mut day = Daylight::default();
        let now = Instant::now();
        assert!(!day.update_clock(now, || panic!("manual mode must not read the clock")));
        day.set_local_time(true);
        assert!(day.update_clock(now, || 6.25));
        assert_eq!(day.label(), "06:15");
        assert!(
            !day.update_clock(now + Duration::from_millis(500), || panic!(
                "clock should be throttled"
            ))
        );
        assert!(
            !day.update_clock(now + Duration::from_secs(1), || 6.25),
            "unchanged time must preserve cached shadows"
        );
        assert!(day.update_clock(now + Duration::from_secs(2), || 19.5));
        assert_eq!(day.label(), "19:30");
        day.advance(-0.5);
        assert!(!day.follows_local_time());
        assert_eq!(day.label(), "19:00");
        assert!(!day.update_clock(now + Duration::from_secs(3), || panic!(
            "manual override must stop syncing"
        )));
    }
    #[test]
    fn turning_sync_off_keeps_time_and_clock_labels_do_not_round_up_a_minute() {
        let mut day = Daylight::default();
        let now = Instant::now();
        day.set_local_time(true);
        day.update_clock(now, || (23.0 * 3600.0 + 59.0 * 60.0 + 59.0) / 3600.0);
        assert_eq!(day.label(), "23:59");
        day.update_clock(now + Duration::from_secs(1), || 0.0);
        assert_eq!(day.label(), "00:00");
        day.set_local_time(false);
        assert!(!day.update_clock(now + Duration::from_secs(2), || 12.0));
        assert_eq!(day.label(), "00:00");
    }
    #[test]
    fn time_wraps_and_midnight_is_cool_and_dim() {
        let mut day = Daylight {
            hours: 23.75,
            ..Default::default()
        };
        day.advance(0.5);
        assert!((day.hours - 0.25).abs() < 0.001);
        day.advance(-0.5);
        assert!((day.hours - 23.75).abs() < 0.001);
        let (_, night_power, color, night) = day.parameters();
        let (_, day_power, _, noon) = Daylight {
            hours: 12.0,
            ..Default::default()
        }
        .parameters();
        assert!(night_power < day_power);
        assert!(color.z > color.x);
        assert!(night[0] < noon[0]);
    }
}
