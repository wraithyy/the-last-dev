//! Pathfinding: chase, evade and wander around walls, the no-route fallback, determinism, cost.

mod common;

use combat_core::{Aabb, AiState, Command, Event, Vec3, line_of_sight};
use common::*;
use std::time::Instant;

fn bx(x0: f32, z0: f32, x1: f32, z1: f32) -> Aabb {
    Aabb {
        min: Vec3::new(x0, -1.0, z0),
        max: Vec3::new(x1, 3.0, z1),
    }
}

/// Vertical bar west of the CEO with a foot to the east at its south end: the CEO has to leave
/// the pocket northwards, round the end of the bar at z = 2.
fn l_wall() -> Vec<Aabb> {
    vec![bx(-3.5, -12.0, -2.5, 2.0), bx(-3.5, -12.5, 6.0, -11.5)]
}

fn inside(blockers: &[Aabb], p: Vec3) -> bool {
    blockers
        .iter()
        .any(|b| p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z)
}

fn move_player(h: &mut H, pos: Vec3) {
    h.sim.push(Command::PlayerMoved {
        pos,
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

/// CEO (spawn (0,-6)) aggroes while the player stands in the open, then the walls go up and
/// the player steps behind them.
fn l_chase(seed: u64, ticks: u32) -> (H, Vec<Vec3>) {
    let mut h = H::new(seed, -6.0, |_| {});
    h.run(2);
    h.sim.set_blockers(l_wall());
    move_player(&mut h, Vec3::new(-6.0, 0.0, -6.0));
    let mut trace = Vec::new();
    for _ in 0..ticks {
        h.tick();
        trace.push(h.unit(h.mob).pos);
    }
    (h, trace)
}

#[test]
fn chaser_rounds_an_l_wall_and_reaches_melee() {
    let (h, trace) = l_chase(1, 30 * 12);
    let walls = l_wall();
    assert!(
        trace.iter().all(|p| !inside(&walls, *p)),
        "walked through a wall"
    );
    let player = h.unit(h.player).pos;
    let closest = trace
        .iter()
        .map(|p| dist(*p, player))
        .fold(f32::MAX, f32::min);
    assert!(
        closest <= 2.5,
        "never got into melee range (closest {closest} m)"
    );
    // It really went round the north end rather than slipping through.
    assert!(trace.iter().any(|p| p.z > 1.5));
}

#[test]
fn evading_unit_walks_home_around_the_wall() {
    let mut h = H::new(1, -6.0, |c| {
        if let Some(ai) = &mut def_mut(c, "ceo").ai {
            ai.leash_m = 12.0;
        }
    });
    h.sim.set_wandering(false);
    h.run(2);
    h.sim.set_blockers(l_wall());
    move_player(&mut h, Vec3::new(-30.0, 0.0, -6.0));
    let spawn = Vec3::new(0.0, 0.0, -6.0);
    let mut evaded = false;
    let mut far_west = false;
    for _ in 0..30 * 40 {
        h.tick();
        let u = h.unit(h.mob);
        evaded |= u.ai == Some(AiState::Evade);
        far_west |= u.pos.x < -4.0;
        assert!(!inside(&l_wall(), u.pos), "evade walked through a wall");
    }
    assert!(far_west && evaded, "setup never left the pocket / evaded");
    let u = h.unit(h.mob);
    assert_eq!(u.ai, Some(AiState::Idle));
    assert!(dist(u.pos, spawn) < 0.01, "ended at {:?}", u.pos);
}

#[test]
fn wander_goals_in_a_sealed_pen_stay_inside() {
    // 3 m pen: every wander spot outside it is walled off, so the unit must keep still.
    let pen = vec![
        bx(-2.0, -2.0, 2.0, -1.5),
        bx(-2.0, 1.5, 2.0, 2.0),
        bx(-2.0, -1.5, -1.5, 1.5),
        bx(1.5, -1.5, 2.0, 1.5),
    ];
    let mut h = H::new(3, -40.0, |_| {});
    h.sim.set_blockers(pen);
    let id = match h.sim.spawn_unit("meatproxy", Vec3::new(0.0, 0.0, 0.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    for _ in 0..30 * 120 {
        h.tick();
        let p = h.unit(id).pos;
        assert!(p.x.abs() < 1.5 && p.z.abs() < 1.5, "left the pen: {p:?}");
    }
}

#[test]
fn wanderers_beside_an_obstacle_still_stroll_without_clipping() {
    let walls = vec![bx(3.0, -3.0, 4.0, 3.0)];
    let mut h = H::new(5, -40.0, |_| {});
    h.sim.set_blockers(walls.clone());
    let id = match h.sim.spawn_unit("meatproxy", Vec3::new(0.0, 0.0, 0.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let mut far = 0.0_f32;
    for _ in 0..30 * 300 {
        h.tick();
        let p = h.unit(id).pos;
        assert!(!inside(&walls, p), "clipped into a wall at {p:?}");
        far = far.max(dist(p, Vec3::new(0.0, 0.0, 0.0)));
    }
    assert!(far > 1.0, "never wandered");
}

#[test]
fn unreachable_target_falls_back_to_evade_after_a_timeout() {
    let ring = vec![
        bx(-2.5, -2.5, 2.5, -2.0),
        bx(-2.5, 2.0, 2.5, 2.5),
        bx(-2.5, -2.0, -2.0, 2.0),
        bx(2.0, -2.0, 2.5, 2.0),
    ];
    let mut h = H::new(1, -6.0, |_| {});
    h.sim.set_wandering(false);
    h.run(2);
    h.sim.set_blockers(ring.clone());
    let spawn = Vec3::new(0.0, 0.0, -6.0);
    let mut evade_tick = None;
    for t in 0..30 * 12 {
        h.tick();
        let u = h.unit(h.mob);
        assert!(!inside(&ring, u.pos));
        if evade_tick.is_none() && u.ai == Some(AiState::Evade) {
            evade_tick = Some(t);
        }
    }
    let t = evade_tick.unwrap_or_else(|| panic!("never evaded"));
    assert!((140..200).contains(&t), "gave up at tick {t}");
    let u = h.unit(h.mob);
    assert_eq!(u.ai, Some(AiState::Idle));
    assert!(dist(u.pos, spawn) < 0.01);
    assert_eq!(h.count(|e| matches!(e, Event::Despawned { .. })), 0);
}

#[test]
fn find_path_is_clear_ends_at_goal_and_none_when_sealed() {
    let h = H::new(1, -6.0, |_| {});
    let mut sim = h.sim;
    let walls = l_wall();
    sim.set_blockers(walls.clone());
    let (from, to) = (Vec3::new(0.0, 0.0, -6.0), Vec3::new(-6.0, 0.0, -6.0));
    let path = sim.find_path(from, to).unwrap_or_default();
    assert!(path.len() >= 2, "expected a detour, got {path:?}");
    assert_eq!(path.last().copied(), Some(to));
    let mut prev = from;
    for p in &path {
        assert!(
            line_of_sight(prev, *p, &walls),
            "segment {prev:?}->{p:?} hits a wall"
        );
        prev = *p;
    }
    assert_eq!(
        sim.find_path(from, Vec3::new(5.0, 0.0, -6.0)),
        Some(vec![Vec3::new(5.0, 0.0, -6.0)])
    );
    let mut sealed = walls;
    sealed.extend([
        bx(-9.0, 5.0, -4.0, 6.0),
        bx(-9.0, -9.0, -8.0, 6.0),
        bx(-9.0, -9.0, -4.0, -8.0),
        bx(-5.0, -9.0, -4.0, 6.0),
    ]);
    sim.set_blockers(sealed);
    assert!(sim.find_path(from, Vec3::new(-6.5, 0.0, -1.0)).is_none());
}

#[test]
fn same_inputs_give_identical_paths() {
    let (_, a) = l_chase(7, 30 * 8);
    let (_, b) = l_chase(7, 30 * 8);
    assert_eq!(a, b);
}

/// Pseudo-random maze over a floor-sized area, 40 chasers: the nav cost per tick must stay far
/// under the 33 ms budget (target well under 1 ms).
#[test]
fn chasing_a_full_floor_stays_cheap() {
    let mut h = H::new(1, -6.0, |_| {});
    let mut s = 0x2545_F491_4F6C_DD1D_u64;
    let mut rnd = |n: u64| {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 33) % n
    };
    let mut walls = vec![
        bx(-60.0, -45.0, 60.0, -44.0),
        bx(-60.0, 44.0, 60.0, 45.0),
        bx(-61.0, -45.0, -60.0, 45.0),
        bx(60.0, -45.0, 61.0, 45.0),
    ];
    for _ in 0..250 {
        let (x, z) = (rnd(110) as f32 - 55.0, rnd(80) as f32 - 40.0);
        let (w, d) = (1.0 + rnd(6) as f32, 1.0 + rnd(6) as f32);
        // Keep the start cluster open.
        if x.abs() > 10.0 || z.abs() > 12.0 {
            walls.push(bx(x, z, x + w, z + d));
        }
    }
    for k in 0..40 {
        let (x, z) = ((k % 8) as f32 - 4.0, (k / 8) as f32 * 2.0 - 5.0);
        if h.sim.spawn_unit("ceo", Vec3::new(x, 0.0, z), 0.0).is_err() {
            panic!("spawn failed");
        }
    }
    h.run(2);
    h.sim.set_blockers(walls);
    move_player(&mut h, Vec3::new(50.0, 0.0, 30.0));
    let ticks = 30 * 20;
    let t0 = Instant::now();
    h.run(ticks);
    let per_tick = t0.elapsed().as_secs_f64() * 1000.0 / f64::from(ticks);
    println!("nav full-floor cost: {per_tick:.3} ms/tick for 41 units");
    assert!(per_tick < 1.0, "{per_tick} ms per tick");
}
