//! Day and night from the sim's office clock (`combat_core::live::darkness`): outdoors the
//! sun, ambient light, fog and sky dim towards a blue night and the office windows
//! (`Mat::Window`) glow; indoors the lights only drop a little. Each zone load sets the day
//! look (`floors`); this system takes whatever it set as the base and blends from there, so
//! the zone themes stay the single source of the daytime look. Cheap: one shared material
//! per zone, and outdoors after dark a handful of real lights at the lamps nearest the
//! camera (`light_the_lamps`), the count set by the quality preset.

use super::{LiveState, unix_now};
use crate::combat::{CombatSim, SimView};
use crate::floors::{StreetLamps, WindowMats};
use crate::game::GameFlow;
use crate::net::NetWorld;
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use combat_core::ZoneKind;

/// Settings hook: `freeze_day` keeps the scene at noon (accessibility: some players need the
/// daylight). The settings window can bind it; `WOW_FREEZE_DAY=1` sets it at start. The sim's
/// night (spawns, chatter) is unaffected; only the lighting is frozen.
#[derive(Resource)]
pub struct DayNight {
    pub freeze_day: bool,
    /// QA hook: `WOW_NIGHT=1` holds the scene at full night (lamp and night screenshots).
    pub force_night: bool,
    /// What the scene shows now, 0 (day) to 1 (night); read-only for other modules.
    pub darkness: f32,
}

impl Default for DayNight {
    fn default() -> Self {
        DayNight {
            freeze_day: std::env::var_os("WOW_FREEZE_DAY").is_some(),
            force_night: std::env::var_os("WOW_NIGHT").is_some(),
            darkness: 0.0,
        }
    }
}

/// The zone's daytime look, as the zone load left it.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Look {
    ambient: Color,
    brightness: f32,
    sky: Color,
    sun: Color,
    lux: f32,
}

const NIGHT_AMBIENT: Color = Color::srgb(0.42, 0.5, 0.85);
const NIGHT_SKY: Color = Color::srgb(0.035, 0.045, 0.1);
const MOON: Color = Color::srgb(0.55, 0.62, 0.95);
const WINDOW_GLOW: LinearRgba = LinearRgba::rgb(2.2, 1.6, 0.8);

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    Color::LinearRgba(LinearRgba::new(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    ))
}

impl Look {
    fn at_night(self, d: f32, outdoor: bool) -> Look {
        if !outdoor {
            // Indoors the office lights stay on; the building just gets quieter.
            return Look {
                brightness: self.brightness * (1.0 - 0.2 * d),
                lux: self.lux * (1.0 - 0.35 * d),
                ..self
            };
        }
        Look {
            ambient: mix(self.ambient, NIGHT_AMBIENT, d),
            // Dark enough for the lamps to matter, light enough to still see the paths.
            brightness: self.brightness * (1.0 - 0.4 * d),
            sky: mix(self.sky, NIGHT_SKY, d),
            sun: mix(self.sun, MOON, d),
            lux: self.lux * (1.0 - 0.85 * d),
        }
    }
}

pub(super) fn light_the_world(
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    st: Res<LiveState>,
    flow: Res<GameFlow>,
    mut dn: ResMut<DayNight>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fogs: Query<&mut DistanceFog>,
    mut suns: Query<&mut DirectionalLight>,
    windows: Option<Res<WindowMats>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut base: Local<Option<(Look, Look)>>,
) {
    let Some(sun) = suns.iter().next().map(|l| (l.color, l.illuminance)) else {
        return;
    };
    let now = Look {
        ambient: ambient.color,
        brightness: ambient.brightness,
        sky: clear.0,
        sun: sun.0,
        lux: sun.1,
    };
    // Anything but what we wrote last frame is a fresh zone look: the new daytime base.
    let day = match *base {
        Some((day, written)) if written == now => day,
        _ => now,
    };
    let minute = unix_now(&*sim, &offline, net.is_some(), &st)
        .map(|u| (u % combat_core::live::GAME_DAY_S as f64) as f32);
    let d = match minute {
        _ if dn.force_night => 1.0,
        Some(m) if !dn.freeze_day => combat_core::live::darkness(m),
        _ => 0.0,
    };
    dn.darkness = d;
    let outdoor = sim
        .content()
        .zone(&flow.zone)
        .is_some_and(|z| z.kind == ZoneKind::Outdoor);
    let look = day.at_night(d, outdoor);
    if look != now {
        ambient.color = look.ambient;
        ambient.brightness = look.brightness;
        clear.0 = look.sky;
        for mut fog in &mut fogs {
            fog.color = look.sky;
        }
        for mut l in &mut suns {
            l.color = look.sun;
            l.illuminance = look.lux;
        }
    }
    *base = Some((day, look));
    if let Some(h) = windows.as_ref().and_then(|w| w.0.as_ref()) {
        let glow = WINDOW_GLOW * d;
        if materials.get(h).is_some_and(|m| m.emissive != glow)
            && let Some(mut m) = materials.get_mut(h)
        {
            m.emissive = glow;
        }
    }
}

/// One of the few real lights parked under the street lamps nearest the camera after dark.
#[derive(Component)]
pub(super) struct NightLamp;

const LAMP_POOL_GLOW: LinearRgba = LinearRgba::rgb(0.75, 0.52, 0.24);
const LAMP_LUMENS: f32 = 220_000.0;
/// Below this darkness the lamps are off: dusk only tints the heads.
const LAMPS_ON: f32 = 0.15;

/// Street lamps after dark: the heads glow harder, the ground pools fade in (both shared
/// materials, so free) and a few real point lights follow the camera from lamp to lamp so
/// characters and props near the player are actually lit.
#[allow(clippy::too_many_arguments)] // Bevy system params
pub(super) fn light_the_lamps(
    mut commands: Commands,
    dn: Res<DayNight>,
    lamps: Option<Res<StreetLamps>>,
    settings: Res<crate::settings::Settings>,
    cam: Query<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lights: Query<(&mut Transform, &mut PointLight, &mut Visibility), With<NightLamp>>,
    mut last: Local<f32>,
) {
    let Some(lamps) = lamps else { return };
    let d = if lamps.heads.is_empty() {
        0.0
    } else {
        dn.darkness
    };
    // Material writes re-upload; only touch them when the darkness moved noticeably.
    if (d - *last).abs() > 0.01 || lamps.is_changed() {
        *last = d;
        if let Some(mut m) = lamps.head.as_ref().and_then(|h| materials.get_mut(h)) {
            m.emissive = crate::floors::LAMP_DAY * (1.0 + 3.0 * d);
        }
        if let Some(mut m) = lamps.pool.as_ref().and_then(|h| materials.get_mut(h)) {
            m.base_color = (LAMP_POOL_GLOW * d).into();
        }
    }
    let want = if d < LAMPS_ON {
        0
    } else {
        (settings.quality.max_lights() / 2).max(1)
    };
    let have = lights.iter().len();
    for _ in have..want {
        commands.spawn((
            NightLamp,
            PointLight {
                color: Color::srgb(1.0, 0.82, 0.55),
                range: 16.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    let Some(eye) = cam.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let mut near: Vec<Vec3> = lamps.heads.clone();
    near.sort_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
    let mut spots = near.into_iter().take(want);
    for (mut tf, mut light, mut vis) in &mut lights {
        match spots.next() {
            Some(head) => {
                tf.translation = head - Vec3::Y * 0.4;
                light.intensity = LAMP_LUMENS * d;
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}
