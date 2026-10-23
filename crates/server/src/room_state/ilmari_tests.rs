//! Ilmari in a co-op room (docs/ILMARI_09.md B.8, B16): the room's own sim decides what is
//! open, and what a client may claim about it is a stamp, a reveal and the lead's difficulty.
//! Child of `room_state` so it can reach the room's private fields.

use super::*;
use combat_core::{RaidView, SpellId};
use net_proto::decode;

fn room() -> RoomState {
    RoomState::new("TECH-DEBT-00000000".into(), World::embedded().unwrap())
}

fn eid_of(s: &RoomState, p: PlayerId) -> EntityId {
    s.members[&p].eid
}

fn msgs(out: &[(Audience, Bytes)], to: PlayerId) -> Vec<ServerMsg> {
    out.iter()
        .filter(|(a, _)| match a {
            Audience::All => true,
            Audience::Only(p) => *p == to,
        })
        .map(|(_, b)| decode::<ServerMsg>(b).unwrap())
        .collect()
}

fn events(out: &[(Audience, Bytes)], to: PlayerId) -> Vec<Event> {
    msgs(out, to)
        .into_iter()
        .flat_map(|m| match m {
            ServerMsg::Events { events, .. } => events,
            _ => Vec::new(),
        })
        .collect()
}

fn pending_msgs(out: &[(Audience, Bytes)], to: PlayerId) -> Vec<bool> {
    msgs(out, to)
        .into_iter()
        .filter_map(|m| match m {
            ServerMsg::IlmariPending { on } => Some(on),
            _ => None,
        })
        .collect()
}

fn raid_view(out: &[(Audience, Bytes)], to: PlayerId) -> Option<RaidView> {
    events(out, to).into_iter().rev().find_map(|e| match e {
        Event::Raid { view } => Some(view),
        _ => None,
    })
}

fn stamp_now(s: &RoomState) -> Command {
    Command::StampIlmari {
        unix_secs: s.sim.unix_s(),
        utc_offset_min: 120,
        tz: "CEST".into(),
    }
}

fn has_ilmari_unit(s: &RoomState) -> bool {
    s.sim.units().any(|u| u.key.starts_with("il_"))
}

fn opens_resistance(s: &RoomState) -> bool {
    s.sim
        .zone_links("parking_p3")
        .iter()
        .any(|l| l.to == "resistance/1" && l.unlocked)
}

/// The group's AGI is down, as after the real fight; nothing else changes.
fn credits_rolled(s: &mut RoomState) {
    s.sim.debug_ilmari_opening();
}

#[tokio::test(start_paused = true)]
async fn a_room_before_the_credits_has_no_ilmari_whatever_a_client_sends() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    let out = s.tick();
    assert!(pending_msgs(&out, 0).is_empty());
    let (a, now) = (eid_of(&s, 0), s.sim.unix_s());
    let _ = a;
    for cmd in [
        stamp_now(&s),
        Command::IlmariReveal,
        Command::StampIlmari {
            unix_secs: now,
            utc_offset_min: 0,
            tz: "UTC".into(),
        },
        Command::IlmariReveal,
    ] {
        s.on_message(0, ClientMsg::Cmd(cmd));
    }
    let out = s.tick();
    assert!(pending_msgs(&out, 0).is_empty());
    assert!(!s.sim.ilmari_pending() && !s.sim.ilmari_open());
    assert!(s.sim.ilmari_stamp().is_none());
    assert!(!opens_resistance(&s));
    // The occupied campus and the base stay shut and empty.
    for zone in ["campus", "parking_p3"] {
        s.world.load(&mut s.sim, zone, None).unwrap();
        assert!(!has_ilmari_unit(&s), "{zone}");
    }
    let doors: Vec<_> = s.sim.zone_links("campus").into_iter().collect();
    assert!(
        doors
            .iter()
            .all(|l| !l.unlocked || !combat_core::ilmari::is_ilmari_zone(&l.to)),
        "{doors:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_rooms_opening_is_announced_stamped_once_and_revealed_to_everyone() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    credits_rolled(&mut s);
    let out = s.tick();
    assert_eq!(pending_msgs(&out, 0), [true]);
    assert_eq!(pending_msgs(&out, 1), [true]);
    // A reveal needs a stamp first, and the stamp has to come from this decade's clock.
    s.on_message(1, ClientMsg::Cmd(Command::IlmariReveal));
    s.on_message(
        1,
        ClientMsg::Cmd(Command::StampIlmari {
            unix_secs: s.sim.unix_s() + 30 * 24 * 3600,
            utc_offset_min: 0,
            tz: "UTC".into(),
        }),
    );
    s.tick();
    assert!(s.sim.ilmari_pending() && s.sim.ilmari_stamp().is_none());
    // The first stamp counts, a later one (a hand-edited clock) does not move it.
    let first = stamp_now(&s);
    s.on_message(0, ClientMsg::Cmd(first));
    s.on_message(
        1,
        ClientMsg::Cmd(Command::StampIlmari {
            unix_secs: s.sim.unix_s() + 3600,
            utc_offset_min: -300,
            tz: "<script>EST".into(),
        }),
    );
    s.tick();
    let line = s.sim.ilmari_stamp().unwrap().line();
    assert!(line.contains("CEST"), "{line}");
    // Only the lead ends everyone's Opening while it still has time to.
    s.on_message(1, ClientMsg::Cmd(Command::IlmariReveal));
    s.tick();
    assert!(
        s.sim.ilmari_pending(),
        "a member revealed before the lead's grace ran out"
    );
    s.on_message(0, ClientMsg::Cmd(Command::IlmariReveal));
    let out = s.tick();
    assert!(s.sim.ilmari_open() && !s.sim.ilmari_pending());
    assert_eq!(pending_msgs(&out, 0), [false]);
    assert_eq!(pending_msgs(&out, 1), [false]);
    for p in [0, 1] {
        let e = eid_of(&s, p);
        assert!(
            s.sim
                .quests_of(e)
                .completed
                .iter()
                .any(|q| q == "il_00_wake"),
            "member {p}"
        );
    }
    assert!(opens_resistance(&s));
}

#[tokio::test(start_paused = true)]
async fn a_fresh_player_joining_a_revealed_room_is_past_the_opening_and_stays_so_without_the_lead()
{
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.tick();
    credits_rolled(&mut s);
    let st = stamp_now(&s);
    s.on_message(0, ClientMsg::Cmd(st));
    s.on_message(0, ClientMsg::Cmd(Command::IlmariReveal));
    s.tick();
    assert!(s.sim.ilmari_open());
    s.on_join(1, "Fresh", "devops");
    let out = s.tick();
    let fresh = eid_of(&s, 1);
    assert!(
        s.sim
            .quests_of(fresh)
            .completed
            .iter()
            .any(|q| q == "il_00_wake")
    );
    assert!(pending_msgs(&out, 1).is_empty(), "nothing to open anymore");
    // The Welcome's log carries it, and the hatch outlives the member who watched the Opening.
    let (quests, unlocked) = msgs(&out, 1)
        .into_iter()
        .find_map(|m| match m {
            ServerMsg::Welcome {
                quests, unlocked, ..
            } => Some((quests, unlocked)),
            _ => None,
        })
        .unwrap();
    assert!(quests.completed.iter().any(|q| q == "il_00_wake"));
    assert!(unlocked.iter().any(|z| z == "resistance/1"), "{unlocked:?}");
    s.on_leave(0);
    assert!(opens_resistance(&s), "the door belongs to the room");
    // And a joiner after the credits but before the reveal gets the announcement.
    let mut t = room();
    credits_rolled(&mut t);
    t.on_join(0, "Late", "backend");
    let out = t.tick();
    assert_eq!(pending_msgs(&out, 0).first(), Some(&true));
}

#[tokio::test(start_paused = true)]
async fn the_occupied_campus_the_base_and_the_wings_load_in_a_room_and_phases_are_per_room() {
    let mut a = room();
    a.on_join(0, "Lead", "backend");
    a.tick();
    credits_rolled(&mut a);
    let st = stamp_now(&a);
    a.on_message(0, ClientMsg::Cmd(st));
    a.on_message(0, ClientMsg::Cmd(Command::IlmariReveal));
    a.tick();
    for zone in [
        "resistance/1",
        "campus_occupied",
        "ilmari1/1",
        "ilmari1/2",
        "ilmari1/3",
        "ilmari2/1",
        "ilmari2/2",
        "ilmari2/3",
    ] {
        a.world.load(&mut a.sim, zone, None).unwrap();
        assert_eq!(a.sim.zone(), Some(zone));
        a.tick();
    }
    // Walking through the hatch is the sim's door logic, which the server follows.
    a.world.load(&mut a.sim, "parking_p3", None).unwrap();
    assert!(a.sim.zone_links("resistance/1").iter().any(|l| l.unlocked));
    // The next room never learned any of it.
    let mut b = room();
    b.on_join(0, "Other", "backend");
    b.tick();
    assert!(!b.sim.ilmari_open() && !opens_resistance(&b));
    b.world.load(&mut b.sim, "campus", None).unwrap();
    assert!(!has_ilmari_unit(&b));
}

#[tokio::test(start_paused = true)]
async fn only_the_lead_picks_story_and_the_pick_reaches_everyone_and_late_joiners() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    // Two players: the wings default to Story (below the Full group size).
    assert!(s.sim.raid_view().story_next);
    s.on_message(1, ClientMsg::Cmd(Command::SetIlmariStory { story: false }));
    s.tick();
    assert!(s.sim.raid_view().story_next, "a member is not the lead");
    s.on_message(0, ClientMsg::Cmd(Command::SetIlmariStory { story: false }));
    let out = s.tick();
    assert!(!s.sim.raid_view().story_next);
    for p in [0, 1] {
        assert_eq!(
            raid_view(&out, p).map(|v| v.story_next),
            Some(false),
            "member {p}"
        );
    }
    s.on_join(2, "Late", "devops");
    let out = s.tick();
    assert_eq!(raid_view(&out, 2).map(|v| v.story_next), Some(false));
    // The pick stays the room's, and the next lead can change it.
    s.on_leave(0);
    s.on_message(1, ClientMsg::Cmd(Command::SetIlmariStory { story: true }));
    s.tick();
    assert!(s.sim.raid_view().story_next);
}

#[tokio::test(start_paused = true)]
async fn wing_lockouts_per_difficulty_belong_to_the_room_and_reach_late_joiners() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.tick();
    s.sim.debug_set_raid_lockout(&[
        "story:il_recruiter".to_string(),
        "il_cable_hydra".to_string(),
    ]);
    s.on_leave(0);
    s.on_join(1, "Late", "devops");
    let out = s.tick();
    let view = raid_view(&out, 1).unwrap();
    for key in ["story:il_recruiter", "il_cable_hydra"] {
        assert!(view.locked.iter().any(|k| k == key), "{key} in {view:?}");
    }
}

/// A Committee pull with the vote open and two members standing where the terminals are.
fn committee_vote(s: &mut RoomState) -> (EntityId, EntityId) {
    for p in [0, 1] {
        let e = eid_of(s, p);
        s.sim.set_level_of(e, 10);
    }
    s.sim.debug_force_raid_async(Some(false));
    s.world.load(&mut s.sim, "ilmari2/2", None).unwrap();
    let (a, b) = (eid_of(s, 0), eid_of(s, 1));
    for m in s.members.values_mut() {
        if let Some(u) = s.sim.unit(m.eid) {
            m.resync(u.pos);
        }
    }
    let boss = s.sim.units().find(|u| u.key == "il_cc_cpo").unwrap().id;
    let bp = s.sim.unit(boss).unwrap().pos;
    let near = Vec3::new(bp.x, bp.y, bp.z + 8.0);
    for e in [a, b] {
        move_sim(s, e, near);
    }
    s.sim.apply_as(
        a,
        Command::SetTarget {
            unit: a,
            target: Some(boss),
        },
    );
    s.sim.apply_as(
        a,
        Command::Cast {
            caster: a,
            spell: SpellId(1),
        },
    );
    for _ in 0..60 {
        s.tick();
        for k in ["il_cc_cpo", "il_cc_culture", "il_cc_vibes"] {
            let ids: Vec<_> = s.sim.units().filter(|u| u.key == k).map(|u| u.id).collect();
            for id in ids {
                s.sim.debug_set_hp(id, 10_000_000);
            }
        }
        for e in [a, b] {
            s.sim.debug_set_hp(e, 10_000_000);
        }
    }
    (a, b)
}

fn move_sim(s: &mut RoomState, e: EntityId, pos: Vec3) {
    s.sim.apply_as(
        e,
        Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
}

#[tokio::test(start_paused = true)]
async fn ballot_terminals_count_only_from_beside_them_and_only_while_a_vote_is_open() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    // No Committee, no vote: a terminal press is an interaction with nothing.
    s.on_message(
        0,
        ClientMsg::Cmd(Command::Interact {
            prop: "il_ballot_a".into(),
        }),
    );
    s.tick();
    assert!(s.sim.raid_view().ballots.is_empty());
    let (a, b) = committee_vote(&mut s);
    let view = s.sim.raid_view();
    assert_eq!(view.vote.len(), 3, "{view:?}");
    assert_eq!(view.ballots, [0, 0, 0]);
    let at = |s: &RoomState, key: &str| {
        s.world
            .zone("ilmari2/2")
            .and_then(|g| g.anchor(&format!("prop:{key}")))
            .map(|(p, _)| p)
            .unwrap()
    };
    // Out of reach: nothing counts. Garbage and oversized names never get past the wire.
    let c = at(&s, "il_ballot_c");
    move_sim(&mut s, a, Vec3::new(c.x + 40.0, c.y, c.z));
    for prop in ["il_ballot_c", "il_ballot_zz", "ballot/../../c"] {
        s.on_message(0, ClientMsg::Cmd(Command::Interact { prop: prop.into() }));
    }
    s.tick();
    assert_eq!(s.sim.raid_view().ballots, [0, 0, 0]);
    let long = ClientMsg::Cmd(Command::Interact {
        prop: "b".repeat(200),
    });
    assert!(decode::<ClientMsg>(&encode(&long).unwrap()).is_err());
    // Beside terminal C it counts once; the cooldown eats a held key; B is a separate voter.
    let (pc, pa) = (at(&s, "il_ballot_c"), at(&s, "il_ballot_a"));
    move_sim(&mut s, a, pc);
    move_sim(&mut s, b, pa);
    let mut out = Vec::new();
    for _ in 0..3 {
        s.on_message(
            0,
            ClientMsg::Cmd(Command::Interact {
                prop: "il_ballot_c".into(),
            }),
        );
        out.extend(s.tick());
    }
    s.on_message(
        1,
        ClientMsg::Cmd(Command::Interact {
            prop: "il_ballot_a".into(),
        }),
    );
    out.extend(s.tick());
    assert_eq!(s.sim.raid_view().ballots, [1, 0, 1]);
    // The updated board went to both members.
    for p in [0, 1] {
        assert_eq!(raid_view(&out, p).map(|v| v.ballots), Some(vec![1, 0, 1]));
    }
}

#[tokio::test(start_paused = true)]
async fn label_printers_answer_only_to_a_member_standing_at_them() {
    let mut s = room();
    s.on_join(0, "Lead", "devops");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let (a, b) = (eid_of(&s, 0), eid_of(&s, 1));
    s.world.load(&mut s.sim, "ilmari1/3", None).unwrap();
    let printer = s
        .sim
        .content()
        .world
        .props
        .iter()
        .find(|p| p.kind == combat_core::PropKind::Label && p.zone == "ilmari1/3")
        .map(|p| p.key.clone())
        .unwrap();
    let spot = s
        .world
        .zone("ilmari1/3")
        .and_then(|g| g.anchor(&format!("prop:{printer}")))
        .map(|(p, _)| p)
        .unwrap();
    let used = |out: &[(Audience, Bytes)], who: EntityId| {
        events(out, 0)
            .into_iter()
            .chain(events(out, 1))
            .any(|e| matches!(e, Event::Interacted { unit, .. } if unit == who))
    };
    move_sim(&mut s, b, Vec3::new(spot.x + 40.0, spot.y, spot.z));
    s.on_message(
        1,
        ClientMsg::Cmd(Command::Interact {
            prop: printer.clone(),
        }),
    );
    assert!(!used(&s.tick(), b), "out of reach");
    move_sim(&mut s, a, spot);
    s.on_message(
        1,
        ClientMsg::Cmd(Command::Interact {
            prop: printer.clone(),
        }),
    );
    assert!(
        !used(&s.tick(), b),
        "the sender is the actor, not whoever stands there"
    );
    s.on_message(0, ClientMsg::Cmd(Command::Interact { prop: printer }));
    assert!(used(&s.tick(), a), "beside it, the press lands");
}

#[tokio::test(start_paused = true)]
async fn the_weekly_token_cap_is_each_players_own() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let (a, b) = (eid_of(&s, 0), eid_of(&s, 1));
    let cap = combat_core::ilmari::TOKEN_WEEK_CAP;
    assert_eq!(s.sim.grant_ilmari_tokens(a, 90), 90);
    assert_eq!(s.sim.ilmari_tokens_left_of(a), cap - 90);
    assert_eq!(
        s.sim.ilmari_tokens_left_of(b),
        cap,
        "A's pay is not B's allowance"
    );
    assert_eq!(s.sim.grant_ilmari_tokens(b, 60), 60);
    assert_eq!(s.sim.ilmari_tokens_left_of(a), cap - 90);
    assert_eq!(s.sim.ilmari_tokens_left_of(b), cap - 60);
}

/// A member cannot stamp or reveal for the room while the lead still has time, but an AFK
/// lead does not block the room for ever: after the grace any member may finish the Opening.
#[tokio::test]
async fn an_afk_lead_hands_the_opening_to_the_room_after_the_grace() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    credits_rolled(&mut s);
    s.tick();
    s.on_message(1, ClientMsg::Cmd(stamp_now(&s)));
    s.tick();
    assert!(
        s.sim.ilmari_stamp().is_none(),
        "a member stamped inside the lead's grace"
    );
    // Two minutes of an idle lead (30 ticks a second).
    for _ in 0..(121 * 30) {
        s.tick();
    }
    s.on_message(1, ClientMsg::Cmd(stamp_now(&s)));
    s.on_message(1, ClientMsg::Cmd(Command::IlmariReveal));
    s.tick();
    assert!(
        s.sim.ilmari_open(),
        "the room stayed stuck behind an idle lead"
    );
}

/// Companions in a room (docs/ILMARI_09.md B.7): the sheet is plain `Talk`, a wing on Story brings
/// each member's pair, the room stops at two whoever asked, and Full gets none.
#[tokio::test(start_paused = true)]
async fn companions_belong_to_their_owner_and_a_room_stops_at_two() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let (a, b) = (eid_of(&s, 0), eid_of(&s, 1));
    s.sim.debug_force_raid_async(Some(true));
    s.world.load(&mut s.sim, "resistance/1", None).unwrap();
    let sheet = s.sim.units().find(|u| u.key == "il_roster").unwrap();
    let (sheet, at) = (sheet.id, sheet.pos);
    for (p, e) in [(0, a), (1, b)] {
        move_sim(&mut s, e, Vec3::new(at.x, at.y, at.z + 1.0));
        s.on_message(p, ClientMsg::Cmd(Command::Talk { unit: sheet }));
        s.tick();
    }
    assert_eq!(s.sim.companion_picks(a).len(), 2);
    assert_eq!(s.sim.companion_picks(b).len(), 2);

    s.world.load(&mut s.sim, "ilmari1/1", None).unwrap();
    let (mine, theirs) = (s.sim.companions_of(a), s.sim.companions_of(b));
    assert_eq!((mine.len(), theirs.len()), (2, 0), "join order, cap two");

    s.sim.debug_force_raid_async(Some(false));
    s.world.load(&mut s.sim, "ilmari1/2", None).unwrap();
    assert!(s.sim.companions_of(a).is_empty(), "Full has none");
}

/// Illusions in a room (docs/ILMARI_10.md C.3.2): both members get the same public salt, each
/// derives a different fake layout from it, and damage follows the truth whatever either believes.
#[tokio::test(start_paused = true)]
async fn illusions_are_cosmetic_in_a_room_and_a_joiner_learns_the_running_cycle() {
    use combat_core::illusion::{IllusionArena, layout};
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let (a, b) = (eid_of(&s, 0), eid_of(&s, 1));
    s.world.load(&mut s.sim, "ilmari1/1", None).unwrap();
    let source = s.sim.units().find(|u| u.hostile).map(|u| u.id).unwrap();
    let arena = IllusionArena {
        x_cm: 0,
        z_cm: 0,
        tile_cm: 400,
    };
    let cycle = s.sim.illusion_begin(arena, source, 100, 1000);
    let (_, salt, _) = s.sim.illusion_cycle().unwrap();
    let out = s.tick();
    for p in [0, 1] {
        let got = events(&out, p).into_iter().find_map(|e| match e {
            Event::IllusionCycle { cycle, salt, .. } => Some((cycle, salt)),
            _ => None,
        });
        assert_eq!(got, Some((cycle, salt)), "member {p} got a different draw");
    }
    let (la, lb) = (
        layout(salt, a, cycle, &arena),
        layout(salt, b, cycle, &arena),
    );
    assert_ne!(la, lb, "two members were shown the same illusion");
    assert_eq!(la, layout(salt, a, cycle, &arena), "layout is not pure");

    // Standing on a real overflow tile hurts both, whatever each believes is safe.
    let real = s.sim.illusion_real().unwrap();
    let k = (0..9).find(|k| real.overflow >> k & 1 == 1).unwrap();
    let (cx, cz) = arena.tile_center(k);
    let spot = s.sim.unit(a).unwrap().pos;
    let pos = Vec3::new(cx as f32 / 100.0, spot.y, cz as f32 / 100.0);
    let (hp_a, hp_b) = (s.sim.unit(a).unwrap().hp, s.sim.unit(b).unwrap().hp);
    for e in [a, b] {
        move_sim(&mut s, e, pos);
    }
    for _ in 0..40 {
        s.tick();
    }
    assert!(s.sim.unit(a).unwrap().hp < hp_a && s.sim.unit(b).unwrap().hp < hp_b);

    // A late joiner is told the cycle that is already running.
    s.sim.illusion_begin(arena, source, 100, 60_000);
    s.on_join(2, "Late", "devops");
    let out = s.tick();
    assert!(
        events(&out, 2)
            .iter()
            .any(|e| matches!(e, Event::IllusionCycle { .. })),
        "the joiner never saw the cycle"
    );
}

// ---- Wing 3, Training Data (docs/ILMARI_10.md C.4): the room's sim owns the truth.

/// Two members in `zone` at level 10 on Story (Full needs three), pulled onto its boss; returns the boss id.
fn pull_wing3(s: &mut RoomState, zone: &str, boss_key: &str) -> EntityId {
    for p in [0, 1] {
        let e = eid_of(s, p);
        s.sim.set_level_of(e, 10);
    }
    s.sim.debug_force_raid_async(Some(true));
    s.world.load(&mut s.sim, zone, None).unwrap();
    for m in s.members.values_mut() {
        if let Some(u) = s.sim.unit(m.eid) {
            m.resync(u.pos);
        }
    }
    let (a, b) = (eid_of(s, 0), eid_of(s, 1));
    let boss = s.sim.units().find(|u| u.key == boss_key).unwrap().id;
    let bp = s.sim.unit(boss).unwrap().pos;
    for e in [a, b] {
        move_sim(s, e, Vec3::new(bp.x, bp.y, bp.z + 8.0));
    }
    s.sim.apply_as(
        a,
        Command::SetTarget {
            unit: a,
            target: Some(boss),
        },
    );
    s.sim.apply_as(
        a,
        Command::Cast {
            caster: a,
            spell: SpellId(1),
        },
    );
    boss
}

fn keep_alive(s: &mut RoomState, boss: EntityId) {
    s.sim.debug_set_hp(boss, 10_000_000);
    for p in [0, 1] {
        let e = eid_of(s, p);
        s.sim.debug_set_hp(e, 10_000_000);
    }
}

#[tokio::test(start_paused = true)]
async fn the_scraper_queens_drafts_reach_every_member_and_a_late_joiner() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let boss = pull_wing3(&mut s, "ilmari3/1", "il_scraper_queen");
    let mut out = Vec::new();
    for _ in 0..300 {
        out.extend(s.tick());
        keep_alive(&mut s, boss);
    }
    for p in [0, 1] {
        let sent = events(&out, p)
            .into_iter()
            .filter_map(|e| match e {
                Event::Drafts { integrity, marked } => Some((integrity.len(), marked.len())),
                _ => None,
            })
            .next_back();
        assert_eq!(sent, Some((5, 5)), "member {p} sees five drafts");
    }
    assert_eq!(s.sim.drafts().len(), 5);
    assert!(
        !s.sim.swarm_clusters().is_empty(),
        "the swarm is in the room"
    );
    // A third player walking in mid-fight is told about the drafts without waiting for a change.
    s.on_join(2, "Late", "devops");
    let mut late = Vec::new();
    for _ in 0..60 {
        late.extend(s.tick());
        keep_alive(&mut s, boss);
    }
    assert!(
        events(&late, 2)
            .iter()
            .any(|e| matches!(e, Event::Drafts { integrity, .. } if integrity.len() == 5)),
        "the joiner got the drafts"
    );
}

#[tokio::test(start_paused = true)]
async fn the_hallucinations_cycle_gives_everyone_one_salt_and_nobody_the_fakes() {
    let mut s = room();
    s.on_join(0, "Lead", "backend");
    s.on_join(1, "Dps", "fullstack");
    s.tick();
    let boss = pull_wing3(&mut s, "ilmari3/2", "il_hallucination");
    let mut out = Vec::new();
    for _ in 0..30 * 12 {
        out.extend(s.tick());
        keep_alive(&mut s, boss);
    }
    let cycle_of = |p: PlayerId| {
        events(&out, p).into_iter().find_map(|e| match e {
            Event::IllusionCycle { cycle, salt, arena } => Some((cycle, salt, arena)),
            _ => None,
        })
    };
    let (a, b) = (cycle_of(0), cycle_of(1));
    assert!(a.is_some(), "a cycle opened within 12 s");
    assert_eq!(
        a, b,
        "one cycle, one salt: the fakes are derived per player on the client"
    );
    let (cycle, salt, arena) = a.unwrap();
    let (ea, eb) = (eid_of(&s, 0), eid_of(&s, 1));
    assert_ne!(
        combat_core::illusion::layout(salt, ea, cycle, &arena),
        combat_core::illusion::layout(salt, eb, cycle, &arena)
    );
    // The wire never carries a fake: the only illusion event is the cycle itself.
    assert!(
        events(&out, 0)
            .iter()
            .all(|e| !format!("{e:?}").contains("fake")),
        "no fake on the wire"
    );
}

#[tokio::test(start_paused = true)]
async fn wing_three_loot_stays_in_the_ilmari_band_of_the_wings_before_it() {
    let s = room();
    let content = s.sim.content();
    let top = |zone: &str| {
        let bosses = content
            .raid_of(zone)
            .map(|r| r.bosses(content))
            .unwrap_or_default();
        content
            .economy
            .gear
            .iter()
            .filter(|g| {
                g.source == combat_core::GearSource::Ilmari
                    && g.drops_from
                        .as_deref()
                        .is_some_and(|b| bosses.iter().any(|k| k == b))
            })
            .map(|g| g.ilvl)
            .max()
            .unwrap_or(0)
    };
    assert!(top("ilmari3/1") > 0 && top("ilmari3/1") <= top("ilmari2/1"));
}
