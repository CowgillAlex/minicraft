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
    before_spectator: Mode,
    spawn_pending: bool,
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
            before_spectator: Mode::Walk,
            spawn_pending: false,
        }
    }
    pub fn queue_jump(&mut self) {
        self.jump_pending = true;
    }
    pub fn clear_input(&mut self) {
        self.jump_pending = false;
    }
    pub fn advance(&mut self, world: &World, input: Input, seconds: f64) {
        if self.spawn_pending {
            if !Player::body_ready(world, self.player.position) {
                return;
            }
            self.reset(world);
            if self.spawn_pending {
                return;
            }
        }
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
        if self.player.mode == Mode::Spectator {
            return false;
        }
        let block = Aabb::voxel(cell);
        block.intersects(self.player.bounds())
            || block.intersects(Player::bounds_at(self.render_position()))
    }
    pub fn toggle_flight(&mut self) {
        if self.player.mode == Mode::Spectator {
            return;
        }
        self.player.mode = if self.player.mode == Mode::Walk {
            Mode::Fly
        } else {
            Mode::Walk
        };
        self.player.velocity = Vec3::ZERO;
        self.jump_pending = false;
    }
    /// Preserve the visible pose; leaving spectator requires clear space for the body.
    pub fn toggle_spectator(&mut self, world: &World) -> bool {
        let position = self.render_position();
        if self.player.mode == Mode::Spectator {
            if !Player::can_occupy(world, position) {
                return false;
            }
            self.player.mode = self.before_spectator;
        } else {
            self.before_spectator = self.player.mode;
            self.player.mode = Mode::Spectator;
        }
        self.spawn_pending = false;
        self.player.position = position;
        self.previous = position;
        self.accumulator = 0.0;
        self.player.velocity = Vec3::ZERO;
        self.player.grounded = false;
        self.jump_pending = false;
        true
    }
    pub fn reset(&mut self, world: &World) {
        *self = Self::new(Player::safe_spawn(world));
        self.spawn_pending = !Player::body_ready(world, self.player.position);
    }
}
#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
