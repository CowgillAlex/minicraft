use super::*;
use crate::{
    door_gpu::DoorGpu,
    doors::Status,
    physics::Aabb,
    picking::{self, Target},
    world::DoorAction,
};
impl Renderer {
    pub(super) fn sync_doors(&mut self) {
        self.door_meshes.truncate(self.scene.world.doors.len());
        for (id, door) in self.scene.world.doors.iter().enumerate() {
            if id == self.door_meshes.len() {
                self.door_meshes.push(DoorGpu::new(&self.device, door));
            }
            if self.door_meshes[id].update(&self.device, &self.queue, door) {
                self.shadow_ready = false;
            }
        }
    }
    pub fn finish_doors(&mut self) {
        self.sync_doors();
        for mesh in &mut self.door_meshes {
            mesh.settle();
        }
        self.sync_doors();
    }
    pub fn interact_door(&mut self, camera: &Camera, player: Aabb, rendered_player: Aabb) -> bool {
        let Some(Target::Door { id, .. }) =
            picking::cast(&self.scene.world, &self.door_meshes, camera)
        else {
            return false;
        };
        let (action, cells) = self.scene.world.toggle_door(id, player, rendered_player);
        let message = match action {
            DoorAction::Changed => {
                self.door_notice = None;
                self.shadow_ready = false;
                self.scene.stream_edits(&cells);
                self.lighting.request(&self.scene.world, cells);
                None
            }
            DoorAction::FrameBroken => Some("Repair the wooden outline"),
            DoorAction::HandleMissing => Some("Replace the handle (N)"),
            DoorAction::Blocked => Some("Door is blocked"),
        };
        if let Some(message) = message {
            self.door_notice = Some((message, Instant::now()));
        }
        action == DoorAction::Changed
    }
    pub(super) fn interaction_hint(&self) -> Option<&'static str> {
        if let Some((message, since)) = self.door_notice {
            if since.elapsed().as_secs_f32() < 2.0 {
                return Some(message);
            }
        }
        if let Some(Target::Door { id, .. }) = self.target {
            let door = &self.scene.world.doors[id];
            return Some(match door.status() {
                Status::FrameBroken => "Repair the wooden outline",
                Status::HandleMissing => "Replace the handle (N)",
                Status::Ready => {
                    if door.open {
                        "E  Close door"
                    } else {
                        "E  Open door"
                    }
                }
            });
        }
        if self.selected_material == crate::world::HANDLE {
            return Some("Handle: place halfway up a 1 x 2 m wooden outline");
        }
        if self
            .target
            .is_some_and(|t| t.material(&self.scene.world) == crate::world::HANDLE)
        {
            return Some("Needs a 1 x 2 m wooden outline, 2 voxels thick");
        }
        None
    }
}
