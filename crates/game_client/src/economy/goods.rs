//! Item presentation shared by the character sheet, the shop, loot and chat: rarity colours,
//! dev-flavoured stat names, tooltips with a comparison against what is worn, upgrade checks.

use bevy::prelude::*;
use combat_core::{Content, GearDef, GearStats, Goods, ItemTotals, Perk, Rarity, Role, SLOTS};

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

/// Per-stat change from `old` to `new` gear, as coloured "+3 Velocity" / "-12 Uptime" lines.
pub fn delta_lines(new: &GearStats, old: &GearStats) -> Vec<(String, Color)> {
    let a = stat_parts(&new.totals, new.haste_tenths);
    let b = stat_parts(&old.totals, old.haste_tenths);
    a.into_iter()
        .zip(b)
        .filter_map(|((va, name, _, t), (vb, ..))| {
            let d = va - vb;
            (d != 0).then(|| {
                let sign = if d > 0 { "+" } else { "-" };
                let color = if d > 0 { UPGRADE_GREEN } else { DOWNGRADE_RED };
                (format!("{sign}{} {name}", fmt_val(d, t)), color)
            })
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

/// True when the piece scores higher for the role than what the slot holds (or the slot is empty).
pub fn is_upgrade(content: &Content, name: &str, equipped: &[String], role: Option<Role>) -> bool {
    let eco = &content.economy;
    let Some(g) = eco.gear(name) else {
        return false;
    };
    if equipped.iter().any(|e| e == name) {
        return false;
    }
    let worn = worn_in_slot(content, g, equipped).map_or(0, |w| eco.score(w, role));
    eco.score(g, role) > worn
}

/// Sum of stat budgets of the worn pieces: the "gear score" the chat brags about.
pub fn gear_score(content: &Content, equipped: &[String]) -> u64 {
    equipped
        .iter()
        .filter_map(|n| content.economy.gear(n))
        .map(GearDef::budget)
        .sum()
}

/// Tooltip lines: name in rarity colour, rarity and slot, stats, role hint, set progress,
/// perk, comparison with the worn piece, description, price.
pub fn tooltip(
    content: &Content,
    name: &str,
    equipped: &[String],
    role: Option<Role>,
) -> Vec<(String, Color)> {
    let eco = &content.economy;
    let dim = Color::srgb(0.6, 0.66, 0.76);
    let mut out = vec![(name.to_string(), name_color(content, name))];
    match eco.goods(name) {
        Some(Goods::Gear(g)) => {
            out.push((
                format!(
                    "{}  |  {}  |  item level {}",
                    g.rarity.label(),
                    g.slot.label(),
                    g.ilvl
                ),
                rarity_color(g.rarity),
            ));
            for l in stat_lines(&g.stats()) {
                out.push((l, Color::WHITE));
            }
            match g.role {
                Some(r) if Some(r) == role => {
                    out.push((format!("Recommended for {}", r.tag()), UPGRADE_GREEN))
                }
                Some(r) => out.push((format!("Recommended for {}", r.tag()), DOWNGRADE_RED)),
                None => out.push(("Recommended for anyone with a pulse".into(), dim)),
            }
            if let Some(key) = &g.set
                && let Some(set) = eco.set(key)
            {
                let n = eco.set_count(key, equipped);
                out.push((format!("{} ({n}/4)", set.name), PERK_GOLD));
                let on = |k: usize| if n >= k { PERK_GOLD } else { dim };
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
            if equipped.iter().any(|e| e == name) {
                out.push(("Equipped".into(), dim));
            } else {
                match worn_in_slot(content, g, equipped) {
                    Some(w) => {
                        out.push((format!("Compared with [{}]:", w.name), dim));
                        let d = delta_lines(&g.stats(), &w.stats());
                        if d.is_empty() {
                            out.push((
                                "  no stat change (a sidegrade, like most reorgs)".into(),
                                dim,
                            ));
                        }
                        out.extend(d.into_iter().map(|(t, c)| (format!("  {t}"), c)));
                    }
                    None => out.push(("Empty slot: pure upgrade".into(), UPGRADE_GREEN)),
                }
                if is_upgrade(content, name, equipped, role) {
                    out.push(("UPGRADE".into(), UPGRADE_GREEN));
                }
            }
            out.push((format!("\"{}\"", g.description), dim));
        }
        Some(Goods::Junk(j)) => {
            out.push((
                "Junk. Sells to any vendor, judged by all of them.".into(),
                JUNK_GREY,
            ));
            out.push((format!("\"{}\"", j.description), dim));
        }
        Some(Goods::Consumable(c)) => {
            out.push((
                "Consumable. Click in the bag to use (shared 30 s cooldown).".into(),
                dim,
            ));
            out.push((c.description.clone(), Color::WHITE));
        }
        None => {
            if let Some(t) = content.item(name) {
                out.push(("Trophy (permanent bonus, window B)".into(), dim));
                out.push((
                    crate::copy::bonus_text(&t.bonus.totals(1)),
                    crate::copy::BONUS_GREEN,
                ));
                out.push((format!("\"{}\"", t.description), dim));
            }
        }
    }
    if let Some(p) = eco.sell_price(name) {
        out.push((format!("Sells for {p} SP"), SP_GOLD));
    }
    out
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

    #[test]
    fn tooltip_compares_with_the_worn_piece_and_flags_upgrades() {
        let c = content();
        let mut worn = vec![String::new(); SLOTS];
        worn[1] = "Hoodie From a Hackathon Nobody Won".into();
        let lines = tooltip(
            &c,
            "Hoodie of Plausible Deniability",
            &worn,
            Some(Role::Tank),
        );
        let text: Vec<&str> = lines.iter().map(|(t, _)| t.as_str()).collect();
        assert!(
            text.iter().any(|t| t.starts_with("Compared with")),
            "{text:?}"
        );
        assert!(text.contains(&"UPGRADE"));
        assert!(text.contains(&"Recommended for TANK"));
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
}
