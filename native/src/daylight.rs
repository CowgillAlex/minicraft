use glam::{Mat4, Vec3};
#[derive(Clone, Copy)]
pub struct Daylight {
    pub hours: f32,
}
impl Default for Daylight {
    fn default() -> Self {
        Self { hours: 17.5 }
    }
}
impl Daylight {
    pub fn advance(&mut self, hours: f32) {
        self.hours = (self.hours + hours).rem_euclid(24.0);
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
        let minutes = (self.hours * 60.0).round() as u32 % 1440;
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
    fn time_wraps_and_midnight_is_cool_and_dim() {
        let mut day = Daylight { hours: 23.75 };
        day.advance(0.5);
        assert!((day.hours - 0.25).abs() < 0.001);
        day.advance(-0.5);
        assert!((day.hours - 23.75).abs() < 0.001);
        let (_, night_power, color, night) = day.parameters();
        let (_, day_power, _, noon) = Daylight { hours: 12.0 }.parameters();
        assert!(night_power < day_power);
        assert!(color.z > color.x);
        assert!(night[0] < noon[0]);
    }
}
