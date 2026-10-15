//! Day and night from the sim's office clock (`combat_core::live::darkness`): outdoors the
//! sun, ambient light, fog and sky dim towards a blue night and the office windows
//! (`Mat::Window`) glow; indoors the lights only drop a little. Each zone load sets the day
//! look (`floors`); this system takes whatever it set as the base and blends from there, so
//! the zone themes stay the single source of the daytime look. WebGL2-safe: no extra lights,
//! one shared material per zone.

use super::{LiveState, unix_now};
use crate::combat::{CombatSim, SimView};
use crate::floors::WindowMats;
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
    /// What the scene shows now, 0 (day) to 1 (night); read-only for other modules.
    pub darkness: f32,
}

impl Default for DayNight {
    fn default() -> Self {
        DayNight {
            freeze_day: std::env::var_os("WOW_FREEZE_DAY").is_some(),
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

const NIGHT_AMBIENT: Color = Color::srgb(0.35, 0.42, 0.78);
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
            brightness: self.brightness * (1.0 - 0.62 * d),
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
