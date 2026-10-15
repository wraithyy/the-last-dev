//! Raid presentation ("All-Hands Offsite"). Entering a raid room names the difficulty in the
//! zone banner slot (once per raid visit), a locked encounter becomes a notice, and "Pair Up"
//! tethers are drawn as a cord between partners: green while together, red once stretched past
//! the range. No permanent HUD element: everything else reuses the boss frame, auras and
//! telegraphs. Rules live in `combat_core::raid`.
//!
//! QA: `WOW_RAID_ASYNC=0|1` forces the difficulty of offline raid loads; `WOW_RAID_LOCKS=a,b`
//! sets this week's raid lockout (keys as in `RaidView::locked`); `WOW_LADDER_DEMO=1` puts three
//! sample runs on this week's Weekly Sprint ladder.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit, SimView, UiNotice};
use crate::hud::Banner;
use bevy::prelude::*;
use combat_core::{Event, RaidView};

/// Lilac of the raid banners (the offsite's lanyard colour).
pub const RAID: Color = Color::srgb(0.72, 0.55, 1.0);
/// At most this many pairs: five players make two pairs and one with the boss.
const CORDS: usize = 3;
const CORD_Y: f32 = 1.1;

#[derive(Component)]
struct Cord(usize);

#[derive(Resource)]
struct CordMats {
    ok: Handle<StandardMaterial>,
    stretched: Handle<StandardMaterial>,
}

pub struct RaidPlugin;

impl Plugin for RaidPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_cords)
            .add_systems(PostStartup, qa_async.after(crate::game::start_game))
            .add_systems(Update, (qa_locks, events, cords).in_set(SimReady));
    }
}

fn qa_async(mut sim: ResMut<CombatSim>) {
    match std::env::var("WOW_RAID_ASYNC").as_deref() {
        Ok("1") => sim.0.debug_force_raid_async(Some(true)),
        Ok("0") => sim.0.debug_force_raid_async(Some(false)),
        _ => {}
    }
}

/// The lockout belongs to an ISO week, so it waits for the office clock.
fn qa_locks(mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if *done || !sim.0.clock_on() {
        return;
    }
    *done = true;
    if let Ok(v) = std::env::var("WOW_RAID_LOCKS") {
        let keys: Vec<String> = v.split(',').map(|b| b.trim().to_string()).collect();
        sim.0.debug_set_raid_lockout(&keys);
    }
    if std::env::var_os("WOW_LADDER_DEMO").is_some() {
        for (ms, deaths, crew) in [
            (
                731_000,
                0,
                &[
                    "Ada (Backend Engineer)",
                    "Linus (Fullstack Engineer)",
                    "Grace (DevOps Engineer)",
                ][..],
            ),
            (812_500, 2, &["Fullstack Engineer"][..]),
            (
                1_004_000,
                5,
                &["Margaret (Frontend Developer)", "Dennis (DevOps Engineer)"][..],
            ),
        ] {
            sim.0.debug_ladder_add(ms, deaths, crew);
        }
    }
}

/// Banner title and subtitle for entering the raid on a difficulty.
pub fn difficulty_banner(raid_name: &str, async_mode: bool) -> (String, String) {
    if async_mode {
        (
            format!("{}: ASYNC ATTENDANCE", raid_name.to_uppercase()),
            "Fewer than three of you: the recorded version. Softer bosses, fewer gifts, same lanyard."
                .into(),
        )
    } else {
        (
            format!("{}: FULL PROGRAMME", raid_name.to_uppercase()),
            "Attendance is mandatory. Each session can be attended once a week.".into(),
        )
    }
}

/// The notice for walking into a room whose boss this group already beat this week.
pub fn locked_notice(view: &RaidView, boss_name: &str, boss_key: &str) -> Option<String> {
    let key = combat_core::raid::lock_key(boss_key, view.async_mode);
    view.locked_in(view.week).contains(&key).then(|| {
        format!("{boss_name} already presented this week. The room is being 'reset' until Monday.")
    })
}

fn events(
    sim: SimView,
    mut last: Local<(String, bool)>,
    mut reader: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut notices: MessageWriter<UiNotice>,
) {
    for CombatEvent(e) in reader.read() {
        let Event::Raid { view } = e else { continue };
        let content = sim.content();
        let entered = !view.zone.is_empty() && last.0 != view.zone;
        // One difficulty banner per visit, not per room.
        if entered && (last.0.is_empty() || last.1 != view.async_mode) {
            let (title, subtitle) = difficulty_banner(&content.raid.name, view.async_mode);
            banners.write(Banner {
                title,
                subtitle,
                color: RAID,
            });
        }
        if entered && let Some(b) = content.zone(&view.zone).and_then(|z| z.boss.as_ref()) {
            let name = content
                .unit_def(&b.unit)
                .map_or("The boss", |u| u.name.as_str());
            if let Some(n) = locked_notice(view, name, &b.unit) {
                notices.write(UiNotice(n));
            }
        }
        *last = (view.zone.clone(), view.async_mode);
    }
}

fn spawn_cords(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let glow = |c: Color| StandardMaterial {
        base_color: c,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let ok = mats.add(glow(Color::srgba(0.45, 1.0, 0.55, 0.75)));
    let stretched = mats.add(glow(Color::srgba(1.0, 0.25, 0.2, 0.9)));
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for k in 0..CORDS {
        commands.spawn((
            Cord(k),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(ok.clone()),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
        ));
    }
    commands.insert_resource(CordMats { ok, stretched });
}

/// Places a thin box between each tethered pair, coloured by whether it is stretched.
fn cords(
    sim: SimView,
    mats: Res<CordMats>,
    units: Query<(&SimUnit, &GlobalTransform)>,
    mut q: Query<(
        &Cord,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let view = sim.raid_view();
    let pos = |id: combat_core::EntityId| {
        units
            .iter()
            .find(|(u, _)| u.0 == id)
            .map(|(_, t)| t.translation())
    };
    let range = f32::from(view.tether_m.max(1));
    for (cord, mut tf, mut vis, mut mat) in &mut q {
        let ends = view
            .tethers
            .get(cord.0)
            .and_then(|&(a, b)| Some((pos(a)?, pos(b)?)));
        let Some((a, b)) = ends else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (a, b) = (a.with_y(a.y + CORD_Y), b.with_y(b.y + CORD_Y));
        let d = a.distance(b);
        let stretched = Vec2::new(a.x - b.x, a.z - b.z).length() > range;
        let want = if stretched { &mats.stretched } else { &mats.ok };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
        let thick = if stretched { 0.09 } else { 0.05 };
        *tf = Transform::from_translation((a + b) * 0.5)
            .looking_at(b, Vec3::Y)
            .with_scale(Vec3::new(thick, thick, d.max(0.01)));
        *vis = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_names_the_difficulty_and_locked_rooms_say_so() {
        let (t, s) = difficulty_banner("All-Hands Offsite", true);
        assert!(t.contains("ASYNC ATTENDANCE") && s.contains("three"));
        let (t, _) = difficulty_banner("All-Hands Offsite", false);
        assert!(t.contains("FULL PROGRAMME"));
        let view = RaidView {
            zone: "offsite/1".into(),
            async_mode: true,
            week: 7,
            locked: vec!["async:icebreaker_facilitator".into()],
            ..Default::default()
        };
        assert!(locked_notice(&view, "Icebreaker", "icebreaker_facilitator").is_some());
        let normal = RaidView {
            async_mode: false,
            ..view
        };
        assert!(locked_notice(&normal, "Icebreaker", "icebreaker_facilitator").is_none());
    }
}
