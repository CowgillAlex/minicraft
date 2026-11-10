use crate::{camera::Camera, renderer::Result};
use glam::Vec3;

/// Repeatable views of the wall, terrain and leaf surfaces reported by the user.
pub fn shadow_view(name: &str) -> Result<Camera> {
    let (position, target) = match name {
        "house" => (Vec3::new(2.0, 4.3, 12.2), Vec3::new(1.75, 3.7, 7.8)),
        "terrain" => (Vec3::new(-7.6, 2.1, 10.5), Vec3::new(-7.6, 1.1, 9.2)),
        "leaves" => (Vec3::new(-9.8, 5.0, 12.7), Vec3::new(-10.5, 5.1, 10.6)),
        "street" => (Vec3::new(-9.0, 3.3, 3.2), Vec3::new(-4.6, 2.6, -0.4)),
        "interior" => {
            let base = crate::world::terrain_height(348, 445) + 2;
            (
                crate::world::World::meters(358, base + 25, 450),
                crate::world::World::meters(334, base + 22, 472),
            )
        }
        "window" => {
            let base = crate::world::terrain_height(348, 445) + 2;
            (
                crate::world::World::meters(326, base + 24, 498),
                crate::world::World::meters(342, base + 23, 451),
            )
        }
        _ => {
            return Err("--view must be house, terrain, leaves, street, interior or window".into())
        }
    };
    let direction = (target - position).normalize();
    Ok(Camera {
        position,
        yaw: direction.z.atan2(direction.x),
        pitch: direction.y.asin(),
        ..Default::default()
    })
}
