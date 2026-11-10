use super::{collision, Aabb, TICK_SECONDS};
use crate::world::{self, World, VOXEL};
use glam::Vec3;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

pub const EYE_HEIGHT: f32 = 1.62;
const HEIGHT: f32 = 1.8;
const HALF_WIDTH: f32 = 0.3;
const GRAVITY: f32 = 24.0;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Walk,
    Fly,
}
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub direction: Vec3,
    pub jump: bool,
    pub descend: bool,
    pub sprint: bool,
    pub speed: f32,
}
impl Input {
    pub fn from_keys(keys: &HashSet<KeyCode>, yaw: f32, speed: f32) -> Self {
        let forward = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        let right = forward.cross(Vec3::Y);
        let mut direction = Vec3::ZERO;
        for (key, axis) in [
            (KeyCode::KeyW, forward),
            (KeyCode::KeyS, -forward),
            (KeyCode::KeyD, right),
            (KeyCode::KeyA, -right),
        ] {
            if keys.contains(&key) {
                direction += axis;
            }
        }
        Self {
            direction: direction.normalize_or_zero(),
            jump: keys.contains(&KeyCode::Space) || keys.contains(&KeyCode::KeyE),
            descend: keys.contains(&KeyCode::ShiftLeft)
                || keys.contains(&KeyCode::ShiftRight)
                || keys.contains(&KeyCode::KeyQ),
            sprint: keys.contains(&KeyCode::ControlLeft) || keys.contains(&KeyCode::ControlRight),
            speed,
        }
    }
}
pub struct Player {
    /// Position is the centre of the feet, never an interpolated render pose.
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub mode: Mode,
}
impl Player {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            grounded: false,
            mode: Mode::Walk,
        }
    }
    pub fn spawn() -> Self {
        Self::new(World::meters(100, 26, 347) + Vec3::Y * 0.00002)
    }
    pub fn safe_spawn(world: &World) -> Self {
        let mut player = Self::spawn();
        for _ in 0..=world::HEIGHT {
            if Self::can_occupy(world, player.position) {
                return player;
            }
            player.position.y += VOXEL;
        }
        player
    }
    pub fn bounds_at(position: Vec3) -> Aabb {
        Aabb {
            min: position - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH),
            max: position + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH),
        }
    }
    pub fn bounds(&self) -> Aabb {
        Self::bounds_at(self.position)
    }
    pub fn can_occupy(world: &World, position: Vec3) -> bool {
        !collision::overlaps_solid(world, Self::bounds_at(position))
    }
    pub fn tick(&mut self, world: &World, input: Input) {
        let dt = TICK_SECONDS as f32;
        let boost = if input.sprint {
            if self.mode == Mode::Fly {
                3.0
            } else {
                1.5
            }
        } else {
            1.0
        };
        let speed = input.speed.clamp(0.3, 40.0) * boost;
        let mut wanted_velocity = input.direction * speed;
        if self.mode == Mode::Fly {
            wanted_velocity.y =
                (u8::from(input.jump) as f32 - u8::from(input.descend) as f32) * speed;
            wanted_velocity = wanted_velocity.clamp_length_max(speed);
            self.velocity = wanted_velocity;
        } else {
            if input.descend {
                wanted_velocity *= 0.35;
            }
            let horizontal = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
            let acceleration = if self.grounded { 50.0 } else { 12.0 };
            let change = (wanted_velocity - horizontal).clamp_length_max(acceleration * dt);
            self.velocity.x += change.x;
            self.velocity.z += change.z;
            if input.jump && self.grounded {
                self.velocity.y = 8.4;
            }
            self.velocity.y = (self.velocity.y - GRAVITY * dt).max(-48.0);
        }
        let wanted = self.velocity * dt;
        let motion = collision::move_body(
            world,
            self.bounds(),
            wanted,
            self.grounded && self.mode == Mode::Walk,
        );
        self.position += motion.delta;
        for axis in 0..3 {
            if (motion.delta[axis] - wanted[axis]).abs() > 0.000001 {
                self.velocity[axis] = 0.0;
            }
        }
        self.grounded = motion.grounded;
    }
}
