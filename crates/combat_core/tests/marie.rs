//! Ill Marie (docs/ILMARI_09.md Part A): spawn gates, "one at a time", the annoyance budget of
//! her lines, the hex trail and the acquaintance mask. Flavour only, so none of it may touch
//! combat, XP or loot.

mod common;

use combat_core::{Command, Content, EntityId, Sim, Unlock, Vec3};
use common::content;

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let far = name.starts_with("pack_") || name.starts_with("rare_") || name == "boss";
    let at = if far { 400.0 } else { 0.0 };
    Some((Vec3::new(at, 0.0, at), 0.0))
}

fn world(zone: &str, level: u8, bosses: &[&str]) -> (Sim, EntityId) {
    let mut sim = Sim::new(content(), 21);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, level);
    for b in bosses {
        sim.debug_mark_boss(b);
    }
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn maries(sim: &Sim) -> Vec<String> {
    sim.units()
        .filter(|u| u.key.starts_with("ill_marie") && u.alive)
        .map(|u| u.key.clone())
        .collect()
}

#[test]
fn marie_spawn_gates_follow_level_and_bosses() {
    let desk = vec!["ill_marie".to_string()];
    assert_eq!(maries(&world("campus", 1, &[]).0), desk);
    assert_eq!(
        maries(&world("campus", 5, &[]).0),
        ["ill_marie", "ill_marie_queue"]
    );
    // The queue stays until her Cough Drops errand is handed in, so the ticket never strands.
    assert_eq!(maries(&world("campus", 9, &[]).0).len(), 2);
    // The bench is the post-credits beat and never shows before the AGI is dead.
    assert_eq!(
        maries(&world("campus", 10, &["agi_prototype"]).0),
        ["ill_marie", "ill_marie_queue", "ill_marie_bench"]
    );
    // The two dungeon cameos wait for their boss.
    assert!(maries(&world("onboarding/3", 4, &[]).0).is_empty());
    assert_eq!(
        maries(&world("onboarding/3", 4, &["onboarding_buddy"]).0),
        ["ill_marie_onboarding"]
    );
    assert!(maries(&world("basement/4", 7, &[]).0).is_empty());
    assert_eq!(
        maries(&world("basement/4", 7, &["big_bang_cutover"]).0),
        ["ill_marie_oncall"]
    );
    // Ungated beats.
    for (zone, key) in [
        ("parking_p3", "ill_marie_p3"),
        ("tower/2", "ill_marie_call"),
        ("datacenter/1", "ill_marie_racks"),
        ("home/1", "ill_marie_home"),
    ] {
        assert_eq!(maries(&world(zone, 3, &[]).0), [key], "{zone}");
    }
}

/// One Marie per zone, except the campus, where the desk, the queue and the bench are far
/// enough apart that no screen holds two (the layout test measures the distance).
#[test]
fn marie_never_stacks() {
    let c = content();
    let all_bosses = ["onboarding_buddy", "big_bang_cutover", "agi_prototype"];
    for z in c
        .zones
        .iter()
        .filter(|z| z.friendly.iter().any(|f| f.unit.starts_with("ill_marie")))
    {
        for level in 1..=10 {
            for bosses in [&[][..], &all_bosses[..]] {
                let n = maries(&world(&z.id, level, bosses).0).len();
                let max = if z.id == "campus" { 3 } else { 1 };
                assert!(n <= max, "{} at level {level}: {n} Maries", z.id);
            }
        }
    }
}

#[test]
fn marie_no_new_aggro() {
    let c = content();
    let units: Vec<_> = c
        .units
        .iter()
        .filter(|u| u.key.starts_with("ill_marie"))
        .collect();
    assert_eq!(units.len(), 9, "the desk plus eight appearances");
    for u in units {
        assert!(
            !u.hostile && u.ai.is_none() && u.auto_attack.is_none(),
            "{}",
            u.key
        );
        assert_eq!(u.name, "Ill Marie", "{}", u.key);
        assert_eq!(u.xp_reward, 0, "{}", u.key);
        assert!(u.loot.is_empty(), "{}", u.key);
    }
}

fn marie_texts(c: &Content) -> Vec<String> {
    let mut out = Vec::new();
    for u in c.units.iter().filter(|u| u.key.starts_with("ill_marie")) {
        out.extend(u.yells.talk.iter().cloned());
        out.extend(u.yells.ambient.iter().cloned());
    }
    for (k, lines) in &c.live.night.ambient {
        if k.starts_with("ill_marie") {
            out.extend(lines.iter().cloned());
        }
    }
    for q in c
        .quests
        .iter()
        .filter(|q| q.giver.npc.starts_with("ill_marie"))
    {
        out.extend([q.text.clone(), q.progress_text.clone(), q.done_text.clone()]);
    }
    out
}

/// "0x57 0x41 0x4B 0x45" runs in a line, decoded to ASCII; `None` when a token is not printable.
fn hex_words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for tok in line.split_whitespace() {
        let tok = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        match tok.strip_prefix("0x") {
            Some(h) => {
                let b = u8::from_str_radix(h, 16).ok()?;
                if !(0x20..0x7f).contains(&b) {
                    return None;
                }
                cur.push(b as char);
            }
            None if !cur.is_empty() => words.push(std::mem::take(&mut cur)),
            None => {}
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    Some(words)
}

#[test]
fn marie_lines_are_tidy() {
    let c = content();
    for u in c.units.iter().filter(|u| u.key.starts_with("ill_marie")) {
        // No chatter or at least a minute apart, so a bubble is an event, not a stream.
        assert!(
            u.yells.ambient_ms == 0 || u.yells.ambient_ms >= 55_000,
            "{}: ambient every {} ms",
            u.key,
            u.yells.ambient_ms
        );
        assert!(!u.yells.talk.is_empty(), "{} has nothing to say", u.key);
    }
    let mut trail: Vec<String> = Vec::new();
    for t in marie_texts(&c) {
        assert!(
            !t.to_lowercase().contains("ilmari"),
            "a spoiler before the credits: {t}"
        );
        let Some(words) = hex_words(&t) else {
            panic!("hex that is not printable ASCII: {t}");
        };
        trail.extend(words);
    }
    trail.sort();
    trail.dedup();
    // The sentence over the journey (A.4); the desk's LEAK and AUTO come from the existing lines.
    assert_eq!(
        trail,
        ["AUTO", "BODY", "LEAK", "NODE", "ONLINE", "PILOT", "WAKE"]
    );
}

#[test]
fn marie_gates_only_use_existing_progress() {
    let c = content();
    // `requires`/`until` are validated at load; the level gate has to be a real level.
    let gated: Vec<_> = c
        .zones
        .iter()
        .flat_map(|z| z.friendly.iter())
        .filter(|f| f.requires.is_some() || f.until.is_some())
        .collect();
    assert_eq!(gated.len(), 4);
    assert!(gated.iter().all(|f| f.unit.starts_with("ill_marie")));
    assert!(matches!(gated[0].requires, Some(Unlock::PlayerLevel(5))));
}

#[test]
fn a_spawn_gate_on_an_unknown_boss_or_quest_is_rejected() {
    let gate = r#"requires: Some(BossKilled("agi_prototype"))),"#;
    assert!(common::SRC.contains(gate));
    for bad in [
        r#"requires: Some(BossKilled("no_such_boss"))),"#,
        r#"until: Some(QuestDone("no_such_quest"))),"#,
    ] {
        assert!(
            Content::from_ron(&common::SRC.replace(gate, bad)).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn talking_to_marie_is_remembered_in_the_save_and_changes_nothing_else() {
    let (mut sim, p) = world("parking_p3", 1, &[]);
    assert_eq!(sim.marie_met(p), 0);
    let before = sim.export_save(p).unwrap();
    let marie = sim
        .units()
        .find(|u| u.key == "ill_marie_p3")
        .map(|u| u.id)
        .unwrap();
    // She is at the origin like the player; no walking needed.
    sim.apply_as(p, Command::Talk { unit: marie });
    for _ in 0..3 {
        sim.tick();
    }
    assert_eq!(sim.marie_met(p), 1 << 0);
    let mut after = sim.export_save(p).unwrap();
    assert_eq!(after.marie_met, 1);
    // Everything but the mask is as it was: no XP, no loot, no kudos.
    after.marie_met = 0;
    assert_eq!(before, after);

    let mut save = sim.export_save(p).unwrap();
    save.marie_met = 0b1_0101;
    let back = combat_core::SaveData::decode(&save.encode()).unwrap();
    assert_eq!(back.marie_met, 0b1_0101);
    // Older saves have no line and load as no acquaintances.
    let old = save.encode().replace("marie_met=21\n", "");
    assert_eq!(combat_core::SaveData::decode(&old).unwrap().marie_met, 0);
}
