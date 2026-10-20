//! The newer half of the campus (`districts.rs` has the map): the South Gate where the road from
//! Car Park P3 comes in, the Lunch Lawn across the boulevard from the food trucks, the Old Campus
//! with Building 1, the Founders' Terrace and the Legacy Systems Depot, the Co-Living Commons
//! along the commuter lane, Executive Hill with the board's villa, and the sports field.

use super::districts::{HILLTOP, TERRACE, lamp, signpost, spot};
use super::kit::*;
use super::props::bench;
use super::themes::{pt, runner, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub(super) fn build(p: &mut Plan) {
    south_gate(p);
    lunch_lawn(p);
    old_campus(p);
    co_living(p);
    exec_hill(p);
    sports_field(p);
}

/// Chain-link fence: posts and a frosted mesh panel per span; `gaps` along the run stay open.
pub(super) fn fence(p: &mut Plan, (x0, z0): (f32, f32), (x1, z1): (f32, f32), gaps: &[(f32, f32)]) {
    let along_x = (x1 - x0).abs() > (z1 - z0).abs();
    let (a0, a1) = if along_x { (x0, x1) } else { (z0, z1) };
    let mut a = a0;
    while a < a1 - 0.1 {
        let b = (a + 6.0).min(a1);
        let open = gaps.iter().any(|g| a < g.1 && b > g.0);
        let mid = (a + b) / 2.0;
        let (pos, size, post) = if along_x {
            (v(mid, 1.1, z0), v(b - a, 2.2, 0.08), v(a, 1.2, z0))
        } else {
            (v(x0, 1.1, mid), v(0.08, 2.2, b - a), v(x0, 1.2, a))
        };
        if !open {
            p.boxed(pos, size, Mat::Frosted, true);
        }
        p.boxed(post, v(0.12, 2.4, 0.12), Mat::Metal, false);
        a = b;
    }
}

/// The South Gate: gatehouse pillars over the boulevard with the arch sign, raised barrier arms,
/// the guard hut, and the road on to Car Park P3 (the `door:parking_p3` trigger).
fn south_gate(p: &mut Plan) {
    const Z: f32 = 150.0;
    for x in [-7.5, 7.5] {
        p.boxed(v(x, 2.6, Z), v(2.0, 5.2, 2.0), Mat::Concrete, true);
        p.boxed(v(x, 5.35, Z), v(2.3, 0.3, 2.3), Mat::Trim, false);
    }
    p.boxed(v(0.0, 6.0, Z), v(17.0, 1.2, 1.0), Mat::Trim, false);
    p.sign(v(0.0, 6.0, Z - 0.52), PI, 9.0, Sign::SouthGate);
    p.sign(v(0.0, 6.0, Z + 0.52), 0.0, 9.0, Sign::SouthGate);
    // Barrier arms, up: the badge readers have been "in beta" since they were installed.
    for (x, s) in [(-5.5, 1.0), (5.5, -1.0)] {
        p.boxed(v(x, 0.6, Z + 1.6), v(0.5, 1.2, 0.5), Mat::Hazard, true);
        p.slab(
            v(x + s * 0.3, 2.6, Z + 1.6),
            0.0,
            v(0.12, 3.6, 0.12),
            Mat::Red,
        );
        p.boxed(v(x, 1.1, Z + 1.3), v(0.3, 0.4, 0.1), Mat::LedGreen, false);
    }
    // Guard hut beside the east pillar.
    p.boxed(v(12.0, 1.3, Z - 4.0), v(3.0, 2.6, 3.0), Mat::Beige, true);
    p.boxed(v(12.0, 2.7, Z - 4.0), v(3.4, 0.2, 3.4), Mat::Trim, false);
    p.boxed(
        v(10.48, 1.5, Z - 4.0),
        v(0.04, 0.9, 2.2),
        Mat::Window,
        false,
    );
    // The road out, the sign that says how far, and a hedge break round the opening.
    signpost(p, -8.0, 160.0, true, 4.0, Sign::ToCarPark);
    signpost(p, 9.0, 126.0, true, 4.0, Sign::Boulevard);
    for x in [-12.0, 12.0] {
        p.boxed(v(x, 1.0, 165.0), v(4.0, 2.0, 1.4), Mat::LeafDark, true);
    }
    let door = v(0.0, 0.0, 160.2);
    p.anchor("door:parking_p3", door, 0.0);
    lamp(p, -10.0, 142.0);
    lamp(p, 10.0, 156.0);
}

/// The Lunch Lawn west of the boulevard: picnic tables, benches, the town-hall bandstand nobody
/// books because the town hall is a stream.
fn lunch_lawn(p: &mut Plan) {
    signpost(p, -8.0, 60.5, true, 3.6, Sign::LunchLawn);
    for (x, z) in [(-32.0, 76.0), (-14.0, 84.0), (-32.0, 64.0), (-12.0, 100.0)] {
        p.boxed(v(x, 0.75, z), v(2.0, 0.08, 0.9), Mat::Wood, true);
        for dz in [-0.8, 0.8] {
            p.boxed(v(x, 0.45, z + dz), v(2.0, 0.07, 0.35), Mat::Wood, false);
        }
    }
    for (x, z, yaw) in [(-10.0, 66.0, FRAC_PI_2), (-10.0, 90.0, FRAC_PI_2)] {
        bench(p, x, z, yaw);
    }
    // Bandstand: a round stage under a cone roof.
    let (bx, bz) = (-30.0, 86.0);
    // Low enough to step onto: the town hall is open to everyone, in theory.
    p.prim(
        PrimKind::Cylinder,
        v(bx, 0.14, bz),
        v(7.0, 0.28, 7.0),
        Mat::Wood,
    );
    p.block(v(bx, 0.14, bz), v(5.0, 0.28, 5.0));
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0;
        p.boxed(
            v(bx + 3.1 * a.cos(), 1.8, bz + 3.1 * a.sin()),
            v(0.18, 3.2, 0.18),
            Mat::White,
            true,
        );
    }
    p.prim(
        PrimKind::Cone,
        v(bx, 4.0, bz),
        v(8.0, 1.6, 8.0),
        Mat::CarRed,
    );
}

/// Old Campus: Building 1 from 1998, the Founders' Terrace with the original garage, the
/// Legacy Systems Depot on the sidings, the mainframe shed and the cooling tower.
fn old_campus(p: &mut Plan) {
    signpost(p, 7.0, -60.0, true, 3.6, Sign::OldCampus);
    // Building 1: brick, boarded windows, storage now.
    let (bx, bz) = (-60.0, -130.0);
    p.boxed(v(bx, 4.0, bz), v(26.0, 8.0, 16.0), Mat::Brick, true);
    p.boxed(v(bx, 8.15, bz), v(26.4, 0.3, 16.4), Mat::Trim, false);
    for k in 0..5 {
        let x = bx - 10.0 + k as f32 * 5.0;
        for y in [2.2, 5.6] {
            p.boxed(v(x, y, bz + 8.03), v(2.2, 1.4, 0.06), Mat::Window, false);
            p.slab(v(x, y, bz + 8.08), 0.0, v(2.4, 0.18, 0.04), Mat::Wood);
        }
    }
    p.sign(v(bx, 7.0, bz + 8.12), 0.0, 6.0, Sign::Building1);
    // The Founders' Terrace: stairs up its east side, the garage it all began in, the statue.
    let (x0, z0, x1, z1) = TERRACE;
    let zc = (z0 + z1) / 2.0;
    p.boxed(
        v((x0 + x1) / 2.0, 0.02, zc),
        v(x1 - x0, 0.04, z1 - z0),
        Mat::Concrete,
        false,
    );
    for k in 0..9 {
        let x = x1 + 0.4 + k as f32 * 0.8;
        p.boxed(v(x, 0.08, zc), v(0.8, 0.16, 6.0), Mat::Concrete, false);
    }
    for s in [-3.3, 3.3] {
        p.slab(v(x1 + 3.5, 0.5, zc + s), 0.0, v(7.2, 0.08, 0.1), Mat::Metal);
    }
    let (gx, gz) = (x0 + 8.0, zc - 2.0);
    p.boxed(v(gx, 1.5, gz - 2.4), v(6.0, 3.0, 0.2), Mat::Beige, true);
    for s in [-1.0, 1.0] {
        p.boxed(v(gx + s * 2.9, 1.5, gz), v(0.2, 3.0, 5.0), Mat::Beige, true);
    }
    p.boxed(v(gx, 3.1, gz), v(6.4, 0.2, 5.4), Mat::Trim, false);
    p.boxed(v(gx, 0.5, gz - 1.2), v(2.0, 1.0, 0.8), Mat::Wood, true);
    p.boxed(v(gx, 1.15, gz - 1.2), v(0.6, 0.3, 0.4), Mat::Dark, false);
    p.sign(v(gx, 2.6, gz + 2.55), 0.0, 4.0, Sign::FoundersTerrace);
    let (sx, sz) = (x1 - 9.0, zc + 4.0);
    p.boxed(v(sx, 0.5, sz), v(2.2, 1.0, 1.6), Mat::Concrete, true);
    for dx in [-0.45, 0.45] {
        p.boxed(v(sx + dx, 1.75, sz), v(0.45, 1.5, 0.3), Mat::Gold, false);
        p.prim(
            PrimKind::Sphere,
            v(sx + dx, 2.75, sz),
            v(0.4, 0.4, 0.4),
            Mat::Gold,
        );
    }
    p.boxed(v(sx, 1.9, sz + 0.25), v(0.8, 0.05, 0.5), Mat::Gold, false);
    signpost(p, -4.0, -97.0, false, 3.0, Sign::FoundersTerrace);
    // The Legacy Systems Depot: loading docks on the road side.
    let (dx, dz) = (40.0, -135.0);
    p.boxed(v(dx, 4.5, dz), v(40.0, 9.0, 22.0), Mat::Brick, true);
    p.boxed(v(dx, 9.15, dz), v(40.4, 0.3, 22.4), Mat::Trim, false);
    for k in 0..4 {
        let x = dx - 15.0 + k as f32 * 10.0;
        p.boxed(v(x, 2.0, dz + 11.05), v(5.0, 4.0, 0.1), Mat::Dark, false);
        for i in 0..5 {
            p.boxed(
                v(x - 2.0 + i as f32 * 1.0, 4.15, dz + 11.12),
                v(0.5, 0.3, 0.06),
                Mat::Hazard,
                false,
            );
        }
        p.boxed(v(x, 0.6, dz + 12.0), v(5.0, 1.2, 2.0), Mat::Concrete, true);
    }
    p.sign(v(dx, 6.8, dz + 11.15), 0.0, 10.0, Sign::LegacyDepot);
    // Pipes along the wall that nobody can trace to either end.
    for y in [6.0, 6.6] {
        p.prim_yawed(
            PrimKind::Cylinder,
            v(dx, y, dz + 11.4),
            v(0.35, 40.0, 0.35),
            FRAC_PI_2,
            Mat::Metal,
        );
    }
    // The mainframe shed: it hums, it blinks, it runs payroll.
    let (mx, mz) = (50.0, -80.0);
    p.boxed(v(mx, 2.0, mz), v(8.0, 4.0, 6.0), Mat::Concrete, true);
    p.boxed(v(mx, 4.1, mz), v(8.4, 0.2, 6.4), Mat::Trim, false);
    for k in 0..6 {
        let m = [Mat::BlinkGreenA, Mat::BlinkAmber, Mat::BlinkGreenB][k % 3];
        p.boxed(
            v(mx - 2.5 + k as f32 * 1.0, 2.4, mz + 3.03),
            v(0.2, 0.2, 0.05),
            m,
            false,
        );
    }
    p.sign(v(mx, 3.2, mz + 3.05), 0.0, 3.0, Sign::DoNotUnplug);
    p.boxed(
        v(mx - 2.6, 0.7, mz + 3.6),
        v(0.9, 1.4, 0.6),
        Mat::Dark,
        true,
    );
    p.boxed(
        v(mx - 2.6, 1.2, mz + 3.92),
        v(0.7, 0.45, 0.04),
        Mat::ScreenGlow,
        false,
    );
    spot(
        p,
        "prop:legacy_terminal",
        (mx - 2.6, 1.2, mz + 3.9),
        (mx - 2.6, mz + 5.4),
    );
    // Cooling tower, cooling nothing since the servers moved to the data center.
    super::models::put(
        p,
        &super::models::CHIMNEY_LARGE,
        v(-20.0, 0.0, -145.0),
        0.0,
        9.0,
        true,
    );
}

/// One co-living pod: two shipping containers stacked (one dweller per container, the
/// brochure calls it "vertical community"), a balcony rail on top and an AC unit doing its best.
fn pod(p: &mut Plan, x: f32, z: f32, face: Face, flip: bool) {
    use super::models::{SHIPPING_CONTAINER_A as A, SHIPPING_CONTAINER_B as B, put};
    let pair = if flip { [&B, &A] } else { [&A, &B] };
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        for (k, y) in [0.0, 2.56].into_iter().enumerate() {
            put(p, pair[k], v(0.0, y, 0.0), FRAC_PI_2, 7.3, k == 0);
        }
        p.boxed(v(0.0, 5.4, 1.2), v(6.0, 0.5, 0.06), Mat::Metal, false);
        p.boxed(v(-2.4, 2.6, 1.55), v(0.8, 0.6, 0.5), Mat::White, false);
    });
}

/// The Co-Living Commons: pods along the commuter lane, the Bus 9 stop, the laundromat, the
/// community fridge and a bike dock with one bike left.
fn co_living(p: &mut Plan) {
    signpost(p, 189.0, -44.0, true, 4.0, Sign::CoLiving);
    for (k, z) in [-30.0, -8.0, 14.0, 36.0, 58.0, 80.0]
        .into_iter()
        .enumerate()
    {
        pod(p, 177.0, z, Face::PosX, k % 2 == 0);
    }
    for (k, z) in [-20.0, 24.0, 46.0, 68.0].into_iter().enumerate() {
        pod(p, 215.0, z, Face::NegX, k % 2 == 1);
    }
    // Bus 9: a shelter, a timetable that is technically accurate.
    let (bx, bz) = (189.5, -2.0);
    for (dx, dz) in [(-1.6, -2.0), (-1.6, 2.0), (0.6, -2.0), (0.6, 2.0)] {
        p.boxed(
            v(bx + dx, 1.3, bz + dz),
            v(0.12, 2.6, 0.12),
            Mat::Metal,
            false,
        );
    }
    p.boxed(v(bx - 0.5, 2.66, bz), v(2.8, 0.12, 4.8), Mat::Dark, false);
    p.boxed(v(bx - 1.66, 1.3, bz), v(0.04, 2.3, 4.2), Mat::Glass, false);
    p.boxed(v(bx - 1.2, 0.5, bz), v(0.5, 0.1, 2.4), Mat::Wood, true);
    p.sign(v(bx - 1.6, 1.6, bz), FRAC_PI_2, 2.0, Sign::Bus9);
    // The laundromat.
    let (lx, lz) = (216.0, 92.0);
    p.boxed(v(lx, 2.0, lz), v(10.0, 4.0, 8.0), Mat::White, true);
    p.boxed(v(lx - 5.03, 1.6, lz), v(0.06, 2.0, 6.0), Mat::Window, false);
    p.sign(v(lx - 5.1, 3.3, lz), -FRAC_PI_2, 5.0, Sign::WeWash);
    for k in 0..4 {
        p.prim_yawed(
            PrimKind::Cylinder,
            v(lx - 5.06, 1.2, lz - 2.4 + k as f32 * 1.6),
            v(0.8, 0.05, 0.8),
            FRAC_PI_2,
            Mat::Chrome,
        );
    }
    // The community fridge: labels mandatory, enforcement passive-aggressive.
    let (fx, fz) = (184.0, 22.0);
    p.boxed(v(fx, 1.0, fz), v(1.0, 2.0, 0.9), Mat::White, true);
    p.boxed(v(fx + 0.51, 1.3, fz), v(0.04, 0.8, 0.6), Mat::Paper, false);
    spot(p, "prop:community_fridge", (fx, 1.0, fz), (fx + 1.6, fz));
    // Bike dock.
    for k in 0..6 {
        let z = 48.0 + k as f32 * 1.0;
        p.boxed(v(186.5, 0.5, z), v(0.1, 1.0, 0.1), Mat::Metal, false);
    }
    p.slab(v(187.2, 0.5, 50.0), 0.0, v(1.6, 0.05, 0.05), Mat::CarRed);
    p.prim(
        PrimKind::Cylinder,
        v(186.5, 0.08, 51.0),
        v(1.2, 0.05, 7.0),
        Mat::Concrete,
    );
    lamp(p, 192.0, 30.0);
    lamp(p, 200.0, 70.0);
}

/// Executive Hill: the gate at the top of the private road, the villa, the pool, the helipad
/// and the statue of Shareholder Value. All of it stands on the hilltop (`drape` lifts it).
fn exec_hill(p: &mut Plan) {
    let (x0, z0, x1, z1) = HILLTOP;
    signpost(p, 189.0, 109.0, false, 3.6, Sign::ExecHill);
    // The gate.
    for x in [191.5, 200.5] {
        p.boxed(v(x, 1.4, z0 + 2.0), v(0.8, 2.8, 0.8), Mat::White, true);
        p.prim(
            PrimKind::Sphere,
            v(x, 3.1, z0 + 2.0),
            v(0.7, 0.7, 0.7),
            Mat::Gold,
        );
    }
    p.slab(v(193.5, 2.4, z0 + 2.0), 0.0, v(0.12, 2.8, 0.12), Mat::Red);
    p.boxed(v(205.0, 1.3, z0 + 3.0), v(2.4, 2.6, 2.4), Mat::White, true);
    p.boxed(v(205.0, 2.7, z0 + 3.0), v(2.8, 0.2, 2.8), Mat::Trim, false);
    // Lawn edging round the top so the hill reads as kept.
    p.boxed(
        v((x0 + x1) / 2.0, 0.02, (z0 + z1) / 2.0),
        v(x1 - x0 - 2.0, 0.03, z1 - z0 - 2.0),
        Mat::Leaf,
        false,
    );
    // The villa: two white volumes, a glass band, deep roof slabs.
    let (hx, hz) = (210.0, 145.0);
    p.boxed(v(hx, 1.7, hz), v(24.0, 3.4, 14.0), Mat::White, true);
    p.boxed(
        v(hx, 1.6, hz - 7.03),
        v(20.0, 2.2, 0.06),
        Mat::Window,
        false,
    );
    p.boxed(v(hx, 3.55, hz), v(26.0, 0.3, 16.0), Mat::Trim, false);
    p.boxed(
        v(hx + 3.0, 5.2, hz - 1.0),
        v(16.0, 3.0, 10.0),
        Mat::White,
        false,
    );
    p.boxed(
        v(hx + 3.0, 5.2, hz - 6.03),
        v(14.0, 2.0, 0.06),
        Mat::Window,
        false,
    );
    p.boxed(
        v(hx + 3.0, 6.85, hz - 1.0),
        v(18.0, 0.3, 12.0),
        Mat::Trim,
        false,
    );
    p.sign(v(hx - 6.0, 2.6, hz - 7.1), PI, 5.0, Sign::Villa);
    // Infinity pool with a view of the overflow lot.
    let (px, pz) = (188.0, 148.0);
    p.boxed(v(px, 0.08, pz), v(11.0, 0.16, 6.0), Mat::Marble, false);
    p.boxed(v(px, 0.17, pz), v(10.0, 0.04, 5.0), Mat::Water, false);
    p.block(v(px, 0.4, pz), v(10.0, 0.8, 5.0));
    for k in 0..3 {
        let x = px - 3.0 + k as f32 * 3.0;
        p.slab(v(x, 0.35, pz - 4.4), 0.0, v(0.8, 0.12, 1.9), Mat::White);
    }
    // Helipad.
    let (hpx, hpz) = (190.0, 133.0);
    p.prim(
        PrimKind::Cylinder,
        v(hpx, 0.04, hpz),
        v(9.0, 0.06, 9.0),
        Mat::Concrete,
    );
    for (dx, sx, sz) in [(-1.0, 0.4, 3.2), (1.0, 0.4, 3.2), (0.0, 2.0, 0.4)] {
        p.boxed(v(hpx + dx, 0.08, hpz), v(sx, 0.02, sz), Mat::Yellow, false);
    }
    // Shareholder Value, in gold, facing the view.
    let (sx, sz) = (200.0, 156.0);
    p.boxed(v(sx, 0.6, sz), v(1.8, 1.2, 1.8), Mat::Marble, true);
    p.boxed(v(sx, 2.1, sz), v(0.6, 1.8, 0.4), Mat::Gold, false);
    p.prim(
        PrimKind::Sphere,
        v(sx, 3.25, sz),
        v(0.55, 0.55, 0.55),
        Mat::Gold,
    );
    p.slab(v(sx, 3.0, sz - 0.3), 0.6, v(0.12, 1.1, 0.12), Mat::Gold);
    // Topiary that costs more than your salary.
    for (x, z) in [
        (184.0, 128.0),
        (222.0, 128.0),
        (222.0, 158.0),
        (196.0, 140.0),
    ] {
        p.prim(PrimKind::Sphere, v(x, 0.9, z), v(1.6, 1.8, 1.6), Mat::Leaf);
        p.block(v(x, 0.9, z), v(1.2, 1.8, 1.2));
    }
}

/// The sports field south of the gym: a running track nobody runs and the Leadership Padel
/// courts, booked solid.
fn sports_field(p: &mut Plan) {
    let (cx, cz) = (88.0, 128.0);
    let ring: Vec<Vec2> = (0..=24)
        .map(|k| {
            let a = k as f32 * TAU / 24.0;
            pt(cx + 26.0 * a.cos(), cz + 14.0 * a.sin())
        })
        .collect();
    runner(p, &ring, 3.0, Mat::Red, Mat::White);
    for (k, x) in [124.0, 138.0].into_iter().enumerate() {
        let z = 150.0;
        p.boxed(v(x, 0.02, z), v(10.0, 0.04, 18.0), Mat::Felt, false);
        p.boxed(v(x, 0.5, z), v(10.0, 1.0, 0.06), Mat::White, false);
        for s in [-1.0, 1.0] {
            p.boxed(v(x, 1.5, z + s * 9.0), v(10.0, 3.0, 0.08), Mat::Glass, true);
        }
        if k == 0 {
            p.sign(v(x, 2.6, z - 9.06), PI, 5.0, Sign::Padel);
        }
    }
}
