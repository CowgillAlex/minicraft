use super::*;
use crate::{
    doors::{Door, Status},
    physics::Aabb,
};
use glam::Vec3;
fn view(door: &Door, cell: [i32; 3]) -> Camera {
    let target = door.point(
        Vec3::from_array(cell.map(|v| v as f32 + 0.5)) * crate::world::VOXEL,
        door.angle(),
    );
    let position = target + door.rotation(door.angle()) * Vec3::Z * 0.8;
    let direction = (target - position).normalize();
    Camera {
        position,
        yaw: direction.z.atan2(direction.x),
        pitch: direction.y.asin(),
        ..Default::default()
    }
}
impl Renderer {
    pub fn verify_doors(&mut self) -> Result<()> {
        self.finish_doors();
        let original = self.scene.world.doors[0].clone();
        let far = Aabb {
            min: Vec3::splat(100.0),
            max: Vec3::splat(101.0),
        };
        let camera = view(&original, [8, 10, 2]);
        if !self.interact_door(&camera, far, far) {
            return Err("Door did not open through its interaction target".into());
        }
        self.finish_doors();
        let camera = view(&self.scene.world.doors[0], [0, 10, 2]);
        if !self.edit(&camera, false) || self.scene.world.doors[0].status() != Status::FrameBroken {
            return Err("Breaking the outline did not disable the open door".into());
        }
        self.finish_doors();
        if self.interact_door(&camera, far, far) {
            return Err("Broken frame still operated".into());
        }
        self.selected_material = crate::world::WOOD;
        if !self.edit(&camera, true) || self.scene.world.doors[0].status() != Status::Ready {
            return Err("Frame repair did not restore operation".into());
        }
        self.finish_doors();
        let camera = view(&self.scene.world.doors[0], [14, 15, 3]);
        self.pick_material(&camera);
        if self.selected_material != crate::world::HANDLE {
            return Err("Handle material picking failed".into());
        }
        if !self.edit(&camera, false) || self.scene.world.doors[0].status() != Status::HandleMissing
        {
            return Err("Breaking the handle did not disable the door".into());
        }
        self.finish_doors();
        if self.interact_door(&camera, far, far) {
            return Err("Door operated without a handle".into());
        }
        self.selected_material = crate::world::HANDLE;
        if !self.edit(&camera, true) || self.scene.world.doors[0].status() != Status::Ready {
            return Err("Handle repair failed".into());
        }
        self.finish_doors();
        let camera = view(&self.scene.world.doors[0], [8, 10, 2]);
        let body = Aabb::voxel(original.world_cell([8, 10, 2]));
        if self.interact_door(&camera, body, far) {
            return Err("Door closed through the player".into());
        }
        if !self.interact_door(&camera, far, far) {
            return Err("Repaired door did not close".into());
        }
        self.finish_doors();
        self.finish_lighting();
        for z in 0..4 {
            for y in 0..32 {
                for x in 0..16 {
                    let p = [x, y, z];
                    if self.scene.world.doors[0].get(p) != original.get(p) {
                        return Err("Door edit round trip changed its design".into());
                    }
                }
            }
        }
        self.selected_material = crate::world::WOOD;
        println!("Door verification passed: open, local voxel picking, outline/handle break and repair, blocked close, mesh refresh and light updates");
        Ok(())
    }
    pub fn capture_door_state(&mut self, state: &str) -> Result<()> {
        let (open, broken) = match state {
            "closed" => (false, false),
            "open" => (true, false),
            "broken" => (true, true),
            _ => return Err("--door expects closed, open or broken".into()),
        };
        if self.scene.world.doors[0].open != open {
            let far = Aabb {
                min: Vec3::splat(100.0),
                max: Vec3::splat(101.0),
            };
            let (_, cells) = self.scene.world.toggle_door(0, far, far);
            self.lighting.request(&self.scene.world, cells);
        }
        if broken {
            let cell = self.scene.world.doors[0].world_cell([0, 10, 2]);
            if let Some(cells) = self.scene.edit(&self.device, cell, crate::world::AIR) {
                self.lighting.request(&self.scene.world, cells);
            }
        }
        self.finish_doors();
        self.finish_lighting();
        Ok(())
    }
}
