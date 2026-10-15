//! Item presentation shared by the character sheet, the shop, loot and chat: rarity colours,
//! dev-flavoured stat names, tooltips with a comparison against what is worn, upgrade checks.

use bevy::prelude::*;
use combat_core::{
    Content, GearDef, GearStats, Goods, ItemTotals, Perk, Rarity, Role, SLOTS, role_package,
};

pub const JUNK_GREY: Color = Color::srgb(0.55, 0.55, 0.55);
pub const UPGRADE_GREEN: Color = Color::srgb(0.25, 1.0, 0.35);
pub const DOWNGRADE_RED: Color = Color::srgb(1.0, 0.35, 0.3);
pub const PERK_GOLD: Color = Color::srgb(1.0, 0.85, 0.35);
pub const SP_GOLD: Color = Color::srgb(1.0, 0.8, 0.25);

/// Classic rarity colours.
pub fn rarity_color(r: Rarity) -> Color {
    match r {
        Rarity::Common => Color::srgb(0.72, 0.72, 0.72),
        Rarity::Uncommon => Color::srgb(0.12, 1.0, 0.0),
        Rarity::Rare => Color::srgb(0.0, 0.44, 0.87),
        Rarity::Epic => Color::srgb(0.64, 0.21, 0.93),
        Rarity::Legendary => Color::srgb(1.0, 0.5, 0.0),
    }
}

/// Name colour for anything that drops: gear by rarity, trophies gold-ish, junk grey.
pub fn name_color(content: &Content, name: &str) -> Color {
    match content.economy.goods(name) {
        Some(Goods::Gear(g)) => rarity_color(g.rarity),
        Some(Goods::Junk(_)) => JUNK_GREY,
        Some(Goods::Consumable(_)) => Color::WHITE,
        None => crate::copy::BONUS_GREEN,
    }
}

pub fn icon_of<'a>(content: &'a Content, name: &str) -> &'a str {
    if let Some(g) = content.economy.goods(name) {
        return match g {
            Goods::Gear(g) => &g.icon,
            Goods::Junk(j) => &j.icon,
            Goods::Consumable(c) => &c.icon,
        };
    }
    content.item(name).map_or("unknown", |i| i.icon.as_str())
}

/// One stat as the sheet names it: value, dev name, plain name.
fn stat_parts(t: &ItemTotals, haste_tenths: u32) -> Vec<(i64, &'static str, &'static str, bool)> {
    // (value, name, plain, value is in tenths)
    vec![
        (i64::from(t.max_hp), "Uptime", "HP", false),
        (i64::from(t.max_mana), "Context Window", "mana", false),
        (i64::from(t.spell_power), "Velocity", "spell power", false),
        (
            i64::from(t.power_tenths),
            "Throughput",
            "damage and healing done",
            true,
        ),
        (i64::from(t.crit_tenths), "Hot Takes", "crit chance", true),
        (
            i64::from(t.haste_pct * 10 + haste_tenths),
            "Caffeine",
            "haste",
            true,
        ),
        (i64::from(t.mana_regen), "Focus Time", "mana per 5 s", false),
        (
            i64::from(t.armor_tenths),
            "Plausible Deniability",
            "less damage taken",
            true,
        ),
    ]
}

fn fmt_val(v: i64, tenths: bool) -> String {
    if !tenths {
        return v.abs().to_string();
    }
    let a = v.abs();
    if a % 10 == 0 {
        format!("{}%", a / 10)
    } else {
        format!("{}.{}%", a / 10, a % 10)
    }
}

/// "+12 Uptime" lines for a stat block.
pub fn stat_lines(s: &GearStats) -> Vec<String> {
    stat_parts(&s.totals, s.haste_tenths)
        .into_iter()
        .filter(|(v, ..)| *v > 0)
        .map(|(v, name, _, t)| format!("+{} {name}", fmt_val(v, t)))
        .collect()
}

/// The character sheet's summary: every stat with its plain meaning.
pub fn sheet_lines(t: &ItemTotals) -> Vec<String> {
    stat_parts(t, 0)
        .into_iter()
        .map(|(v, name, plain, tenths)| {
            if tenths {
                format!("{name}: +{} ({plain})", fmt_val(v, true))
            } else {
                format!("{name}: +{v} ({plain})")
            }
        })
        .collect()
}

pub fn worn_in_slot<'a>(
    content: &'a Content,
    g: &GearDef,
    equipped: &[String],
) -> Option<&'a GearDef> {
    equipped
        .get(g.slot.index())
        .and_then(|n| content.economy.gear(n))
}

/// `stat_parts` order mapped onto `role_package` order (HP, mana, SP, crit, haste, mp5, armour,
/// power).
const PACKAGE_INDEX: [usize; 8] = [0, 1, 2, 7, 3, 4, 5, 6];

/// Whether the role cares about the `stat_parts` stat `k` at all (everyone cares without a role).
fn relevant(role: Option<Role>, k: usize) -> bool {
    role.is_none() || role_package(role)[PACKAGE_INDEX[k]] > 0
}

/// What a piece is worth to a role: its stats weighted by the role's own stat package, so a
/// healer's mug is judged on mana and haste, not on HP; perks add 15%. Without a role, the budget.
pub fn value(g: &GearDef, role: Option<Role>) -> u64 {
    let Some(role) = role else {
        return g.budget();
    };
    let s = g.stats();
    let t = &s.totals;
    let stats = [
        t.max_hp,
        t.max_mana,
        t.spell_power,
        t.crit_tenths,
        s.haste_tenths,
        t.mana_regen,
        t.armor_tenths,
        t.power_tenths,
    ];
    let pkg = role_package(Some(role));
    let v: u64 = stats.iter().zip(pkg).map(|(a, w)| u64::from(*a) * w).sum();
    v * if g.perk.is_some() { 115 } else { 100 } / 100
}

/// How a piece compares with what the slot holds for the role: `Greater` is an upgrade (an empty
/// slot always is), `None` for non-gear or the piece already worn.
pub fn compare(
    content: &Content,
    name: &str,
    equipped: &[String],
    role: Option<Role>,
) -> Option<std::cmp::Ordering> {
    let g = content.economy.gear(name)?;
    if equipped.iter().any(|e| e == name) {
        return None;
    }
    Some(match worn_in_slot(content, g, equipped) {
        Some(w) => value(g, role).cmp(&value(w, role)),
        None => std::cmp::Ordering::Greater,
    })
}

/// True when the piece is worth more to the role than what the slot holds (or the slot is empty).
pub fn is_upgrade(content: &Content, name: &str, equipped: &[String], role: Option<Role>) -> bool {
    compare(content, name, equipped, role) == Some(std::cmp::Ordering::Greater)
}

/// Sum of stat budgets of the worn pieces: the "gear score" the chat brags about.
pub fn gear_score(content: &Content, equipped: &[String]) -> u64 {
    equipped
        .iter()
        .filter_map(|n| content.economy.gear(n))
        .map(GearDef::budget)
        .sum()
}

/// Who is looking at a tooltip and where.
pub struct Viewer<'a> {
    /// Worn pieces padded to `SLOTS`.
    pub equipped: &'a [String],
    pub role: Option<Role>,
    pub level: u8,
    pub kudos: u32,
    /// Hovered on a vendor's shelf: show the price and the level the vendor wants.
    pub buying: bool,
}

/// A tooltip: the hovered item, and beside it the piece worn in that slot (WoW-style).
pub struct Tip {
    pub main: Vec<(String, Color)>,
    pub worn: Option<Vec<(String, Color)>>,
}

const DIM_TIP: Color = Color::srgb(0.6, 0.66, 0.76);

/// Name, rarity line, stats, role hint, set progress, perk and description of one piece.
fn gear_lines(content: &Content, g: &GearDef, v: &Viewer) -> Vec<(String, Color)> {
    let eco = &content.economy;
    let mut out = vec![
        (g.name.clone(), rarity_color(g.rarity)),
        (
            format!(
                "{}  |  {}  |  item level {}",
                g.rarity.label(),
                g.slot.label(),
                g.ilvl
            ),
            rarity_color(g.rarity),
        ),
    ];
    for l in stat_lines(&g.stats()) {
        out.push((l, Color::WHITE));
    }
    match g.role {
        Some(r) if Some(r) == v.role => {
            out.push((format!("Recommended for {}", r.tag()), UPGRADE_GREEN))
        }
        Some(r) => out.push((format!("Recommended for {}", r.tag()), DOWNGRADE_RED)),
        None => out.push(("Recommended for anyone with a pulse".into(), DIM_TIP)),
    }
    if let Some(key) = &g.set
        && let Some(set) = eco.set(key)
    {
        let n = eco.set_count(key, v.equipped);
        out.push((format!("{} ({n}/4)", set.name), PERK_GOLD));
        let on = |k: usize| if n >= k { PERK_GOLD } else { DIM_TIP };
        out.push((format!("(2) {}", set.two_text), on(2)));
        out.push((format!("(4) {}", set.four_text), on(4)));
    }
    if !g.perk_text.is_empty() {
        let tag = if matches!(g.perk, Some(Perk::OnUse { .. })) {
            ""
        } else {
            "Perk: "
        };
        out.push((format!("{tag}{}", g.perk_text), PERK_GOLD));
    }
    out.push((format!("\"{}\"", g.description), DIM_TIP));
    out
}

/// Set pieces worn before and after swapping `g` in, for each set the swap touches.
fn set_changes(content: &Content, g: &GearDef, v: &Viewer) -> Vec<(String, Color)> {
    let eco = &content.economy;
    let mut after = v.equipped.to_vec();
    if let Some(slot) = after.get_mut(g.slot.index()) {
        slot.clone_from(&g.name);
    }
    let worn = worn_in_slot(content, g, v.equipped);
    let mut keys: Vec<&String> = g.set.iter().collect();
    if let Some(k) = worn.and_then(|w| w.set.as_ref())
        && !keys.contains(&k)
    {
        keys.push(k);
    }
    keys.into_iter()
        .filter_map(|k| {
            let set = eco.set(k)?;
            let (a, b) = (eco.set_count(k, v.equipped), eco.set_count(k, &after));
            let bonus = |n: usize| match n {
                0 | 1 => "no bonus",
                2 | 3 => "(2) bonus",
                _ => "(2) and (4) bonus",
            };
            let color = if b > a { UPGRADE_GREEN } else { DOWNGRADE_RED };
            (a != b).then(|| {
                let tail = if bonus(a) == bonus(b) {
                    String::new()
                } else {
                    format!(": {} -> {}", bonus(a), bonus(b))
                };
                (format!("{} {a}/4 -> {b}/4{tail}", set.name), color)
            })
        })
        .collect()
}

/// The verdict line under a comparison.
fn verdict(new: u64, old: u64) -> (String, Color) {
    if new > old {
        let sure = if new * 100 >= old * 120 {
            "definitely"
        } else {
            "probably, 51% confident"
        };
        (
            format!("Upgrade (according to the AI: {sure})"),
            UPGRADE_GREEN,
        )
    } else if new < old {
        (
            "Downgrade (according to the AI: a bold refactor)".into(),
            DOWNGRADE_RED,
        )
    } else {
        (
            "Sidegrade (according to the AI: it depends)".into(),
            DIM_TIP,
        )
    }
}

/// Comparison block for gear that is not worn: role-relevant stat deltas in green or red (the
/// rest dimmed), perk and set changes, and the verdict.
fn comparison(content: &Content, g: &GearDef, v: &Viewer) -> Vec<(String, Color)> {
    let Some(w) = worn_in_slot(content, g, v.equipped) else {
        return vec![("Empty slot - anything is an upgrade".into(), UPGRADE_GREEN)];
    };
    let mut out = vec![("If you wear it instead:".to_string(), DIM_TIP)];
    let (new, old) = (g.stats(), w.stats());
    let a = stat_parts(&new.totals, new.haste_tenths);
    let b = stat_parts(&old.totals, old.haste_tenths);
    let tag = v.role.map_or("", Role::tag);
    for (k, ((va, name, _, tenths), (vb, ..))) in a.into_iter().zip(b).enumerate() {
        let d = va - vb;
        if d == 0 {
            continue;
        }
        let sign = if d > 0 { "+" } else { "-" };
        let text = format!("  {sign}{} {name}", fmt_val(d, tenths));
        out.push(if relevant(v.role, k) {
            (text, if d > 0 { UPGRADE_GREEN } else { DOWNGRADE_RED })
        } else {
            (format!("{text} (not a {tag} stat)"), DIM_TIP)
        });
    }
    if out.len() == 1 {
        out.push((
            "  no stat change (a sidegrade, like most reorgs)".into(),
            DIM_TIP,
        ));
    }
    if g.perk_text != w.perk_text {
        if !g.perk_text.is_empty() {
            out.push((format!("  Gains: {}", g.perk_text), UPGRADE_GREEN));
        }
        if !w.perk_text.is_empty() {
            out.push((format!("  Loses: {}", w.perk_text), DOWNGRADE_RED));
        }
    }
    out.extend(
        set_changes(content, g, v)
            .into_iter()
            .map(|(t, c)| (format!("  {t}"), c)),
    );
    out.push(verdict(value(g, v.role), value(w, v.role)));
    out
}

/// The hovered item's tooltip, with the worn piece of the same slot beside gear.
pub fn tooltip(content: &Content, name: &str, v: &Viewer) -> Tip {
    let eco = &content.economy;
    let mut worn = None;
    let mut out = match eco.goods(name) {
        Some(Goods::Gear(g)) => {
            let mut out = gear_lines(content, g, v);
            if v.equipped.iter().any(|e| e == name) {
                out.push(("Equipped".into(), DIM_TIP));
            } else {
                out.extend(comparison(content, g, v));
                worn = worn_in_slot(content, g, v.equipped).map(|w| {
                    let mut lines = vec![("Currently equipped".to_string(), DIM_TIP)];
                    lines.extend(gear_lines(content, w, v));
                    lines
                });
            }
            if v.buying {
                let need = g.required_level();
                let color = if v.level >= need {
                    DIM_TIP
                } else {
                    DOWNGRADE_RED
                };
                out.push((format!("Requires Level {need}"), color));
            }
            out
        }
        Some(Goods::Junk(j)) => vec![
            (name.to_string(), JUNK_GREY),
            (
                "Junk. Sells to any vendor, judged by all of them.".into(),
                JUNK_GREY,
            ),
            (format!("\"{}\"", j.description), DIM_TIP),
        ],
        Some(Goods::Consumable(c)) => vec![
            (name.to_string(), Color::WHITE),
            (
                "Consumable. Click in the bag to use (shared 30 s cooldown).".into(),
                DIM_TIP,
            ),
            (c.description.clone(), Color::WHITE),
        ],
        None => {
            let mut out = vec![(name.to_string(), name_color(content, name))];
            if let Some(t) = content.item(name) {
                out.push(("Trophy (permanent bonus, window B)".into(), DIM_TIP));
                out.push((
                    crate::copy::bonus_text(&t.bonus.totals(1)),
                    crate::copy::BONUS_GREEN,
                ));
                out.push((format!("\"{}\"", t.description), DIM_TIP));
            }
            out
        }
    };
    if v.buying
        && let Some(g) = eco.goods(name)
    {
        let p = g.price();
        let color = if v.kudos >= p { SP_GOLD } else { DOWNGRADE_RED };
        out.push((format!("Price: {}", crate::copy::kudos(p)), color));
    } else if let Some(p) = eco.sell_price(name) {
        out.push((format!("Sells for {}", crate::copy::kudos(p)), SP_GOLD));
    }
    Tip { main: out, worn }
}

/// Equipped piece names padded to `SLOTS`.
pub fn slots_of(equipped: &[String]) -> Vec<String> {
    let mut v = equipped.to_vec();
    v.resize(SLOTS, String::new());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        match Content::from_ron(include_str!("../../../../content/game.ron")) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn every_economy_item_has_a_shipped_icon() {
        let c = content();
        let eco = &c.economy;
        let icons = eco
            .gear
            .iter()
            .map(|g| (&g.name, &g.icon))
            .chain(eco.junk.iter().map(|j| (&j.name, &j.icon)))
            .chain(eco.consumables.iter().map(|x| (&x.name, &x.icon)));
        let missing: Vec<String> = icons
            .filter(|(_, i)| !crate::hud::ICON_FILES.contains(&i.as_str()))
            .map(|(n, i)| format!("{n}: {i}"))
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    fn viewer(equipped: &[String], role: Option<Role>) -> Viewer<'_> {
        Viewer {
            equipped,
            role,
            level: 1,
            kudos: 0,
            buying: true,
        }
    }

    fn texts(lines: &[(String, Color)]) -> Vec<&str> {
        lines.iter().map(|(t, _)| t.as_str()).collect()
    }

    #[test]
    fn tooltip_compares_with_the_worn_piece_and_flags_upgrades() {
        let c = content();
        let mut worn = vec![String::new(); SLOTS];
        worn[1] = "Hoodie From a Hackathon Nobody Won".into();
        let tip = tooltip(
            &c,
            "Hoodie of Plausible Deniability",
            &viewer(&worn, Some(Role::Tank)),
        );
        let text = texts(&tip.main);
        assert!(text.contains(&"If you wear it instead:"), "{text:?}");
        assert!(
            text.iter()
                .any(|t| t.starts_with("Upgrade (according to the AI"))
        );
        assert!(text.contains(&"Recommended for TANK"));
        assert!(text.iter().any(|t| t.starts_with("Requires Level ")));
        assert!(
            text.iter()
                .any(|t| t.starts_with("Price: ") && t.ends_with(" Kudos"))
        );
        let side = tip.worn.expect("worn piece beside it");
        assert_eq!(side[0].0, "Currently equipped");
        assert_eq!(side[1].0, "Hoodie From a Hackathon Nobody Won");
        assert!(is_upgrade(
            &c,
            "Hoodie of Plausible Deniability",
            &worn,
            Some(Role::Tank)
        ));
        assert!(!is_upgrade(
            &c,
            "Hoodie From a Hackathon Nobody Won",
            &worn,
            Some(Role::Tank)
        ));
        assert!(gear_score(&c, &worn) > 0);
    }

    #[test]
    fn empty_slots_and_downgrades_read_as_such() {
        let c = content();
        let empty = vec![String::new(); SLOTS];
        let tip = tooltip(
            &c,
            "Hoodie From a Hackathon Nobody Won",
            &viewer(&empty, Some(Role::Healer)),
        );
        assert!(texts(&tip.main).contains(&"Empty slot - anything is an upgrade"));
        assert!(tip.worn.is_none());
        let mut worn = empty.clone();
        worn[1] = "Hoodie of Plausible Deniability".into();
        let tip = tooltip(
            &c,
            "Hoodie From a Hackathon Nobody Won",
            &viewer(&worn, Some(Role::Tank)),
        );
        let text = texts(&tip.main);
        assert!(text.iter().any(|t| t.starts_with("Downgrade")), "{text:?}");
        // Red stat deltas for the stats a tank wants.
        assert!(
            tip.main
                .iter()
                .any(|(t, col)| t.contains("Uptime") && t.contains('-') && *col == DOWNGRADE_RED),
            "{text:?}"
        );
        assert_eq!(
            compare(
                &c,
                "Hoodie From a Hackathon Nobody Won",
                &worn,
                Some(Role::Tank)
            ),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn a_role_values_only_its_own_stats() {
        let c = content();
        let eco = &c.economy;
        let tank = eco
            .gear
            .iter()
            .find(|g| g.role == Some(Role::Tank) && g.ilvl == 2)
            .expect("tier 1 tank piece");
        let healer = eco
            .gear
            .iter()
            .find(|g| g.role == Some(Role::Healer) && g.ilvl == 2 && g.rarity == tank.rarity)
            .expect("tier 1 healer piece");
        assert!(value(tank, Some(Role::Tank)) > value(healer, Some(Role::Tank)));
        assert!(value(healer, Some(Role::Healer)) > value(tank, Some(Role::Healer)));
    }
}
