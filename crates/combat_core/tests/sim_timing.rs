//! Manual tick-cost probe: `cargo test -p combat_core --release --test sim_timing -- --ignored --nocapture`.
mod common;

use combat_core::{Aabb, Sim, Vec3};
use common::content;

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    // Spread every anchor over the campus so NPCs and packs wander and aggro like in the client.
    let h = name
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let (x, z) = ((h % 300) as f32 - 150.0, ((h / 300) % 300) as f32 - 150.0);
    Some((Vec3::new(x, 0.0, z), 0.0))
}

#[test]
#[ignore]
fn campus_tick_cost() {
    let mut seed = 99_u32;
    let mut rnd = |lo: f32, hi: f32| {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        lo + (seed >> 8) as f32 / (1u32 << 24) as f32 * (hi - lo)
    };
    // Roughly the campus scenery density: a few hundred building and prop boxes.
    let blockers: Vec<Aabb> = (0..600)
        .map(|_| {
            let (x, z) = (rnd(-200.0, 200.0), rnd(-200.0, 200.0));
            Aabb {
                min: Vec3::new(x, 0.0, z),
                max: Vec3::new(x + rnd(1.0, 10.0), rnd(1.0, 8.0), z + rnd(1.0, 10.0)),
            }
        })
        .collect();
    let mut sim = Sim::new(content(), 5);
    sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.load_zone("campus", &anchors, blockers)
        .expect("campus loads");
    for _ in 0..300 {
        sim.tick();
    }
    let n = 3000;
    let t = std::time::Instant::now();
    let mut events = 0usize;
    for _ in 0..n {
        events += sim.tick().len();
    }
    println!(
        "TICK_US {:.1} (events {events})",
        t.elapsed().as_secs_f64() * 1e6 / n as f64
    );
}
