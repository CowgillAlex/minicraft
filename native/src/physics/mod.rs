mod aabb;
mod collision;
mod player;

pub use aabb::Aabb;
pub use player::{Input, Mode, Player, EYE_HEIGHT};

pub const STEP_HEIGHT: f32 = 0.5;
pub const TICK_SECONDS: f64 = 1.0 / 20.0;

#[cfg(test)]
mod tests;
