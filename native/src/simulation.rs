use crate::{
    physics::{Aabb, Input, Mode, Player, EYE_HEIGHT, TICK_SECONDS},
    world::World,
};
use glam::Vec3;

/// Fixed 20 Hz simulation with one tick of buffered render interpolation.
pub struct Simulation {
    pub player: Player,
    previous: Vec3,
    accumulator: f64,
    jump_pending: bool,
    pub ticks: u64,
}
impl Default for Simulation {
    fn default() -> Self {
        Self::new(Player::spawn())
    }
}
impl Simulation {
    pub fn new(player: Player) -> Self {
        Self {
            previous: player.position,
            player,
            accumulator: 0.0,
            jump_pending: false,
            ticks: 0,
        }
    }
    pub fn queue_jump(&mut self) {
        self.jump_pending = true;
    }
    pub fn clear_input(&mut self) {
        self.jump_pending = false;
    }
    pub fn advance(&mut self, world: &World, input: Input, seconds: f64) {
        // Limit catch-up after a stall without slowing normal 10/30/60/144 Hz rendering.
        self.accumulator += seconds.clamp(0.0, 0.25);
        while self.accumulator + 1e-10 >= TICK_SECONDS {
            self.previous = self.player.position;
            let tick_input = Input {
                jump: input.jump || self.jump_pending,
                ..input
            };
            self.jump_pending = false;
            self.player.tick(world, tick_input);
            self.accumulator = (self.accumulator - TICK_SECONDS).max(0.0);
            self.ticks += 1;
        }
    }
    pub fn render_position(&self) -> Vec3 {
        self.previous.lerp(
            self.player.position,
            (self.accumulator / TICK_SECONDS).clamp(0.0, 1.0) as f32,
        )
    }
    pub fn eye_position(&self) -> Vec3 {
        self.render_position() + Vec3::Y * EYE_HEIGHT
    }
    pub fn blocks_placement(&self, cell: [i32; 3]) -> bool {
        let block = Aabb::voxel(cell);
        block.intersects(self.player.bounds())
            || block.intersects(Player::bounds_at(self.render_position()))
    }
    pub fn toggle_flight(&mut self) {
        self.player.mode = if self.player.mode == Mode::Walk {
            Mode::Fly
        } else {
            Mode::Walk
        };
        self.player.velocity = Vec3::ZERO;
        self.jump_pending = false;
    }
    pub fn reset(&mut self, world: &World) {
        *self = Self::new(Player::safe_spawn(world));
    }
}
#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
