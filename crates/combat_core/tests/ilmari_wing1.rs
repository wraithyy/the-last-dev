//! Ilmari Wing 1, Datacenter Uprising (docs/ILMARI_09.md B.3, B.4): Patch Tuesday's rule changes,
//! the Recruiter's candidates and `death_burnout`, the Cable Hydra's growing cables and the label
//! printers, the per-difficulty lockouts, tokens and boss drops. Balance (Story solo, Full trio)
//! lives in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::{
    BurnoutCause, Command, Content, EntityId, Event, GearSource, Sim, SpellId, Unlock, Vec3,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;

/// Packs and props out of the way; the boss 20 m north of the arrival, printers at hand.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn sturdy(c: &mut Content) {
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
}

/// `n` capped players on the campus; `story` picks the wing difficulty.
fn wing(n: usize, story: bool, classes: &[&str]) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(
        {
            let mut c = content();
            sturdy(&mut c);
            c
        },
        33,
    );
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player(
                classes[k % classes.len()],
                Vec3::new(k as f32, 0.0, 0.0),
                0.0,
            );
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_force_raid_async(Some(story));
    (sim, ids)
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
}

fn unit_of(sim: &Sim, key: &str) -> Option<EntityId> {
    sim.units().find(|u| u.key == key && u.alive).map(|u| u.id)
}

fn move_to(sim: &mut Sim, p: EntityId, pos: Vec3) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
}

/// Everyone 12 m south of `t`, the first member opening on it with Code Review.
fn pull(sim: &mut Sim, ids: &[EntityId], t: EntityId) {
    for &p in ids {
        move_to(sim, p, Vec3::new(0.0, 0.0, -8.0));
    }
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(t),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
}

fn auras(sim: &Sim, id: EntityId) -> Vec<String> {
    sim.unit(id)
        .map(|u| u.auras.iter().map(|a| a.key.clone()).collect())
        .unwrap_or_default()
}

/// (ability spell, the patch that enables it).
const PATCH_SPELLS: [(u32, &str); 6] = [
    (2412, "patch_meeting_moved"),
    (2413, "patch_dependencies"),
    (2414, "patch_breaking"),
    (2415, "patch_breaking"),
    (2416, "patch_deprecation"),
    (2417, "patch_pair"),
];

/// Watches the boss for `secs` seconds: (tick of each reboot, patch applied at it, whether the
/// boss stood stunned right after, every patch ability that started with the patch it ran under).
type Watched = (Vec<(u64, String)>, Vec<(u32, Vec<String>)>);

fn watch_boss(story: bool, secs: u32) -> Watched {
    let (mut sim, ids) = wing(3, story, &["fullstack", "backend", "frontend"]);
    load(&mut sim, "ilmari1/1");
    let boss = unit_of(&sim, "il_patch_tuesday").expect("boss");
    pull(&mut sim, &ids, boss);
    let (mut reboots, mut casts) = (Vec::new(), Vec::new());
    let mut pending: Option<u64> = None;
    for _ in 0..secs * SECOND {
        for e in sim.tick() {
            match e {
                Event::AuraApplied { target, key } if target == boss => {
                    if key == "il_rebooting" {
                        pending = Some(sim.now().0);
                    } else if key.starts_with("patch_") {
                        reboots.push((pending.take().unwrap_or(0), key));
                    }
                }
                Event::CastStarted { caster, spell, .. }
                    if caster == boss && PATCH_SPELLS.iter().any(|(s, _)| *s == spell.0) =>
                {
                    casts.push((spell.0, auras(&sim, boss)));
                }
                _ => {}
            }
        }
        // The rules run while the boss lives: keep it alive, the group alive.
        sim.debug_set_hp(boss, 10_000_000);
    }
    (reboots, casts)
}

#[test]
fn patch_tuesday_reboots_every_minute_and_each_patch_enables_only_its_ability() {
    let (reboots, casts) = watch_boss(false, 190);
    assert!(
        reboots.len() >= 3,
        "three reboots in three minutes: {reboots:?}"
    );
    for w in reboots.windows(2) {
        let gap = (w[1].0 - w[0].0) / u64::from(SECOND);
        assert!((58..=62).contains(&gap), "a reboot a minute: {gap} s");
    }
    assert!(
        casts.iter().all(|(spell, held)| {
            PATCH_SPELLS
                .iter()
                .find(|(s, _)| s == spell)
                .is_some_and(|(_, patch)| held.iter().any(|a| a == patch))
        }),
        "every patch ability ran under its own patch: {casts:?}"
    );
    // Over the whole watch, more than one patch showed up (the rule really changes).
    let kinds: std::collections::BTreeSet<_> = reboots.iter().map(|r| r.1.clone()).collect();
    assert!(kinds.len() >= 2, "{kinds:?}");
}

#[test]
fn on_story_the_reboot_is_rarer_and_only_three_patches_ship() {
    let (reboots, casts) = watch_boss(true, 280);
    assert!(reboots.len() >= 3, "{reboots:?}");
    for w in reboots.windows(2) {
        let gap = (w[1].0 - w[0].0) / u64::from(SECOND);
        assert!((88..=92).contains(&gap), "every 90 s: {gap}");
    }
    for (_, patch) in &reboots {
        assert!(
            [
                "patch_meeting_moved",
                "patch_dependencies",
                "patch_breaking"
            ]
            .contains(&patch.as_str()),
            "{patch}"
        );
    }
    assert!(
        casts
            .iter()
            .all(|(s, _)| *s != 2416 && *s != 2417 && *s != 2414)
    );
}

#[test]
fn every_patch_aura_carries_a_changelog_of_at_most_five_short_lines() {
    let c = content();
    let patches: Vec<_> = c
        .auras
        .iter()
        .filter(|a| a.key.starts_with("patch_"))
        .collect();
    assert_eq!(patches.len(), 5);
    for a in patches {
        let lines: Vec<&str> = a.changelog.split('|').collect();
        assert!((2..=5).contains(&lines.len()), "{}", a.key);
        assert!(lines.iter().all(|l| l.chars().count() <= 39), "{}", a.key);
    }
}

#[test]
fn candidates_are_three_people_and_three_androids_and_only_a_human_death_costs_burnout() {
    let (mut sim, ids) = wing(3, false, &["fullstack", "backend", "frontend"]);
    load(&mut sim, "ilmari1/2");
    let boss = unit_of(&sim, "il_recruiter").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut fuse_ms = 0;
    for _ in 0..12 * SECOND {
        for e in sim.tick() {
            if let Event::CastStarted {
                spell, total_ms, ..
            } = e
                && spell.0 == 2430
            {
                fuse_ms = total_ms;
            }
        }
        sim.debug_set_hp(boss, 10_000_000);
    }
    let count = |sim: &Sim, k: &str| sim.units().filter(|u| u.key == k && u.alive).count();
    assert_eq!(
        (
            count(&sim, "il_candidate_human"),
            count(&sim, "il_candidate_fake")
        ),
        (3, 3)
    );
    assert_eq!(fuse_ms, 12_000, "the android fuse");
    // Killing an android costs nothing; killing a person costs everyone 15% burnout.
    let fake = unit_of(&sim, "il_candidate_fake").expect("fake");
    assert!(sim.debug_kill(fake));
    let before: Vec<_> = sim
        .tick()
        .into_iter()
        .filter(|e| {
            matches!(
                e,
                Event::Burnout {
                    cause: BurnoutCause::Collateral,
                    ..
                }
            )
        })
        .collect();
    assert!(before.is_empty());
    let human = unit_of(&sim, "il_candidate_human").expect("human");
    assert!(sim.debug_kill(human));
    let hits: Vec<_> = sim
        .tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::Burnout {
                unit,
                delta,
                cause: BurnoutCause::Collateral,
                ..
            } => Some((unit, delta)),
            _ => None,
        })
        .collect();
    assert_eq!(hits.len(), 3, "the whole group: {hits:?}");
    assert!(hits.iter().all(|(_, d)| *d == 150), "{hits:?}");
}

#[test]
fn spared_candidates_walk_out_alive_and_cost_nothing() {
    let (mut sim, ids) = wing(3, false, &["fullstack", "backend", "frontend"]);
    load(&mut sim, "ilmari1/2");
    let boss = unit_of(&sim, "il_recruiter").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut left = 0;
    let mut burnout = 0;
    for _ in 0..22 * SECOND {
        for e in sim.tick() {
            match e {
                Event::Death { unit, killer: None }
                    if sim
                        .unit(unit)
                        .is_some_and(|u| u.key == "il_candidate_human") =>
                {
                    left += 1
                }
                Event::Burnout {
                    cause: BurnoutCause::Collateral,
                    ..
                } => burnout += 1,
                _ => {}
            }
        }
        sim.debug_set_hp(boss, 10_000_000);
        // Anyone who is not killed by a player walks out through the door.
        for f in sim
            .units()
            .filter(|u| u.key == "il_candidate_fake")
            .map(|u| u.id)
            .collect::<Vec<_>>()
        {
            sim.debug_kill(f);
        }
    }
    assert!(left >= 3, "the first wave of people left: {left}");
    assert_eq!(burnout, 0);
}

#[test]
fn story_sends_one_person_and_one_android_with_a_longer_fuse() {
    let (mut sim, ids) = wing(1, true, &["fullstack"]);
    load(&mut sim, "ilmari1/2");
    let boss = unit_of(&sim, "il_recruiter").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut fuse_ms = 0;
    for _ in 0..12 * SECOND {
        for e in sim.tick() {
            if let Event::CastStarted {
                spell, total_ms, ..
            } = e
                && (spell.0 == 2430 || spell.0 == 2431)
            {
                fuse_ms = total_ms;
            }
        }
        sim.debug_set_hp(boss, 10_000_000);
    }
    let fakes = sim
        .units()
        .filter(|u| u.key == "il_candidate_fake" && u.alive)
        .count();
    assert_eq!(fakes, 1);
    assert_eq!(fuse_ms, 20_000);
}

/// Kills the first living unclaimed cable and returns how many cables live afterwards.
fn cables(sim: &Sim) -> usize {
    sim.units()
        .filter(|u| u.key == "il_cable" && u.alive)
        .count()
}

fn first_cable(sim: &Sim, labelled: bool) -> Option<EntityId> {
    sim.units()
        .find(|u| {
            u.key == "il_cable"
                && u.alive
                && u.auras.iter().any(|a| a.key == "labelled") == labelled
        })
        .map(|u| u.id)
}

fn hydra_ready(story: bool, classes: &[&str]) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids) = wing(1, story, classes);
    load(&mut sim, "ilmari1/3");
    let boss = unit_of(&sim, "il_cable_hydra").expect("boss");
    pull(&mut sim, &ids, boss);
    for _ in 0..4 * SECOND {
        sim.tick();
        sim.debug_set_hp(boss, 10_000_000);
    }
    assert!(cables(&sim) >= 1, "the hydra put out cables");
    (sim, ids, boss)
}

#[test]
fn a_cut_cable_grows_two_unless_it_is_labelled() {
    let (mut sim, _, _) = hydra_ready(false, &["fullstack"]);
    let before = cables(&sim);
    let c = first_cable(&sim, false).expect("cable");
    assert!(sim.debug_kill(c));
    sim.tick();
    assert_eq!(cables(&sim), before - 1 + 2, "two grew back");
    // A labelled one stays dead.
    let l = first_cable(&sim, false).expect("cable");
    assert!(sim.debug_apply_aura(l, "labelled", 600_000));
    let before = cables(&sim);
    assert!(sim.debug_kill(l));
    sim.tick();
    assert_eq!(cables(&sim), before - 1);
}

#[test]
fn story_cables_grow_one_and_the_cap_is_eight() {
    let (mut sim, _, _) = hydra_ready(true, &["fullstack"]);
    let before = cables(&sim);
    let c = first_cable(&sim, false).expect("cable");
    assert!(sim.debug_kill(c));
    sim.tick();
    assert_eq!(cables(&sim), before, "one grew back");
    let (mut sim, _, _) = hydra_ready(false, &["fullstack"]);
    for _ in 0..40 {
        if let Some(c) = first_cable(&sim, false) {
            sim.debug_kill(c);
        }
        sim.tick();
        assert!(cables(&sim) <= 8, "cap");
    }
}

#[test]
fn a_printer_labels_the_targeted_cable_and_devops_labels_two() {
    for (class, want) in [("fullstack", 1), ("devops", 2)] {
        let (mut sim, ids, _) = hydra_ready(false, &[class]);
        for _ in 0..6 * SECOND {
            sim.tick();
        }
        assert!(cables(&sim) >= 2, "{class}");
        let target = first_cable(&sim, false).expect("cable");
        sim.apply_as(
            ids[0],
            Command::SetTarget {
                unit: ids[0],
                target: Some(target),
            },
        );
        move_to(&mut sim, ids[0], Vec3::new(1.0, 0.0, 0.0));
        sim.apply_as(
            ids[0],
            Command::Interact {
                prop: "il_label_printer_1".into(),
            },
        );
        sim.tick();
        let labelled = sim
            .units()
            .filter(|u| u.key == "il_cable" && u.auras.iter().any(|a| a.key == "labelled"))
            .count();
        assert_eq!(labelled, want, "{class}");
        assert!(
            sim.unit(target)
                .is_some_and(|u| u.auras.iter().any(|a| a.key == "labelled")),
            "the targeted one first"
        );
    }
}

#[test]
fn the_wing_locks_per_difficulty_pays_tokens_and_drops_only_its_own_pieces() {
    let c = content();
    let raid = c.raid_of("ilmari1/2").expect("wing");
    assert!(raid.ilmari && !raid.is_dungeon());
    for (story, tokens) in [(true, 6), (false, 12)] {
        let (mut sim, ids) = wing(
            if story { 1 } else { 3 },
            story,
            &["fullstack", "backend", "frontend"],
        );
        load(&mut sim, "ilmari1/1");
        let boss = unit_of(&sim, "il_patch_tuesday").expect("boss");
        pull(&mut sim, &ids, boss);
        assert!(sim.debug_kill(boss));
        let mut pieces = Vec::new();
        for e in sim.tick() {
            if let Event::LootDropped { items, .. } = e {
                pieces.extend(items);
            }
        }
        assert!(sim.raid_locked("il_patch_tuesday", story));
        assert!(
            !sim.raid_locked("il_patch_tuesday", !story),
            "each difficulty locks apart"
        );
        let held = sim
            .inventory()
            .iter()
            .find(|(n, _)| n == combat_core::ilmari::TOKEN)
            .map_or(0, |(_, n)| *n);
        assert_eq!(held, tokens, "tokens, half on Story");
        for p in &pieces {
            let g = c
                .economy
                .gear(p)
                .unwrap_or_else(|| panic!("{p} is not gear"));
            assert_eq!(g.source, GearSource::Ilmari, "{p}");
            assert_eq!(g.drops_from.as_deref(), Some("il_patch_tuesday"), "{p}");
        }
        if !story {
            assert!(
                pieces.len() >= 2,
                "Full rolls two pieces per member: {pieces:?}"
            );
        }
    }
}

#[test]
fn the_wing_door_waits_for_the_golem_and_the_group_size_picks_the_default_difficulty() {
    let c = content();
    let door = c
        .zone("campus_occupied")
        .and_then(|z| z.links.iter().find(|l| l.to == "ilmari1/1"))
        .expect("door");
    assert!(matches!(&door.requires, Some(Unlock::QuestDone(q)) if q == "il_04_wing1_attune"));
    let mut sim = Sim::new(content(), 3);
    sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim.raid_view().story_next, "alone: Story");
    sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(!sim.raid_view().story_next, "three: Full");
    let lead = sim.players()[0];
    sim.apply_as(lead, Command::SetIlmariStory { story: true });
    sim.tick();
    assert!(
        sim.raid_view().story_next,
        "the lead can pick Story for the trio"
    );
}
