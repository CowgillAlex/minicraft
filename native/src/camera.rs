use glam::{Mat4, Vec3, Vec4};
use std::collections::HashSet;
use winit::keyboard::KeyCode;

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}
impl Default for Camera {
    fn default() -> Self {
        let position = Vec3::new(-16.0, 10.0, 19.0);
        let direction = (Vec3::new(0.0, 1.8, 0.0) - position).normalize();
        Self {
            position,
            yaw: direction.z.atan2(direction.x),
            pitch: direction.y.asin(),
            speed: 5.0,
        }
    }
}
impl Camera {
    pub fn direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(65_f32.to_radians(), aspect, 0.03, 180.0)
            * Mat4::look_to_rh(self.position, self.direction(), Vec3::Y)
    }
    pub fn look(&mut self, dx: f64, dy: f64) {
        self.yaw += dx as f32 * 0.002;
        self.pitch = (self.pitch - dy as f32 * 0.002).clamp(-1.54, 1.54);
    }
    pub fn update(&mut self, keys: &HashSet<KeyCode>, dt: f32) {
        let forward = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let right = forward.cross(Vec3::Y);
        let mut movement = Vec3::ZERO;
        for (key, direction) in [
            (KeyCode::KeyW, forward),
            (KeyCode::KeyS, -forward),
            (KeyCode::KeyD, right),
            (KeyCode::KeyA, -right),
            (KeyCode::Space, Vec3::Y),
            (KeyCode::ShiftLeft, -Vec3::Y),
            (KeyCode::ShiftRight, -Vec3::Y),
            (KeyCode::KeyQ, -Vec3::Y),
            (KeyCode::KeyE, Vec3::Y),
        ] {
            if keys.contains(&key) {
                movement += direction;
            }
        }
        self.position += movement.normalize_or_zero()
            * self.speed
            * dt
            * if keys.contains(&KeyCode::ControlLeft) || keys.contains(&KeyCode::ControlRight) {
                3.0
            } else {
                1.0
            };
    }
}
pub struct Frustum([Vec4; 6]);
impl Frustum {
    pub fn new(matrix: Mat4) -> Self {
        let rows = matrix.transpose().to_cols_array_2d().map(Vec4::from_array);
        Self(
            [
                rows[3] + rows[0],
                rows[3] - rows[0],
                rows[3] + rows[1],
                rows[3] - rows[1],
                rows[2],
                rows[3] - rows[2],
            ]
            .map(|plane| plane / plane.truncate().length()),
        )
    }
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.0
            .iter()
            .all(|plane| plane.truncate().dot(center) + plane.w >= -radius)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frustum_rejects_behind_camera() {
        let f = Frustum::new(Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0));
        assert!(f.contains_sphere(Vec3::new(0.0, 0.0, -3.0), 0.2));
        assert!(!f.contains_sphere(Vec3::new(0.0, 0.0, 3.0), 0.2));
        assert!(!f.contains_sphere(Vec3::new(100.0, 0.0, -3.0), 0.2));
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    #[test]
    fn shift_descends_and_control_boosts() {
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut camera = Camera::default();
            let start = camera.position;
            camera.update(&HashSet::from([shift]), 1.0);
            assert!((camera.position.y - start.y + camera.speed).abs() < 0.001);
            assert!((camera.position.x - start.x).abs() < 0.001);
        }
        for control in [KeyCode::ControlLeft, KeyCode::ControlRight] {
            let mut camera = Camera::default();
            let start = camera.position;
            camera.update(&HashSet::from([KeyCode::KeyW, control]), 1.0);
            assert!((camera.position.distance(start) - camera.speed * 3.0).abs() < 0.001);
            assert_eq!(camera.position.y, start.y);
        }
    }
}
