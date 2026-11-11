use crate::{camera::Camera, renderer::Result};
use glam::Vec3;

/// Repeatable views of the wall, terrain and leaf surfaces reported by the user.
pub fn shadow_view(name: &str) -> Result<Camera> {
    if matches!(name, "world-surface" | "world-underwater") {
        return Ok(Camera::default());
    }
    let (position, target) = match name {
        n if n.starts_with("feature-ore-")=>{let i:usize=n[12..].parse()?;if i>=8{return Err("ore view index must be 0..7".into());}let x=(i%4)as f32*1.8-2.7;let z=(i/4)as f32*2.-0.8;(Vec3::new(x+1.9,2.8,z+3.0),Vec3::new(x+0.65,1.8,z+0.65))},
        "feature-cave"=>(Vec3::new(-3.,2.7,12.),Vec3::new(0.,2.3,-1.)),
        "door" => {
            let base=crate::world::terrain_height(155,205)+2;
            (crate::world::World::meters(155,base+20,270),crate::world::World::meters(155,base+18,233))
        }
        "door-side" => {
            let base=crate::world::terrain_height(155,205)+2;
            (crate::world::World::meters(180,base+20,223),crate::world::World::meters(148,base+18,223))
        }
        "feature-overview" => (Vec3::new(-10.0,7.0,13.0),Vec3::new(0.0,3.0,0.0)),
        "feature-flowers"=>(Vec3::new(-0.5,2.8,2.7),Vec3::new(-0.2,1.75,-1.2)),
        "feature-wheat"=>(Vec3::new(1.5,2.5,1.7),Vec3::new(0.4,2.1,0.0)),
        "feature-grass"=>(Vec3::new(-0.8,1.7,1.0),Vec3::new(-1.4,1.4,0.0)),
        "feature-fern"=>(Vec3::new(3.1,1.9,1.4),Vec3::new(1.9,1.4,-0.8)),
        "feature-close" => (Vec3::new(2.9,2.8,4.4),Vec3::new(0.2,1.7,-0.2)),
        "feature-tree" => (Vec3::new(-7.0,4.3,8.0),Vec3::new(0.0,5.0,0.0)),
        "feature-water" => (Vec3::new(-5.5,3.7,8.0),Vec3::new(0.0,2.3,-1.0)),
        "feature-cabin" => (Vec3::new(-5.5,3.2,7.5),Vec3::new(0.2,2.5,0.0)),
        "cave" => (Vec3::new(-14.0, 16.7, 17.0), Vec3::new(-4.0, 14.0, 10.0)),
        "house" => (Vec3::new(2.0, 4.3, 12.2), Vec3::new(1.75, 3.7, 7.8)),
        "terrain" => (Vec3::new(-7.6, 2.1, 10.5), Vec3::new(-7.6, 1.1, 9.2)),
        "leaves" => (Vec3::new(-9.8, 5.0, 12.7), Vec3::new(-10.5, 5.1, 10.6)),
        "water" => (Vec3::new(6.0, 2.8, -6.0), Vec3::new(8.0, 1.25, -9.7)),
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
            return Err(
                "--view must be cave, door, door-side, house, terrain, leaves, water, street, interior or window".into(),
            )
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

/// Position a capture relative to actual source terrain, so a diagnostic crop
/// cannot put its camera inside a mountain or beneath the intended surface.
pub fn procedural(preview: crate::worldgen::preview::Preview, underwater: bool) -> Result<Camera> {
    let generator = crate::worldgen::Generator::new(preview.seed)?;
    let mut sampler = generator.sampler();
    let x = preview.origin[0] as f64 + 20.;
    let z = preview.origin[2] as f64 + 20.;
    let target = sampler.feature_ground(x, z);
    let near = sampler.feature_ground(
        x - if underwater { 4. } else { 12. },
        z + if underwater { 6. } else { 13. },
    );
    let eye = glam::DVec3::new(
        near.x,
        if underwater {
            (near.y + 1.5).min(62.5)
        } else {
            near.y.max(target.y) + 14.
        },
        near.z,
    );
    let target = target + glam::DVec3::Y * if underwater { 0.8 } else { 4. };
    let offset = glam::DVec3::new(
        preview.origin[0] as f64 + 20.,
        preview.origin[1] as f64,
        preview.origin[2] as f64 + 20.,
    );
    let position = (eye - offset).as_vec3();
    let direction = (target - eye).normalize().as_vec3();
    Ok(Camera {
        position,
        yaw: direction.z.atan2(direction.x),
        pitch: direction.y.asin(),
        ..Default::default()
    })
}
