use super::*;
use crate::world;
fn flight() -> Simulation {
    let mut player = Player::new(Vec3::new(0.0, 5.0, 0.0));
    player.mode = Mode::Fly;
    Simulation::new(player)
}
fn right() -> Input {
    Input {
        direction: Vec3::X,
        speed: 5.0,
        ..Default::default()
    }
}
#[test]
fn simulation_runs_at_twenty_tps_independent_of_render_rate() {
    let world = World::empty();
    for rate in [10, 30, 60, 144] {
        let mut sim = flight();
        for _ in 0..rate * 2 {
            sim.advance(&world, right(), 1.0 / rate as f64);
        }
        assert_eq!(sim.ticks, 40, "render rate {rate}");
        assert!((sim.player.position.x - 10.0).abs() < 0.0001);
        assert!((sim.render_position().x - 9.75).abs() < 0.0001);
    }
}
#[test]
fn render_moves_between_ticks_and_catchup_keeps_only_last_two_states() {
    let world = World::empty();
    let mut sim = flight();
    sim.advance(&world, right(), 0.075);
    assert!((sim.render_position().x - 0.125).abs() < 0.00001);
    let previous = sim.render_position();
    sim.advance(&world, right(), 0.005);
    assert!(sim.render_position().x > previous.x);
    assert_eq!(sim.ticks, 1);
    let mut sim = flight();
    sim.advance(&world, right(), 0.175);
    assert_eq!(sim.ticks, 3);
    assert!((sim.render_position().x - 0.625).abs() < 0.00001);
    // Every render frame progresses smoothly after the initial interpolation delay.
    for _ in 0..50 {
        let old = sim.render_position();
        sim.advance(&world, right(), 1.0 / 144.0);
        assert!((sim.render_position().x - old.x - 5.0 / 144.0).abs() < 0.00001);
    }
}
#[test]
fn jump_taps_survive_between_ticks_and_placement_respects_whole_body() {
    let mut world = World::empty();
    for x in 310..330 {
        for z in 310..330 {
            world.set(x, 16, z, world::STONE);
        }
    }
    let mut sim = Simulation::new(Player::new(World::meters(320, 17, 320) + Vec3::Y * 0.00002));
    sim.advance(&world, Input::default(), 0.05);
    assert!(sim.player.grounded);
    assert!(sim.blocks_placement([320, 18, 320]));
    assert!(!sim.blocks_placement([327, 18, 320]));
    sim.queue_jump();
    sim.advance(&world, Input::default(), 0.01);
    sim.advance(&world, Input::default(), 0.04);
    assert!(sim.player.velocity.y > 0.0);
    assert!(!sim.player.grounded);
}
#[test]
fn mode_changes_preserve_render_continuity_and_reset_discards_old_locations() {
    let world = World::empty();
    let mut sim = flight();
    sim.advance(&world, right(), 0.175);
    let before = sim.render_position();
    sim.toggle_flight();
    assert!(sim.render_position().abs_diff_eq(before, 0.000001));
    sim.reset(&world);
    assert!(sim
        .render_position()
        .abs_diff_eq(Player::spawn().position, 0.000001));
    assert_eq!(sim.player.mode, Mode::Walk);
}

#[test]
fn a_buffered_tap_is_not_repeated_for_each_catchup_tick() {
    let world = World::empty();
    let mut sim = flight();
    sim.queue_jump();
    sim.advance(
        &world,
        Input {
            speed: 5.0,
            ..Default::default()
        },
        0.1,
    );
    assert_eq!(sim.ticks, 2);
    assert!((sim.player.position.y - 5.25).abs() < 0.00001);
    assert_eq!(sim.player.velocity.y, 0.0);
}
