use crate::{camera::Camera, renderer::Result};
use glam::Vec3;

/// Repeatable views of the wall, terrain and leaf surfaces reported by the user.
pub fn shadow_view(name: &str) -> Result<Camera> {
    let (position, target) = match name {
        "house" => (Vec3::new(2.0, 4.3, 12.2), Vec3::new(1.75, 3.7, 7.8)),
        "terrain" => (Vec3::new(-7.6, 2.1, 10.5), Vec3::new(-7.6, 1.1, 9.2)),
        "leaves" => (Vec3::new(-9.8, 5.0, 12.7), Vec3::new(-10.5, 5.1, 10.6)),
        _ => return Err("--shadow-view must be house, terrain or leaves".into()),
    };
    let direction = (target - position).normalize();
    Ok(Camera {
        position,
        yaw: direction.z.atan2(direction.x),
        pitch: direction.y.asin(),
        ..Default::default()
    })
}
