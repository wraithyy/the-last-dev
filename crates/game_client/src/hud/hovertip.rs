//! IDE-style hover tooltips for aura icons (player, target, party) and the target's cast bar,
//! in the look of the action bar tooltip: `fn name() -> Kind`, a stats line, a `///` doc line.

use super::actionbar::spell_signature;
use super::quests::{GREY_MARK, TipQuest, unit_quest_lines};
use super::{
    ACCENT, DIM, GOLD, HudHidden, HudRects, asset_key, drop_shadow, label, panel_bg, put, set_text,
};
use crate::arena::Colliders;
use crate::combat::{SimReady, SimUnit, SimView};
use crate::input::Paused;
use crate::player::Player;
use crate::targeting::{Pickable, pick_ray};
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::content::DispelType;
use combat_core::{AuraView, CastView, Content, Effect};

const TIP_W: f32 = 330.0;
const GAP: f32 = 6.0;

/// What a hoverable HUD node shows. Written by the frame systems each frame, read here.
#[derive(Component, Default)]
pub struct TipSource {
    pub aura: Option<AuraView>,
    pub cast: Option<CastView>,
    /// The player's burnout (per mille), for the strip under the mana bar.
    pub burnout: Option<u16>,
    /// QA handle for `WOW_HOVER_TIP`: "player-aura", "target-aura", "party-aura", "target-cast",
    /// "burnout".
    pub tag: &'static str,
}

#[derive(Component)]
struct HoverTip;
#[derive(Component)]
struct HoverName;
#[derive(Component)]
struct HoverSig;
#[derive(Component)]
struct HoverLine(usize);

pub struct HoverTipPlugin;

impl Plugin for HoverTipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_tip, spawn_world_tip))
            .add_systems(Update, (update_tip, update_world_tip).in_set(SimReady));
    }
}

fn spawn_tip(mut commands: Commands) {
    commands
        .spawn((
            HoverTip,
            Node {
                position_type: PositionType::Absolute,
                width: px(TIP_W),
                padding: UiRect::axes(px(10), px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(45),
        ))
        .with_children(|t| {
            let span = |size: f32, color: Color| {
                (
                    TextSpan::new(""),
                    TextFont::from_font_size(size),
                    TextColor(color),
                )
            };
            t.spawn((
                label("fn ", 14.0),
                TextColor(Color::srgb(0.78, 0.55, 0.95)),
                children![
                    (HoverName, span(14.0, Color::srgb(0.95, 0.85, 0.55))),
                    (HoverSig, span(14.0, DIM)),
                ],
            ));
            t.spawn((
                HoverLine(0),
                label("", 12.0),
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
            ));
            t.spawn((
                HoverLine(1),
                label("", 12.0),
                TextColor(Color::srgb(0.5, 0.75, 0.55)),
            ));
        });
}

struct Tip {
    name: String,
    sig: String,
    stats: String,
    doc: String,
}

/// "12.3s" under ten seconds, then whole seconds, then "1m 05s".
fn fmt_remaining(ms: u32) -> String {
    if ms < 10_000 {
        format!("{:.1}s", ms as f32 / 1000.0)
    } else if ms < 60_000 {
        format!("{}s", ms.div_ceil(1000))
    } else {
        let s = ms.div_ceil(1000);
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

fn signed_pct(v: i32) -> String {
    format!("{}{}%", if v >= 0 { "+" } else { "-" }, v.abs())
}

/// The content defines no aura prose, so the doc line is derived from what the aura does.
fn aura_doc(a: &AuraView, content: &Content) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = content.aura(&a.key) {
        if d.stun {
            parts.push("Stunned: cannot cast, move or attack".into());
        }
        if d.silence {
            parts.push("Silenced: cannot cast spells".into());
        }
        if let Some(p) = &d.periodic {
            let every = p.interval_ms as f32 / 1000.0;
            parts.push(if p.heal {
                format!("Heals about {} every {every:.0}s", p.base)
            } else {
                format!("Deals about {} damage every {every:.0}s", p.base)
            });
        }
        if d.cast_time_pct != 0 {
            parts.push(format!(
                "{} cast time per stack",
                signed_pct(d.cast_time_pct)
            ));
        }
        if d.damage_taken_pct != 0 {
            parts.push(format!(
                "{} damage taken per stack",
                signed_pct(d.damage_taken_pct)
            ));
        }
        if d.damage_done_pct != 0 {
            parts.push(format!(
                "{} damage dealt per stack",
                signed_pct(d.damage_done_pct)
            ));
        }
        if d.haste_pct > 0 {
            parts.push(format!("+{}% haste", d.haste_pct));
        }
        if d.max_hp_pct != 0 {
            parts.push(format!("{} max HP per stack", signed_pct(d.max_hp_pct)));
        }
        if d.reflect {
            parts.push("Spell damage aimed at the bearer hits the caster instead".into());
        }
        if d.expire_damage > 0 {
            parts.push(format!("Deals {} damage when it runs out", d.expire_damage));
        }
        if d.spread_radius_m > 0.0 {
            parts.push(format!(
                "Spreads to the nearest enemy within {:.0} m when the bearer dies",
                d.spread_radius_m
            ));
        }
        if d.stack_on_repeat_cast {
            parts.push("Gains a stack when the same spell is cast twice in a row".into());
        }
    }
    // Marker auras have no stats of their own; say what uses them instead.
    for spell in &content.spells {
        for e in std::iter::once(&spell.effect).chain(&spell.extra_effects) {
            if let Effect::Damage {
                consume_aura: Some(c),
                ..
            } = e
                && c.aura == a.key
            {
                parts.push(format!(
                    "{} consumes it for +{}% damage per stack",
                    spell.name, c.bonus_pct
                ));
            }
        }
    }
    if let Some(n) = a.absorb_remaining {
        parts.push(format!("Absorbs the next {n} damage"));
    }
    if parts.is_empty() {
        parts.push("Does something. Nobody documented what".into());
    }
    format!("/// {}.", parts.join(". "))
}

fn aura_tip(a: &AuraView, content: &Content) -> Tip {
    let def = content.aura(&a.key);
    let kind = match def {
        Some(d) if d.stun => "Stun",
        Some(d) if d.silence => "Silence",
        Some(d) if d.periodic.as_ref().is_some_and(|p| p.heal) => "Hot",
        Some(d) if d.periodic.is_some() => "Dot",
        _ if a.harmful => "Debuff",
        _ => "Buff",
    };
    let mut stats = vec![format!("{} left", fmt_remaining(a.remaining_ms))];
    if a.stacks > 1 {
        stats.push(format!("x{} stacks", a.stacks));
    }
    if a.harmful {
        stats.push(match def.map(|d| d.dispel) {
            Some(DispelType::Never) => "dispel: never".to_string(),
            _ => "dispel: magic".to_string(),
        });
    }
    Tip {
        name: asset_key(&a.name),
        sig: format!("() -> {kind}"),
        stats: stats.join(" | "),
        doc: aura_doc(a, content),
    }
}

/// `fn burnout() -> 72%`, what it does to you now, and the content's advice for that level.
fn burnout_tip(b: u16, content: &Content) -> Tip {
    let def = &content.endgame.burnout;
    let pct = u32::from(b.min(combat_core::BURNOUT_MAX)) / 10;
    let mut effects = Vec::new();
    if def.speed_pct(b) < 100 {
        effects.push(format!("{}% slower", 100 - def.speed_pct(b)));
    }
    for (at, what) in [
        (def.yawn_at, "yawning"),
        (def.mutter_at, "muttering"),
        (def.reply_all_at, "replies to all"),
    ] {
        if at > 0 && b >= at {
            effects.push(what.to_string());
        }
    }
    if let Some(t) = def.tier(b) {
        let mut pct = |v: i32, what: &str| {
            if v != 0 {
                effects.push(format!("{what} {v:+}%"));
            }
        };
        pct(
            t.power_pct,
            if t.power_pct > 0 {
                "hyperfocus: spell power"
            } else {
                "brain fog: spell power"
            },
        );
        pct(-(t.regen_pct as i32), "mana regen");
        pct(t.cast_pct, "cast time");
        if t.misfire_pct > 0 {
            effects.push(format!("{}% misfire", t.misfire_pct));
        }
    }
    if b > 0 {
        effects.push("hair thins faster".into());
    }
    if def.tier_index(b) > 0 {
        effects.push("relief: coffee, foosball, pets, kill a manager".into());
    }
    let stats = if effects.is_empty() {
        "no side effects yet".to_string()
    } else {
        effects.join(" | ")
    };
    Tip {
        name: "burnout".into(),
        sig: format!("() -> {pct}%"),
        stats,
        doc: format!("/// Burnout {pct}%. {}", def.tip(b)),
    }
}

fn cast_tip(c: &CastView, content: &Content) -> Option<Tip> {
    let d = content.spell(c.spell)?;
    let (name, sig) = spell_signature(d, content);
    let stats = [
        format!("{:.1}s cast", c.total_ms as f32 / 1000.0),
        if d.range_m > 0.0 {
            format!("{:.0} m", d.range_m)
        } else {
            "self".to_string()
        },
        if c.interruptible {
            "interruptible".to_string()
        } else {
            "uninterruptible".to_string()
        },
    ];
    Some(Tip {
        name,
        sig,
        stats: stats.join(" | "),
        doc: format!("/// {}", d.description),
    })
}

fn update_tip(
    sim: SimView,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    camera: Single<&Camera, With<crate::camera::OrbitCamera>>,
    sources: Query<(&TipSource, &Interaction, &ComputedNode, &UiGlobalTransform)>,
    mut tip: Single<&mut Node, With<HoverTip>>,
    mut names: Query<&mut TextSpan, (With<HoverName>, Without<HoverSig>)>,
    mut sigs: Query<&mut TextSpan, (With<HoverSig>, Without<HoverName>)>,
    mut lines: Query<(&HoverLine, &mut Text)>,
    mut forced: Local<Option<Option<String>>>,
) {
    // QA hook: `WOW_HOVER_TIP=<tag>` pretends the pointer rests on the first node with that tag.
    // Read once: an env lookup allocates, and this system runs every frame.
    let forced = forced.get_or_insert_with(|| std::env::var("WOW_HOVER_TIP").ok());
    let live = |s: &TipSource| s.aura.is_some() || s.cast.is_some() || s.burnout.is_some();
    let hit = sources
        .iter()
        .find(|(s, i, ..)| **i != Interaction::None && live(s))
        .or_else(|| {
            let tag = forced.as_deref()?;
            sources.iter().find(|(s, ..)| s.tag == tag && live(s))
        });
    let content = sim.content();
    let shown = hit.and_then(|(s, _, node, gt)| {
        let t = match (&s.aura, &s.cast, s.burnout) {
            (Some(a), _, _) => aura_tip(a, content),
            (None, Some(c), _) => cast_tip(c, content)?,
            (None, None, Some(b)) => burnout_tip(b, content),
            _ => return None,
        };
        Some((t, node, gt))
    });
    let Some((t, node, gt)) = shown else {
        put!(tip.display, Display::None);
        return;
    };
    put!(tip.display, Display::Flex);
    // Physical px to node units (window factor and UiScale divided out), centre to edges.
    let k = node.inverse_scale_factor();
    let (centre, size) = (gt.translation * k, node.size() * k);
    let view_w = camera
        .logical_viewport_size()
        .map_or(window.width(), |v| v.x)
        / ui_scale.0.max(0.01);
    put!(
        tip.left,
        px((centre.x - TIP_W / 2.0).clamp(4.0, (view_w - TIP_W - 4.0).max(4.0)))
    );
    put!(tip.top, px(centre.y + size.y / 2.0 + GAP + 12.0));
    for mut span in &mut names {
        if span.0 != t.name {
            span.0.clone_from(&t.name);
        }
    }
    for mut span in &mut sigs {
        if span.0 != t.sig {
            span.0.clone_from(&t.sig);
        }
    }
    for (HoverLine(i), mut text) in &mut lines {
        set_text(&mut text, if *i == 0 { &t.stats } else { &t.doc });
    }
}

/// Reach of a finger for the long-press pick, like a tap on a unit.
const TOUCH_SLACK_M: f32 = 0.4;
/// A touch held this long (and not dragged) shows the tooltip, as a hover would.
const LONG_PRESS_SECS: f32 = 0.45;
const PRESS_SLOP_PX: f32 = 14.0;
const WORLD_TIP_W: f32 = 240.0;

/// The tooltip of the unit under the pointer: name, level, title and the player's quests.
#[derive(Component)]
struct WorldTip;

fn spawn_world_tip(mut commands: Commands) {
    commands.spawn((
        WorldTip,
        Node {
            position_type: PositionType::Absolute,
            max_width: px(WORLD_TIP_W),
            padding: UiRect::axes(px(10), px(7)),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(0.7)),
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(44),
    ));
}

/// The tooltip lines of a unit: (text, size, colour).
fn world_tip_lines(
    sim: &SimView,
    pal: &crate::settings::Palette,
    id: combat_core::EntityId,
    zone: &str,
) -> Vec<(String, f32, Color)> {
    let Some(u) = sim.unit(id).filter(|u| u.alive) else {
        return Vec::new();
    };
    let content = sim.content();
    let party = sim.players().contains(&id);
    let name_color = if u.hostile {
        pal.hostile
    } else if party {
        crate::nameplates::PARTY_BLUE
    } else {
        pal.friendly
    };
    let mut out = vec![(u.name.clone(), 15.0, name_color)];
    let tag = if u.boss {
        " (Boss)"
    } else if crate::copy::is_elite(&u.key) {
        " (Elite)"
    } else {
        ""
    };
    out.push((format!("Level {}{tag}", u.level), 12.0, DIM));
    let title = content
        .title_text(&u.title)
        .or_else(|| content.unit_def(&u.key).map(|d| d.title.as_str()))
        .unwrap_or("");
    if !title.is_empty() {
        out.push((format!("<{title}>"), 12.0, Color::srgb(0.75, 0.85, 0.78)));
    }
    for l in unit_quest_lines(&**sim, &u.key, zone) {
        out.push(match l {
            TipQuest::Giver { glyph, name, grey } => (
                format!("{glyph} {name}"),
                12.0,
                if grey { GREY_MARK } else { GOLD },
            ),
            TipQuest::Quest(name) => (name, 12.0, GOLD),
            TipQuest::Objective { text, done } => (
                format!("  {text}"),
                12.0,
                if done { GREY_MARK } else { GOLD },
            ),
        });
    }
    out
}

#[derive(Default)]
struct TipState {
    /// Seconds a still finger has been down.
    held: f32,
    /// The lines on screen, to rebuild the node only when they change.
    shown: String,
    forced: Option<Option<String>>,
}

/// Reasons the world tooltip stays hidden.
#[derive(bevy::ecs::system::SystemParam)]
struct TipGate<'w, 's> {
    hidden: Res<'w, HudHidden>,
    paused: Res<'w, Paused>,
    hud: Res<'w, HudRects>,
    buttons: Query<'w, 's, &'static Interaction>,
}

impl TipGate<'_, '_> {
    fn blocked(&self, at: Vec2) -> bool {
        self.hidden.cutscene
            || self.hidden.opening
            || self.paused.0
            || self.hud.hits(Rect::from_center_size(at, Vec2::splat(2.0)))
            || self.buttons.iter().any(|i| *i != Interaction::None)
    }
}

fn update_world_tip(
    mut commands: Commands,
    sim: SimView,
    pal: Res<crate::settings::Palette>,
    flow: Res<crate::game::GameFlow>,
    gate: TipGate,
    colliders: Res<Colliders>,
    touch: Res<crate::touch::TouchMode>,
    touches: Res<Touches>,
    time: Res<Time<Real>>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    units: Query<(Entity, &Transform, &Unit, &SimUnit), Without<Player>>,
    tip: Single<(Entity, &mut Node), With<WorldTip>>,
    mut st: Local<TipState>,
) {
    let TipState {
        held,
        shown,
        forced,
    } = &mut *st;
    let (tip_e, mut node) = tip.into_inner();
    // QA hook: `WOW_HOVER_UNIT=<unit key>` shows that unit's tooltip at the window centre.
    let forced = forced.get_or_insert_with(|| std::env::var("WOW_HOVER_UNIT").ok());
    let scale = ui_scale.0.max(0.01);
    // Touch has no hover: a still finger held down stands in for the cursor.
    let finger = touches
        .iter()
        .next()
        .filter(|t| t.position().distance(t.start_position()) < PRESS_SLOP_PX);
    *held = if finger.is_some() {
        *held + time.delta_secs()
    } else {
        0.0
    };
    let at = if touch.active {
        finger
            .filter(|_| *held >= LONG_PRESS_SECS)
            .map(|t| t.position())
    } else {
        window.cursor_position()
    };
    let slack = if touch.active { TOUCH_SLACK_M } else { 0.0 };
    let id = at.filter(|p| !gate.blocked(*p)).and_then(|p| {
        let (cam, cam_tf) = *camera;
        let ray = cam.viewport_to_world(cam_tf, p).ok()?;
        let by_entity = pick_ray(
            (ray.origin, *ray.direction),
            units
                .iter()
                .filter(|(_, _, u, _)| u.alive)
                .map(|(e, tf, u, _)| Pickable {
                    key: e,
                    pos: tf.translation,
                    unit: *u,
                }),
            &colliders.0,
            slack,
        )?;
        units.get(by_entity).ok().map(|(.., su)| (su.0, p))
    });
    let id = id.or_else(|| {
        let key = forced.as_deref()?;
        let u = sim.units().find(|u| u.key == key && u.alive)?;
        Some((u.id, Vec2::new(window.width(), window.height()) * 0.5))
    });
    let lines = id
        .map(|(id, _)| world_tip_lines(&sim, &pal, id, &flow.zone))
        .unwrap_or_default();
    let (Some((_, p)), false) = (id, lines.is_empty()) else {
        put!(node.display, Display::None);
        return;
    };
    put!(node.display, Display::Flex);
    let key = lines
        .iter()
        .map(|l| l.0.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if *shown != key {
        shown.clone_from(&key);
        commands.entity(tip_e).despawn_related::<Children>();
        for (text, size, color) in lines {
            commands
                .entity(tip_e)
                .with_child((label(text, size), TextColor(color)));
        }
    }
    // Right of and below the pointer; the node's own size is a frame late on a new tip, so
    // flip against the right edge with the widest it can be.
    let (view_w, view_h) = camera
        .0
        .logical_viewport_size()
        .map_or((window.width(), window.height()), |v| (v.x, v.y));
    let (px_, py_) = (p.x / scale, p.y / scale);
    let (vw, vh) = (view_w / scale, view_h / scale);
    let left = if px_ + 18.0 + WORLD_TIP_W > vw {
        px_ - WORLD_TIP_W - 12.0
    } else {
        px_ + 18.0
    };
    let top = if py_ + 120.0 > vh {
        py_ - 110.0
    } else {
        py_ + 16.0
    };
    put!(node.left, px(left.max(4.0)));
    put!(node.top, px(top.max(4.0)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_burnout_tip_quotes_the_level_and_its_advice() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        let t = burnout_tip(720, sim.content());
        assert_eq!(t.sig, "() -> 72%");
        assert_eq!(t.doc, "/// Burnout 72%. Have you tried a wellness webinar?");
        assert!(t.stats.contains("replies to all") && !t.stats.contains("slower"));
        assert_eq!(burnout_tip(0, sim.content()).stats, "no side effects yet");
    }

    #[test]
    fn remaining_time_reads_like_a_timer() {
        assert_eq!(fmt_remaining(4_250), "4.2s");
        assert_eq!(fmt_remaining(12_001), "13s");
        assert_eq!(fmt_remaining(65_000), "1m 05s");
    }

    #[test]
    fn every_aura_in_the_content_gets_a_doc_line() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        let c = sim.content();
        for def in &c.auras {
            let view = AuraView {
                key: def.key.clone(),
                name: def.name.clone(),
                icon: def.icon.clone(),
                harmful: def.harmful,
                remaining_ms: 5000,
                total_ms: 8000,
                absorb_remaining: None,
                stacks: 1,
            };
            let doc = aura_doc(&view, c);
            assert!(doc.starts_with("/// ") && doc.len() > 8, "{}", def.key);
        }
    }
}
