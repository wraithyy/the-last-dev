//! In-process end-to-end check of the online client path: a fake server loop (combat_core +
//! net_proto, no sockets) drives the same decode -> `NetWorld` path the session uses, and the
//! client's picture is compared with the server's sim.

use super::lobby::Intent;
use super::session::{Outcome, Phase, Step, advance, hello_frames};
use super::world::{INTERP_DELAY_S, NetWorld};
use crate::collision::to_core;
use crate::combat::{SimRead, new_sim};
use crate::floors::build_zone;
use combat_core::{Audience, Command, EntityId, QuestStatus, Sim, Vec3};
use net_proto::{
    AuraSnap, ClientMsg, Msg, PlayerInfo, PlayerSnap, QuestSnap, ServerMsg, UnitSnap, decode,
    encode, pos_cm, quantize_yaw, room_code_from_bytes,
};

fn wire<M: Msg + std::fmt::Debug>(m: &M) -> M {
    match encode(m).map(|b| decode::<M>(&b)) {
        Ok(Ok(m)) => m,
        other => panic!("{m:?} did not survive the wire: {other:?}"),
    }
}

fn snapshot(sim: &Sim) -> ServerMsg {
    ServerMsg::Snapshot {
        tick: sim.now().0,
        units: sim
            .units()
            .filter_map(|u| UnitSnap::from_state(u, sim.content()))
            .collect(),
        players: sim
            .players()
            .iter()
            .filter_map(|p| sim.unit(*p))
            .map(|u| PlayerSnap::from_state(u, sim.content()))
            .collect(),
    }
}

/// What a real server does per tick for one client: route, frame, send.
fn events_for(sim: &mut Sim, you: EntityId) -> ServerMsg {
    let events = sim
        .tick_routed()
        .into_iter()
        .filter(|(a, _)| matches!(a, Audience::All) || *a == Audience::Player(you))
        .map(|(_, e)| e)
        .collect();
    ServerMsg::Events {
        tick: sim.now().0,
        events,
    }
}

fn client_cmd(sim: &mut Sim, you: EntityId, cmd: Command) {
    match wire(&ClientMsg::Cmd(cmd)) {
        ClientMsg::Cmd(c) => sim.apply_as(you, c),
        other => panic!("{other:?}"),
    }
}

fn feed(world: &mut NetWorld, msg: ServerMsg, now: f64) {
    match wire(&msg) {
        ServerMsg::Events { tick, events } => world.receive_events(tick, events, now),
        ServerMsg::Snapshot {
            tick,
            units,
            players,
        } => world.apply_snapshot(tick, &units, &players, now),
        ServerMsg::Positions { tick, units } => world.apply_positions(tick, &units, now),
        ServerMsg::Auras { units } => world.apply_auras(&units),
        ServerMsg::Cooldowns { tick, gcd, spells } => world.apply_cooldowns(tick, gcd, &spells),
        other => panic!("unexpected {other:?}"),
    }
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

#[test]
fn fake_server_round_trip_keeps_networld_in_step() {
    // Handshake as the session runs it.
    let (phase, out) = advance(Phase::Idle, &Step::Opened);
    assert_eq!(out, Outcome::SendHello);
    let hello = hello_frames(Intent::Create, "Dana", "", "fullstack");
    assert!(matches!(wire(&hello[0]), ClientMsg::Hello { .. }));
    assert_eq!(wire(&hello[1]), ClientMsg::CreateRoom);

    let Ok(mut sim) = new_sim() else {
        panic!("content")
    };
    let Some(layout) = build_zone("campus") else {
        panic!("campus layout")
    };
    let Some((intern_at, _)) = layout.anchor("npc_1") else {
        panic!("npc_1")
    };
    let you = sim.spawn_player("fullstack", to_core(intern_at), 0.0);
    let bob = sim.spawn_player("fullstack", to_core(intern_at), 0.0);
    let anchors = |n: &str| layout.anchor(n).map(|(p, y)| (to_core(p), y));
    if let Err(e) = sim.load_zone("campus", &anchors, layout.blockers.clone()) {
        panic!("load_zone: {e:?}");
    }
    let room = room_code_from_bytes(&[3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let welcome = wire(&ServerMsg::Welcome {
        you,
        room: room.clone(),
        zone: Some("campus".into()),
        seed: 1,
        players: vec![
            PlayerInfo {
                id: you,
                name: "Dana".into(),
                class: "fullstack".into(),
            },
            PlayerInfo {
                id: bob,
                name: "Bob".into(),
                class: "fullstack".into(),
            },
        ],
        quests: QuestSnap::default(),
        cleared: Vec::new(),
        unlocked: Vec::new(),
    });
    let (phase, out) = advance(phase, &Step::Welcome);
    assert_eq!((phase, out), (Phase::InRoom, Outcome::EnterRoom));
    let ServerMsg::Welcome {
        players,
        quests,
        cleared,
        unlocked,
        ..
    } = welcome
    else {
        panic!("welcome")
    };
    let mut world = NetWorld::new(sim.content().clone(), you, room, &players);
    world.apply_welcome(&quests, &cleared, &unlocked);

    let intern = sim
        .units()
        .find(|u| u.key == "intern")
        .map(|u| u.id)
        .unwrap_or_default();
    let quest = sim
        .content()
        .quests
        .iter()
        .find(|q| q.giver.npc == "intern" && q.requires.is_none() && q.level_min <= 1)
        .map(|q| q.id.clone())
        .unwrap_or_default();
    let mut mob = 0;
    for tick in 0..=310u32 {
        let now = f64::from(tick) / 30.0;
        match tick {
            3 => client_cmd(&mut sim, you, Command::Talk { unit: intern }),
            6 => client_cmd(
                &mut sim,
                you,
                Command::AcceptQuest {
                    quest: quest.clone(),
                },
            ),
            // Walk up to the nearest enemy and start auto attack on it.
            12 => {
                let me = sim.unit(you).map(|u| u.pos).unwrap_or_default();
                let Some(target) = sim
                    .units()
                    .filter(|u| u.hostile && u.alive)
                    .min_by(|a, b| {
                        combat_core::distance(a.pos, me)
                            .total_cmp(&combat_core::distance(b.pos, me))
                    })
                    .map(|u| (u.id, u.pos))
                else {
                    panic!("no enemy on campus")
                };
                mob = target.0;
                let stand = Vec3::new(target.1.x + 6.0, target.1.y, target.1.z);
                sim.apply_as(
                    you,
                    Command::PlayerMoved {
                        pos: stand,
                        yaw: yaw_to(stand, target.1),
                        moving: false,
                        jumping: false,
                    },
                );
                client_cmd(
                    &mut sim,
                    you,
                    Command::SetTarget {
                        unit: you,
                        target: Some(mob),
                    },
                );
            }
            14 => client_cmd(&mut sim, you, Command::StartAutoAttack { unit: you }),
            _ => {}
        }
        let msg = events_for(&mut sim, you);
        feed(&mut world, msg, now);
        if tick % 30 == 0 {
            feed(&mut world, snapshot(&sim), now);
        }
    }

    // Same units, same life state; the fight's HP flowed through events between snapshots.
    let server_ids: Vec<EntityId> = sim.units().map(|u| u.id).collect();
    let client_ids: Vec<EntityId> = world.units().map(|u| u.id).collect();
    assert_eq!(client_ids, server_ids);
    for u in sim.units() {
        let Some(c) = world.unit(u.id) else {
            panic!("missing {}", u.id)
        };
        assert_eq!(c.alive, u.alive, "{}", u.key);
        assert_eq!(c.key, u.key);
    }
    let (Some(sm), Some(cm)) = (sim.unit(mob), world.unit(mob)) else {
        panic!("mob gone")
    };
    assert!(sm.hp < sm.max_hp, "the fight never landed a hit");
    assert_eq!(cm.hp, sm.hp, "mob HP after event-only ticks");
    assert_eq!(world.unit(bob).map(|u| u.name.as_str()), Some("Bob"));
    assert_eq!(world.players(), &[you, bob]);
    assert_ne!(sim.quest_status_of(you, &quest), QuestStatus::Available);
    assert_eq!(world.quest_status(&quest), sim.quest_status_of(you, &quest));
    // Bob's log is not ours: his quests never reach this client.
    assert_eq!(world.quests().active.len(), sim.quests_of(you).active.len());

    // Interpolated positions settle on the last snapshot once the render delay has passed.
    let last = 300.0 / 30.0;
    world.advance(last + INTERP_DELAY_S + 2.5);
    let ServerMsg::Snapshot { units, .. } = snapshot(&sim) else {
        panic!("snapshot")
    };
    let (Some(snap), Some(shown)) = (
        units.iter().find(|s| s.id == bob).map(UnitSnap::pos),
        world.unit(bob).map(|u| u.pos),
    ) else {
        panic!("bob not shown")
    };
    assert!(
        combat_core::distance(snap, shown) < 0.02,
        "{snap:?} vs {shown:?}"
    );
}

fn positions(sim: &Sim) -> ServerMsg {
    ServerMsg::Positions {
        tick: sim.now().0,
        units: sim
            .units()
            .map(|u| (u.id, pos_cm(u.pos), quantize_yaw(u.yaw)))
            .collect(),
    }
}

fn campus_room() -> (Sim, EntityId, EntityId, NetWorld) {
    let Ok(mut sim) = new_sim() else {
        panic!("content")
    };
    let Some(layout) = build_zone("campus") else {
        panic!("campus layout")
    };
    let Some((spawn, _)) = layout.anchor("player_spawn") else {
        panic!("player_spawn")
    };
    let you = sim.spawn_player("fullstack", to_core(spawn), 0.0);
    let bob = sim.spawn_player("fullstack", to_core(spawn), 0.0);
    let anchors = |n: &str| layout.anchor(n).map(|(p, y)| (to_core(p), y));
    if let Err(e) = sim.load_zone("campus", &anchors, layout.blockers.clone()) {
        panic!("load_zone: {e:?}");
    }
    sim.set_wandering(false);
    let players = [
        PlayerInfo {
            id: you,
            name: "Dana".into(),
            class: "fullstack".into(),
        },
        PlayerInfo {
            id: bob,
            name: "Bob".into(),
            class: "fullstack".into(),
        },
    ];
    let world = NetWorld::new(sim.content().clone(), you, "R".into(), &players);
    (sim, you, bob, world)
}

/// Bob walks at 6 m/s; the server sends `Positions` every 3 ticks and a snapshot every 30, as
/// `room_state` does. The rendered Bob must trail the truth by the render delay only (not the
/// old 1.1 s) and move a little every frame instead of jumping once per snapshot.
#[test]
fn remote_movement_is_smooth_and_auras_count_down() {
    let (mut sim, you, bob, mut world) = campus_room();
    let start = sim.unit(bob).map(|u| u.pos).unwrap_or_default();
    let speed = 6.0;
    let mut truth: Vec<(f64, f32)> = Vec::new();
    let mut last_shown: Option<f32> = None;
    let (mut max_err, mut max_step) = (0.0f32, 0.0f32);
    for tick in 0..=150u32 {
        let now = f64::from(tick) / 30.0;
        let x = start.x + speed * tick as f32 / 30.0;
        sim.apply_as(
            bob,
            Command::PlayerMoved {
                pos: Vec3::new(x, start.y, start.z),
                yaw: 0.0,
                moving: true,
                jumping: false,
            },
        );
        let ev = events_for(&mut sim, you);
        feed(&mut world, ev, now);
        truth.push((now, sim.unit(bob).map_or(0.0, |u| u.pos.x)));
        if tick % 30 == 0 {
            feed(&mut world, snapshot(&sim), now);
        } else if tick % 3 == 0 {
            feed(&mut world, positions(&sim), now);
        }
        world.advance(now);
        let Some(shown) = world.unit(bob).map(|u| u.pos.x) else {
            panic!("bob not shown")
        };
        if tick >= 30 {
            // Where Bob really was one render delay ago.
            let delay = INTERP_DELAY_S + 0.1;
            let then = now - delay;
            let expect = truth
                .windows(2)
                .find(|w| w[0].0 <= then && then <= w[1].0)
                .map_or(shown, |w| {
                    let k = ((then - w[0].0) / (w[1].0 - w[0].0)) as f32;
                    w[0].1 + (w[1].1 - w[0].1) * k
                });
            max_err = max_err.max((shown - expect).abs());
            if let Some(prev) = last_shown {
                max_step = max_step.max((shown - prev).abs());
            }
            if tick % 15 == 0 {
                println!(
                    "interp t={now:.2}s server x={:.2} shown x={shown:.2} lag={:.2} m",
                    truth.last().map_or(0.0, |t| t.1),
                    truth.last().map_or(0.0, |t| t.1) - shown
                );
            }
        }
        last_shown = Some(shown);
    }
    println!(
        "interp max error vs delayed truth {max_err:.3} m, max per-frame step {max_step:.3} m"
    );
    // About 0.2 s behind (1.2 m at 6 m/s), within a few centimetres of the delayed truth.
    assert!(max_err < 0.25, "{max_err}");
    // Never more than ~1.5 frames of travel at once (0.2 m per 30 Hz frame at 6 m/s).
    assert!(max_step < 0.35, "{max_step}");

    // Aura timers: the server's list arrives with its timer and then counts down locally.
    let Some(mob) = sim
        .units()
        .find(|u| u.hostile && u.alive && !u.boss)
        .map(|u| u.id)
    else {
        panic!("no mob")
    };
    assert!(sim.debug_apply_aura(mob, "review_fatigue", 8_000));
    let now = 151.0 / 30.0;
    let ev = events_for(&mut sim, you);
    feed(&mut world, ev, now);
    let auras: Vec<AuraSnap> = sim
        .unit(mob)
        .map(|u| {
            u.auras
                .iter()
                .filter_map(|a| AuraSnap::from_view(a, sim.content()))
                .collect()
        })
        .unwrap_or_default();
    feed(
        &mut world,
        ServerMsg::Auras {
            units: vec![(mob, auras)],
        },
        now,
    );
    world.advance(now);
    world.advance(now + 2.0);
    let Some(a) = world.unit(mob).and_then(|u| u.auras.first()) else {
        panic!("aura not shown")
    };
    println!(
        "aura {} {} / {} ms after 2 s",
        a.key, a.remaining_ms, a.total_ms
    );
    assert_eq!(a.total_ms, 8_000);
    assert!((5_900..=6_100).contains(&a.remaining_ms), "{a:?}");
}
