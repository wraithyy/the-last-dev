//! Quest waypoint arrow (opt-in, settings > quest arrow or its key): a small 3D arrow floats above
//! the player and points at the tracked quest's work, with the distance beside it. The target is the
//! nearest living unit (or prop) of an open objective, the turn-in giver once the work is done, or,
//! when that is in another zone, the door that starts the shortest open route there.
//! Which quest: the one picked in the log or by a tracker title click, else the first tracked.

use super::quests::{QuestTrack, objective_unit_keys, tracked};
use super::{HudHidden, label, put};
use crate::combat::{SimRead, SimReady, SimView};
use crate::game::GameFlow;
use crate::input::Paused;
use crate::keybinds::{Action, Keybinds};
use crate::player::Player;
use crate::settings::Settings;
use bevy::prelude::*;
use combat_core::{Content, Objective, QuestDef};
use std::f32::consts::FRAC_PI_2;

/// The arrow hides this close to its target: you are there.
const ARRIVED_M: f32 = 4.0;
/// Height of the arrow above the player's feet.
const HOVER_M: f32 = 2.7;
/// Exponential rate (1/s) at which the arrow turns toward a new bearing.
const TURN_RATE: f32 = 10.0;
/// How often the target is searched again; units move, but not that fast.
const REFRESH_SECS: f32 = 0.3;

/// Where the arrow points right now.
#[derive(Resource, Default)]
struct Waypoint {
    target: Option<Vec3>,
    next_search: f32,
}

#[derive(Component)]
struct ArrowMesh;
#[derive(Component)]
struct ArrowLabel;

pub struct WaypointPlugin;

impl Plugin for WaypointPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Waypoint>()
            .add_systems(Startup, spawn_arrow)
            .add_systems(
                Update,
                (toggle_arrow, find_target, place_arrow)
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_arrow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // One cone lying down so its tip is the arrow's -Z (forward) end; the root is yawed.
    let mesh = meshes.add(Cone::new(0.28, 0.8));
    let mat = materials.add(StandardMaterial {
        base_color: super::GOLD,
        emissive: super::GOLD.to_linear() * 2.0,
        unlit: true,
        ..default()
    });
    commands
        .spawn((ArrowMesh, Transform::default(), Visibility::Hidden))
        .with_child((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ));
    commands.spawn((
        ArrowLabel,
        label("", 13.0),
        TextColor(super::GOLD),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(4),
    ));
}

fn toggle_arrow(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    focus: Res<crate::input::TextFocus>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 && !focus.0 && binds.just(&keys, Action::QuestArrow) {
        settings.quest_arrow = !settings.quest_arrow;
    }
}

/// The quest the arrow follows: the picked one while it is still open, else the first tracked.
fn followed<'a>(sim: &'a dyn SimRead, track: &QuestTrack) -> Option<&'a QuestDef> {
    let content = sim.content();
    let open = |id: &str| sim.quests().active.iter().any(|q| q.id == id);
    track
        .waypoint
        .as_deref()
        .filter(|id| open(id))
        .and_then(|id| content.quest(id))
        .or_else(|| {
            tracked(sim, track)
                .first()
                .and_then(|q| content.quest(&q.id))
        })
}

/// Props an objective works on, by key: the quest props still to use in the zone.
fn objective_props<'a>(content: &'a Content, o: &Objective, taken: &[String]) -> Vec<&'a str> {
    let by = |f: &dyn Fn(&combat_core::world::PropDef) -> bool| {
        content
            .world
            .props
            .iter()
            .filter(|p| f(p) && !taken.contains(&p.key))
            .map(|p| p.key.as_str())
            .collect::<Vec<_>>()
    };
    match o {
        Objective::Interact(k, _) => by(&|p| p.key == *k),
        Objective::Collect(tag, _) | Objective::Inspect(tag, _) => by(&|p| p.tag == *tag),
        Objective::UseOn { target, .. } => by(&|p| p.key == *target || p.tag == *target),
        Objective::Sequence { props, .. } => by(&|p| props.contains(&p.key)),
        _ => Vec::new(),
    }
}

/// First step toward `goal` zones over open links: the door anchor of the current zone to take.
pub(super) fn door_toward(
    sim: &dyn SimRead,
    from: &str,
    goals: &[&str],
    unlock_all: bool,
) -> Option<String> {
    // Breadth-first over the zone graph; each entry remembers the first door taken from `from`.
    let mut seen = vec![from.to_string()];
    let mut queue: Vec<(String, Option<String>)> = vec![(from.to_string(), None)];
    let mut i = 0;
    while i < queue.len() {
        let (zone, first) = queue[i].clone();
        i += 1;
        for l in sim.zone_links(&zone) {
            if !(l.unlocked || unlock_all) || seen.contains(&l.to) {
                continue;
            }
            let door = first.clone().or_else(|| Some(l.anchor.clone()));
            if goals.contains(&l.to.as_str()) {
                return door;
            }
            seen.push(l.to.clone());
            queue.push((l.to, door));
        }
    }
    None
}

/// World position the arrow should point at, if any: see the module docs.
pub fn waypoint_target(
    sim: &dyn SimRead,
    track: &QuestTrack,
    zone: &str,
    anchors: &[(&'static str, Vec3, f32)],
    unlock_all: bool,
    from: Vec3,
) -> Option<Vec3> {
    let content = sim.content();
    let def = followed(sim, track)?;
    let counts = &sim.quests().active.iter().find(|q| q.id == def.id)?.counts;
    let open: Vec<&Objective> = def
        .objectives
        .iter()
        .enumerate()
        .filter(|(k, o)| counts.get(*k).copied().unwrap_or(0) < o.need())
        .map(|(_, o)| o)
        .collect();
    let (keys, props, goals): (Vec<String>, Vec<&str>, Vec<&str>) = if open.is_empty() {
        // Work done: the turn-in giver.
        let e = def.ender();
        (vec![e.npc.clone()], Vec::new(), vec![e.zone.as_str()])
    } else {
        let taken = sim.taken_props();
        (
            open.iter()
                .flat_map(|o| objective_unit_keys(content, o))
                .collect(),
            open.iter()
                .flat_map(|o| objective_props(content, o, &taken))
                .collect(),
            open.iter()
                .flat_map(|o| content.objective_zones(o))
                .collect(),
        )
    };
    let flat = |p: Vec3| Vec2::new(p.x - from.x, p.z - from.z).length();
    let units = sim
        .units()
        .filter(|u| u.alive && keys.contains(&u.key))
        .map(|u| Vec3::new(u.pos.x, u.pos.y, u.pos.z));
    let at_props = props.iter().filter_map(|k| {
        let name = format!("prop:{k}");
        anchors.iter().find(|a| a.0 == name).map(|a| a.1)
    });
    if let Some(best) = units
        .chain(at_props)
        .min_by(|a, b| flat(*a).total_cmp(&flat(*b)))
    {
        return Some(best);
    }
    // Nothing of it here. In a zone that does host the work the arrow stays quiet (a respawn
    // is coming); elsewhere it points at the door that starts the route.
    if goals.contains(&zone) {
        return None;
    }
    let door = door_toward(sim, zone, &goals, unlock_all)?;
    anchors.iter().find(|a| a.0 == door).map(|a| a.1)
}

fn find_target(
    time: Res<Time<Real>>,
    sim: SimView,
    flow: Res<GameFlow>,
    track: Res<QuestTrack>,
    settings: Res<Settings>,
    player: Query<&Transform, With<Player>>,
    mut wp: ResMut<Waypoint>,
) {
    if !settings.quest_arrow {
        wp.target = None;
        return;
    }
    let now = time.elapsed_secs();
    if now < wp.next_search {
        return;
    }
    wp.next_search = now + REFRESH_SECS;
    wp.target = player.single().ok().and_then(|tf| {
        waypoint_target(
            &*sim,
            &track,
            &flow.zone,
            &flow.anchors,
            flow.unlock_all,
            tf.translation,
        )
    });
}

fn place_arrow(
    time: Res<Time>,
    wp: Res<Waypoint>,
    hidden: Res<HudHidden>,
    settings: Res<Settings>,
    paused: Res<Paused>,
    ui_scale: Res<UiScale>,
    player: Query<&Transform, (With<Player>, Without<ArrowMesh>)>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    mut arrow: Single<(&mut Transform, &mut Visibility), With<ArrowMesh>>,
    mut tag: Single<
        (&mut Node, &mut Text, &mut Visibility),
        (With<ArrowLabel>, Without<ArrowMesh>),
    >,
) {
    let (arrow_tf, arrow_vis) = &mut *arrow;
    let (label_node, label_text, label_vis) = &mut *tag;
    let target = wp
        .target
        .filter(|_| settings.quest_arrow && !hidden.cutscene && !hidden.opening && !paused.0);
    let (Some(target), Ok(me)) = (target, player.single()) else {
        arrow_vis.set_if_neq(Visibility::Hidden);
        label_vis.set_if_neq(Visibility::Hidden);
        return;
    };
    let to = Vec2::new(target.x - me.translation.x, target.z - me.translation.z);
    let dist = to.length();
    if dist < ARRIVED_M {
        arrow_vis.set_if_neq(Visibility::Hidden);
        label_vis.set_if_neq(Visibility::Hidden);
        return;
    }
    let bob = (time.elapsed_secs() * 3.0).sin() * 0.08;
    arrow_tf.translation = me.translation + Vec3::Y * (HOVER_M + bob);
    let want = Transform::default()
        .looking_to(Vec3::new(to.x, 0.0, to.y).normalize(), Vec3::Y)
        .rotation;
    let k = 1.0 - (-TURN_RATE * time.delta_secs()).exp();
    // Snap on the first frame shown, then ease.
    arrow_tf.rotation = if **arrow_vis == Visibility::Hidden {
        want
    } else {
        arrow_tf.rotation.slerp(want, k)
    };
    arrow_vis.set_if_neq(Visibility::Inherited);
    // Distance label under the arrow, projected to the window like a nameplate.
    let (cam, cam_tf) = *camera;
    let scale = ui_scale.0.max(0.01);
    let Ok(p) = cam.world_to_viewport(cam_tf, arrow_tf.translation - Vec3::Y * 0.5) else {
        label_vis.set_if_neq(Visibility::Hidden);
        return;
    };
    set_label(label_text, &format!("{dist:.0} m"));
    put!(label_node.left, px(p.x / scale - 18.0));
    put!(label_node.top, px(p.y / scale));
    label_vis.set_if_neq(Visibility::Inherited);
}

fn set_label(text: &mut Text, s: &str) {
    if text.0 != s {
        text.0.clear();
        text.0.push_str(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::{QuestStatus, Sim};

    fn sim() -> Sim {
        match crate::combat::new_sim() {
            Ok(s) => s,
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn a_door_is_the_first_hop_of_the_shortest_open_route() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        // From the campus to a tower floor: the tower's door is the campus door to the lobby
        // chain, whatever it is called, and it must exist among the campus links.
        let links = s.zone_links("campus");
        let door = door_toward(&s, "campus", &["tower/1"], true).expect("a route to tower/1");
        assert!(links.iter().any(|l| l.anchor == door));
        // A locked route yields nothing unless everything is opened.
        assert!(door_toward(&s, "campus", &["tower/5"], false).is_none());
        assert!(door_toward(&s, "campus", &["tower/5"], true).is_some());
    }

    #[test]
    fn the_arrow_points_at_the_giver_when_the_work_is_done_and_at_a_door_from_afar() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        let anchors = |n: &str| {
            let far = n.starts_with("pack_") || n.starts_with("rare_") || n == "boss";
            let at = if far { 400.0 } else { 7.0 };
            Some((combat_core::Vec3::new(at, 0.0, at), 0.0))
        };
        s.load_zone("campus", &anchors, Vec::new()).expect("campus");
        s.debug_set_quest("onboarding", QuestStatus::Ready);
        let track = QuestTrack::default();
        let giver = s.content().quest("onboarding").map(|q| q.ender().clone());
        let giver = giver.expect("onboarding");
        let at = waypoint_target(&s, &track, &giver.zone, &[], true, Vec3::ZERO);
        // The intern stands at the stubbed anchor of the zone that hosts him.
        assert!(at.is_some_and(|p| p.x == 7.0), "{at:?}");
        // From another zone the ticket's turn-in is not here: a door of this zone is the target.
        let door = s
            .zone_links("tower/1")
            .into_iter()
            .next()
            .expect("a door")
            .anchor;
        let doors: Vec<(&'static str, Vec3, f32)> = s
            .zone_links("tower/1")
            .into_iter()
            .map(|l| (&*l.anchor.leak(), Vec3::new(3.0, 0.0, 3.0), 0.0))
            .collect();
        assert!(doors.iter().any(|d| d.0 == door));
        s.load_zone("tower/1", &anchors, Vec::new())
            .expect("tower/1");
        let to_campus = waypoint_target(&s, &track, "tower/1", &doors, true, Vec3::ZERO);
        assert_eq!(to_campus, Some(Vec3::new(3.0, 0.0, 3.0)));
    }
}
