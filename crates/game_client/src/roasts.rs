//! Roast lines for the moments a corporate game should be rude about: death, loading, idling,
//! wipes, lag, long sessions, empty pockets, slow levelling and connection changes.
//!
//! Every pool is drawn through a shuffle bag, so a line comes back only after the whole pool was
//! used (and never twice in a row across the refill). Rare triggers also carry a cooldown. Nothing
//! here blocks input: lines go to banners, the chat log, the error strip or the death panel.
//! Flavour that is purely client-side lives here, not in `game.ron`, because the sim never needs it.

use crate::combat::{CombatEvent, SimRead, SimReady, SimView, UiNotice};
use crate::economy::EconomyUi;
use crate::hud::{Banner, LogLineMsg};
use crate::net::{GameMode, NetWorld};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::input::touch::Touches;
use bevy::prelude::*;
use combat_core::{EntityId, Event};
use std::collections::HashMap;

const AFK_AFTER_SECS: f32 = 120.0;
const AFK_COOLDOWN_SECS: f32 = 600.0;
const PING_LIMIT_MS: f64 = 250.0;
const PING_SUSTAIN_SECS: f32 = 20.0;
const PING_COOLDOWN_SECS: f32 = 900.0;
const WIPE_COOLDOWN_SECS: f32 = 30.0;
const SESSION_STAGES_SECS: [f32; 2] = [2.0 * 3600.0, 4.0 * 3600.0];
/// Levelling counts as slow past this many active seconds per XP of the level's bar. The curve
/// reaches level 10 in a few hours (about 0.2 s per XP); this is 2.5 times that.
const SLOW_SECS_PER_XP: f32 = 0.5;
const SLOW_MIN_SECS: f32 = 15.0 * 60.0;
/// Active time stops counting for slow levelling after this long without input.
const ACTIVE_IDLE_SECS: f32 = 60.0;
/// Third, fifth and tenth death on the same boss get a line of their own.
const BOSS_TIERS: [(u32, Pool); 3] = [(3, Pool::Boss3), (5, Pool::Boss5), (10, Pool::Boss10)];

const ROAST_RED: Color = Color::srgb(0.95, 0.3, 0.25);
const ROAST_AMBER: Color = Color::srgb(1.0, 0.72, 0.25);
const ROAST_GREY: Color = Color::srgb(0.7, 0.74, 0.82);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    DeathBoss,
    DeathTrash,
    DeathEnv,
    DeathBurnout,
    DeathAny,
    Boss3,
    Boss5,
    Boss10,
    Loading,
    Afk,
    Wipe,
    Ping,
    Session2h,
    Session4h,
    VendorEmpty,
    VendorDeclined,
    SlowLevel,
    Disconnect,
    Offline,
}

impl Pool {
    const ALL: [Pool; 19] = [
        Pool::DeathBoss,
        Pool::DeathTrash,
        Pool::DeathEnv,
        Pool::DeathBurnout,
        Pool::DeathAny,
        Pool::Boss3,
        Pool::Boss5,
        Pool::Boss10,
        Pool::Loading,
        Pool::Afk,
        Pool::Wipe,
        Pool::Ping,
        Pool::Session2h,
        Pool::Session4h,
        Pool::VendorEmpty,
        Pool::VendorDeclined,
        Pool::SlowLevel,
        Pool::Disconnect,
        Pool::Offline,
    ];

    fn lines(self) -> &'static [&'static str] {
        match self {
            Pool::DeathBoss => &DEATH_BOSS,
            Pool::DeathTrash => &DEATH_TRASH,
            Pool::DeathEnv => &DEATH_ENV,
            Pool::DeathBurnout => &DEATH_BURNOUT,
            Pool::DeathAny => &DEATH_ANY,
            Pool::Boss3 => &BOSS_3,
            Pool::Boss5 => &BOSS_5,
            Pool::Boss10 => &BOSS_10,
            Pool::Loading => &LOADING,
            Pool::Afk => &AFK,
            Pool::Wipe => &WIPE,
            Pool::Ping => &PING,
            Pool::Session2h => &SESSION_2H,
            Pool::Session4h => &SESSION_4H,
            Pool::VendorEmpty => &VENDOR_EMPTY,
            Pool::VendorDeclined => &VENDOR_DECLINED,
            Pool::SlowLevel => &SLOW_LEVEL,
            Pool::Disconnect => &DISCONNECT,
            Pool::Offline => &OFFLINE,
        }
    }

    fn index(self) -> usize {
        Pool::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

const DEATH_BOSS: [&str; 8] = [
    "Killed by a boss. The AI would have negotiated a better severance.",
    "The boss survived. Your performance review has been moved up to Monday.",
    "Defeated by management. A strong pattern in your career.",
    "Cause of death: a stakeholder with a vision.",
    "The boss was informed of your failure. He sent a calendar invite titled 'Quick sync'.",
    "You lost to someone who has never shipped anything. Welcome to leadership.",
    "Your death was logged by the boss as 'a strategic learning opportunity'. For him.",
    "Your fall was escalated as an incident. The root cause is listed as 'you'.",
];

const DEATH_TRASH: [&str; 8] = [
    "Killed by a regular enemy. HR will mention it in your review, right under 'team player'.",
    "Lost to the rank and file. The AI saw it and updated your skill matrix.",
    "Cause of death: a low-priority ticket that got escalated.",
    "A nobody just killed you. Remember this the next time you ask for a promotion.",
    "Killed by a minion. Your LinkedIn now says 'open to work'. For about ten seconds.",
    "Slain by someone with a lanyard and nothing to lose.",
    "You died to trash. The post-mortem blames 'insufficient synergy'.",
    "Taken out by a mid-level employee. They will take credit for it at the all-hands.",
];

const DEATH_ENV: [&str; 6] = [
    "You fell. The org chart shows it as a lateral move.",
    "Gravity wins again. It has no respect for your notice period either.",
    "Cause of death: the floor. Facilities has been informed. Facilities does not care.",
    "You died to the environment. Management blames the open space.",
    "Unplanned descent. Finance counts it as a cost-saving measure.",
    "The building killed you. Its last renovation was funded by a downsizing.",
];

const DEATH_BURNOUT: [&str; 6] = [
    "You died Burnt Out. The wellness webinar is on Thursday. Attendance is mandatory.",
    "Burnout wins. HR suggests a mindfulness app. It runs on your own time.",
    "You collapsed. Your manager called it a growth opportunity and asked who covers your tickets.",
    "Dead tired, literally. Your out-of-office is accurate for the first time.",
    "Burnout fatality. The company thanks you for your passion and has already forgotten it.",
    "You worked yourself to death. The AI works 24/7 and never asks for coffee. Think about that.",
];

const DEATH_ANY: [&str; 14] = [
    "You died. The AI did your job in the meantime. Nobody noticed.",
    "Your role has been backfilled. Your onboarding is scheduled for the same hour.",
    "You were replaced by an AI agent before you hit the floor.",
    "Please hold. Your resurrection is number 47 in the queue.",
    "You died. The incident bridge did not even have a name for it.",
    "Cause of death logged as 'attrition'. The board is thrilled.",
    "Your departure was announced in a Slack channel nobody reads.",
    "Rest in peace. Your laptop has already been reassigned.",
    "The AI reviewed your death and found five areas for improvement. None was survival.",
    "You died. Your team will remember you fondly until the next reorg.",
    "A moment of silence was scheduled. It was moved to make room for a sync.",
    "Your pull request outlived you. It still has no reviewers.",
    "Death is a hard restart. Management prefers the term 'workforce optimisation'.",
    "Your badge was deactivated before you finished falling.",
];

const BOSS_3: [&str; 4] = [
    "Interesting strategy.",
    "Third time. Persistence is a KPI, just not one that gets bonuses.",
    "The boss has started recognising you at the door.",
    "Third attempt. Even the AI would have read the mechanics by now.",
];

const BOSS_5: [&str; 4] = [
    "We've started a performance review.",
    "Five deaths. HR has opened a folder with your name on it.",
    "The boss has added 'repeat visitor' to your file. It is not a loyalty programme.",
    "At this point the boss is your most reliable colleague. He shows up every time.",
];

const BOSS_10: [&str; 4] = [
    "The boss has asked to be reassigned.",
    "Ten deaths. The boss has filed a harassment complaint.",
    "Ten deaths. The AI has quietly applied for your position on this fight.",
    "Ten deaths. Leadership is calling it a pilot programme and extending it.",
];

const LOADING: [&str; 34] = [
    "Loading... The AI loaded this zone 4 seconds ago. It's waiting for you. Again.",
    "Loading. The AI is summarising the zone so you do not have to experience it.",
    "Loading. Your manager is reorganising the zone while you wait.",
    "Fetching assets. Fetching headcount cuts. Fetching your replacement.",
    "Loading takes longer on days when someone asks a 'quick question'.",
    "This loading screen was optimised by a consultant. It is now 40% slower and 100% more strategic.",
    "Zone loading. A town hall about it is already on your calendar.",
    "Please wait. Your zone is stuck in a steering committee.",
    "Loading. The AI would be done already, but it is in an all-hands about you.",
    "Rendering the zone. Rendering unto Caesar. Rendering your role redundant.",
    "Please do not close this window. We are monitoring how long you wait.",
    "Loading. Unlike your career, this progress bar at least moves.",
    "The next zone has been outsourced. Quality is TBD.",
    "Loading. In the meantime, here is a motivational poster about resilience.",
    "Your zone is being loaded by an intern. The intern is a prompt.",
    "The last zone was a pilot. This one is the rollout. There is no difference.",
    "Loading. Your equity has vested exactly as much as this progress bar.",
    "Hold on, we are aligning on who owns this zone. Nobody wants it.",
    "A LinkedIn post about this zone has been drafted by the AI. It already has 3,000 likes.",
    "A new zone, a new team, the same Slack notifications.",
    "Resolving dependencies. Resolving conflicts. Resolving to quietly update your CV.",
    "Return to office is now mandatory in this zone as well.",
    "This zone was approved without a budget, a plan or you.",
    "The AI estimated this wait at 2 seconds. It estimates your value in fewer.",
    "Please wait. Your zone is queued behind the CEO's podcast setup.",
    "Loading. The AI says it would be faster without a human in the loop.",
    "The zone you are entering survived three layoffs. Its plants did not.",
    "Loading: mandatory fun, 100% synergy, 0% purpose.",
    "Brace for the all-hands. Everything is fine. The slides say so.",
    "A feedback form about this loading screen will be sent. Nobody will read it.",
    "Please wait. Someone scheduled a meeting that could have been an email that could have been silence.",
    "Ready in five minutes. The AI says that to everyone.",
    "Your badge is re-evaluated at every door. So is its owner.",
    "Please wait. Procurement is negotiating a licence for the next room.",
];

const AFK: [&str; 14] = [
    "Inactivity detected. Your manager has been notified. The AI has been offered your seat.",
    "Still there? The AI never had to ask itself.",
    "Two minutes without a keystroke. Your output is now indistinguishable from the average.",
    "Away status is visible to everyone. Everyone has opinions.",
    "Your green dot turned yellow. Somebody in management noticed.",
    "AFK detected. The AI closed three of your tickets and thanked itself in the changelog.",
    "You stepped away. The reorg did not.",
    "The meeting started without you. It went great.",
    "Idle time logged. HR calls it quiet quitting. Finance calls it savings.",
    "Your mouse jiggler is on a performance plan too.",
    "Someone reviewed your absence. Verdict: sustainable.",
    "Screen time is down. So is your visibility. Both are tracked.",
    "Break detected. It will be reflected in your engagement score.",
    "Nobody is watching you. That is either the problem or the dream.",
];

const WIPE: [&str; 12] = [
    "Wipe. Everyone dies. The AI sends its condolences and a feedback survey.",
    "The whole team is dead. Leadership calls it a bold alignment of outcomes.",
    "Total party wipe. The post-mortem will be blameless, until it is not.",
    "Your team has been restructured. All of it. At once.",
    "A synchronised layoff. Rare, efficient, very on-brand.",
    "All hands on deck, all hands dead. The all-hands will be rescheduled.",
    "Wipe. The AI has already onboarded replacements for everybody. They are cheaper.",
    "Everyone is dead. The retro is on Friday. Bring your own resurrection.",
    "Team wiped. Management suspects a lack of ownership. Specifically yours.",
    "No survivors. The incident ticket was assigned to someone who is dead.",
    "A total wipe. It will be called a pivot in the next investor update.",
    "The group is down. Somewhere a VP says 'resilience' and nobody knows to whom.",
];

const PING: [&str; 8] = [
    "Your latency is high. The AI blames the human in the loop.",
    "Lag detected. The server is on a performance improvement plan. So is your router.",
    "High ping. The cloud is just someone else's reorg.",
    "Your connection is slower than the approval chain for a new keyboard.",
    "Network lag. The AI never lags. It is also never actually here.",
    "Your connection was right-sized. Nobody asked the packets.",
    "Ping is high. Try turning it off and on again, as the last IT ticket suggested.",
    "Your packets are in a meeting. They will get back to you.",
];

const SESSION_2H: [&str; 6] = [
    "Two hours in. HR is proud of your engagement. Nobody else knows you exist.",
    "Two hours of play. This will not be reflected in your salary.",
    "Two hours. The AI worked for about two seconds and got the credit.",
    "Two hours of unpaid overtime logged. Thanks for the commitment.",
    "Two hours at the desk. The wellness app suggests a stretch. The AI app suggests more work.",
    "Two hours. Your chair has filed for a transfer.",
];

const SESSION_4H: [&str; 6] = [
    "Four hours. Half a workday. Management thinks you are not working.",
    "Four hours in. The game owns you now. Still better than the employer.",
    "Four hours straight. The AI is not worried. It is a model.",
    "Four hours. Please drink water. Wellness is also a KPI.",
    "Four hours. Your actual job would have paid something for this.",
    "Four hours of play. Your manager would call it a deep-work block if he got the credit.",
];

const VENDOR_EMPTY: [&str; 7] = [
    "Zero Kudos. Procurement thanks you for your visit and your poverty.",
    "You have no Kudos. Nobody noticed your work. Surprise.",
    "Empty wallet. The vendor offers LinkedIn endorsements instead. They are worth less.",
    "Kudos balance: 0. Your last good deed went unrecognised, as usual.",
    "No Kudos. Browsing is free. So is being ignored.",
    "You can window-shop. Like the rest of your equity.",
    "Zero Kudos. The vendor offered you exposure as payment.",
];

const VENDOR_DECLINED: [&str; 14] = [
    "Insufficient Kudos. Have you tried being more visible in meetings?",
    "Payment declined. Your Kudos balance is in the same state as your career.",
    "You can't afford this. The AI could. It doesn't even need it.",
    "Not enough Kudos. Try thanking someone publicly on Slack. Loudly.",
    "Not enough Kudos. Have you considered recognition from your manager? Neither have they.",
    "Declined. The AI could afford this. It is also your replacement.",
    "Transaction failed. Your Kudos was reallocated to the executive offsite.",
    "Insufficient funds. Budget approval takes six to eight weeks and a business case.",
    "You cannot afford this. Raise it in your next one-on-one. With a deck.",
    "Not enough Kudos. The vendor suggests a growth mindset. It is free. It is also useless.",
    "Declined. Your Kudos is with people who are 'looking into it'.",
    "Short on Kudos. Perhaps your equity can cover it. Oh, it vested at zero.",
    "Not enough Kudos. Please submit a request to Finance. Finance will submit one to you.",
    "Can't afford it. The AI wrote a business case in three seconds. Nobody read that either.",
];

const SLOW_LEVEL: [&str; 6] = [
    "Still level {level}. The intern is level 7. The intern is an AI.",
    "Level {level} for a while now. HR calls it a plateau. Finance calls it savings.",
    "Progress is slow. Your career ladder has been converted into a career lattice.",
    "This level is taking forever. The AI would have been promoted twice by now.",
    "No level-up lately. Your promotion case is 'in review'. It was in review before this game started.",
    "Still grinding. Management calls it dedication. Your raise is 'under evaluation'.",
];

const DISCONNECT: [&str; 5] = [
    "Connection lost. Unlike your job, it might come back.",
    "The server dropped you. At least it did not schedule a call first.",
    "Disconnected. Even the network has been downsized.",
    "Connection lost. IT opened a ticket. It will be closed as 'cannot reproduce'.",
    "You are offline. Your colleagues will notice in about three days.",
];

const OFFLINE: [&str; 5] = [
    "Playing offline. The only place left where the AI can't watch you work.",
    "Offline mode. No telemetry, no tracking, no manager. Enjoy it while it lasts.",
    "No network. Nobody can reorganise you right now.",
    "Offline: the only environment where your work is still entirely yours.",
    "Running locally. Like every good idea before leadership presents it as theirs.",
];

/// xorshift64*: enough for picking jokes, no dependency, same on native and wasm.
#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Wall-clock seed; `Instant` has no epoch and the browser has no `SystemTime`.
fn time_seed() -> u64 {
    #[cfg(target_arch = "wasm32")]
    let t = js_sys::Date::now() as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    t | 1
}

/// A shuffled pass over `0..len`; a refill never starts with the line the last pass ended on.
#[derive(Default, Clone)]
struct Bag {
    order: Vec<usize>,
    next: usize,
    last: Option<usize>,
}

impl Bag {
    fn draw(&mut self, len: usize, rng: &mut Rng) -> usize {
        if self.next >= self.order.len() || self.order.len() != len {
            self.order = (0..len).collect();
            for i in (1..len).rev() {
                self.order.swap(i, rng.below(i + 1));
            }
            if len > 1 && Some(self.order[0]) == self.last {
                let j = 1 + rng.below(len - 1);
                self.order.swap(0, j);
            }
            self.next = 0;
        }
        let i = self.order[self.next];
        self.next += 1;
        self.last = Some(i);
        i
    }
}

/// How the player died, as far as the client can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Boss,
    Trash,
    Environment,
    Burnout,
}

impl Cause {
    fn pool(self) -> Pool {
        match self {
            Cause::Boss => Pool::DeathBoss,
            Cause::Trash => Pool::DeathTrash,
            Cause::Environment => Pool::DeathEnv,
            Cause::Burnout => Pool::DeathBurnout,
        }
    }
}

/// Deaths per boss key this session.
#[derive(Default)]
struct BossDeaths(HashMap<String, u32>);

impl BossDeaths {
    fn record(&mut self, boss: &str) -> u32 {
        let n = self.0.entry(boss.to_owned()).or_insert(0);
        *n += 1;
        *n
    }
}

/// The pool for the `n`th death on one boss, if that death is a milestone.
fn escalation(n: u32) -> Option<Pool> {
    BOSS_TIERS.iter().find(|(at, _)| *at == n).map(|(_, p)| *p)
}

#[derive(Resource)]
pub struct Roasts {
    rng: Rng,
    bags: Vec<Bag>,
    /// Zone transitions draw from the roasts and the real tips as one bag.
    loading_bag: Bag,
    /// Seconds of real time since start; cooldowns are measured against it.
    now: f32,
    last_fired: [Option<f32>; Pool::ALL.len()],
    boss_deaths: BossDeaths,
}

impl Roasts {
    fn new(seed: u64) -> Self {
        Self {
            rng: Rng(seed | 1),
            bags: vec![Bag::default(); Pool::ALL.len()],
            loading_bag: Bag::default(),
            now: 0.0,
            last_fired: [None; Pool::ALL.len()],
            boss_deaths: BossDeaths::default(),
        }
    }

    pub fn pick(&mut self, pool: Pool) -> &'static str {
        let lines = pool.lines();
        lines[self.bags[pool.index()].draw(lines.len(), &mut self.rng)]
    }

    /// True once per `cooldown` seconds for this pool (and records the firing).
    fn ready(&mut self, pool: Pool, cooldown: f32) -> bool {
        let slot = &mut self.last_fired[pool.index()];
        if slot.is_some_and(|t| self.now - t < cooldown) {
            return false;
        }
        *slot = Some(self.now);
        true
    }

    /// A loading-screen line: the roasts shuffled together with the gameplay tips.
    pub fn loading(&mut self) -> &'static str {
        let roasts = Pool::Loading.lines();
        let tips = &crate::copy::LOADING_TIPS;
        let i = self
            .loading_bag
            .draw(roasts.len() + tips.len(), &mut self.rng);
        if i < roasts.len() {
            roasts[i]
        } else {
            tips[i - roasts.len()]
        }
    }

    /// The failed-purchase line.
    pub fn declined(&mut self) -> &'static str {
        self.pick(Pool::VendorDeclined)
    }

    /// Death panel text; a milestone death on a boss adds the escalation line below it.
    fn death(&mut self, cause: Cause, boss: Option<&str>) -> (String, String) {
        // The generic pool is mixed in so the cause pools do not run dry in a long session.
        let pool = if self.rng.below(3) == 0 {
            Pool::DeathAny
        } else {
            cause.pool()
        };
        let line = self.pick(pool).to_owned();
        let extra = boss
            .map(|b| self.boss_deaths.record(b))
            .and_then(escalation)
            .map(|p| self.pick(p).to_owned())
            .unwrap_or_default();
        (line, extra)
    }

    fn slow_level(&mut self, level: u8) -> String {
        self.pick(Pool::SlowLevel)
            .replace("{level}", &level.to_string())
    }
}

/// Active seconds a level may take before it counts as slow.
fn slow_threshold_secs(xp_to_next: u32) -> f32 {
    (xp_to_next as f32 * SLOW_SECS_PER_XP).max(SLOW_MIN_SECS)
}

/// What the death panel shows under its fixed title.
#[derive(Resource, Default)]
pub struct DeathRoast {
    pub text: String,
}

/// Seconds since the last keyboard, mouse or touch input.
#[derive(Resource, Default)]
struct Idle(f32);

fn death_cause(
    sim: &dyn SimRead,
    me: EntityId,
    killer: Option<EntityId>,
) -> (Cause, Option<String>) {
    let burnout_def = &sim.content().endgame.burnout;
    let burnt_out = sim.unit(me).is_some_and(|u| {
        let top = burnout_def.tiers.len().saturating_sub(1);
        top > 0 && burnout_def.tier_index(u.career.burnout) == top
    });
    if burnt_out {
        return (Cause::Burnout, None);
    }
    match killer.and_then(|k| sim.unit(k)) {
        Some(k) if k.boss => (Cause::Boss, Some(k.key.clone())),
        Some(_) => (Cause::Trash, None),
        None => (Cause::Environment, None),
    }
}

pub struct RoastPlugin;

impl Plugin for RoastPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Roasts::new(time_seed()))
            .init_resource::<DeathRoast>()
            .init_resource::<Idle>()
            .add_systems(
                Update,
                (
                    tick_clock,
                    on_death,
                    track_idle,
                    afk,
                    wipe,
                    high_ping,
                    long_session,
                    vendor_visit,
                    slow_level,
                    offline_hello,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn tick_clock(time: Res<Time<Real>>, mut roasts: ResMut<Roasts>) {
    roasts.now += time.delta_secs();
}

fn on_death(
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    mut roasts: ResMut<Roasts>,
    mut death: ResMut<DeathRoast>,
) {
    let Some(me) = sim.player() else { return };
    for CombatEvent(ev) in events.read() {
        let Event::Death { unit, killer } = ev else {
            continue;
        };
        if *unit != me {
            continue;
        }
        let (cause, boss) = death_cause(&*sim, me, *killer);
        let (line, extra) = roasts.death(cause, boss.as_deref());
        death.text = if extra.is_empty() {
            line
        } else {
            format!("{line}\n{extra}")
        };
    }
}

fn track_idle(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    mut idle: ResMut<Idle>,
) {
    let active = keys.get_pressed().next().is_some()
        || mouse.get_pressed().next().is_some()
        || motion.delta != Vec2::ZERO
        || scroll.delta != Vec2::ZERO
        || touches.iter().next().is_some();
    idle.0 = if active {
        0.0
    } else {
        idle.0 + time.delta_secs()
    };
}

fn afk(idle: Res<Idle>, mut roasts: ResMut<Roasts>, mut banners: MessageWriter<Banner>) {
    if idle.0 >= AFK_AFTER_SECS && roasts.ready(Pool::Afk, AFK_COOLDOWN_SECS) {
        banners.write(Banner {
            title: "Away from keyboard".into(),
            subtitle: roasts.pick(Pool::Afk).into(),
            color: ROAST_AMBER,
        });
    }
}

/// Co-op only: the whole group (two or more) is down at once.
fn wipe(
    sim: SimView,
    mut roasts: ResMut<Roasts>,
    mut banners: MessageWriter<Banner>,
    mut was_wiped: Local<bool>,
) {
    let players = sim.players();
    let all_dead = players.len() >= 2
        && players
            .iter()
            .all(|id| sim.unit(*id).is_some_and(|u| !u.alive));
    if all_dead && !*was_wiped && roasts.ready(Pool::Wipe, WIPE_COOLDOWN_SECS) {
        banners.write(Banner {
            title: "Total wipe".into(),
            subtitle: roasts.pick(Pool::Wipe).into(),
            color: ROAST_RED,
        });
    }
    *was_wiped = all_dead;
}

fn high_ping(
    time: Res<Time<Real>>,
    net: Option<Res<NetWorld>>,
    mut roasts: ResMut<Roasts>,
    mut banners: MessageWriter<Banner>,
    mut high_for: Local<f32>,
) {
    let slow = net.is_some_and(|w| w.rtt_avg_s() * 1000.0 > PING_LIMIT_MS);
    *high_for = if slow {
        *high_for + time.delta_secs()
    } else {
        0.0
    };
    if *high_for >= PING_SUSTAIN_SECS && roasts.ready(Pool::Ping, PING_COOLDOWN_SECS) {
        *high_for = 0.0;
        banners.write(Banner {
            title: "High latency".into(),
            subtitle: roasts.pick(Pool::Ping).into(),
            color: ROAST_AMBER,
        });
    }
}

fn long_session(
    mut roasts: ResMut<Roasts>,
    mut banners: MessageWriter<Banner>,
    mut stage: Local<usize>,
) {
    let Some(&at) = SESSION_STAGES_SECS.get(*stage) else {
        return;
    };
    if roasts.now < at {
        return;
    }
    let (title, pool) = if *stage == 0 {
        ("Two hours in", Pool::Session2h)
    } else {
        ("Four hours in", Pool::Session4h)
    };
    *stage += 1;
    banners.write(Banner {
        title: title.into(),
        subtitle: roasts.pick(pool).into(),
        color: ROAST_GREY,
    });
}

/// Opening a vendor with 0 Kudos: once per visit.
fn vendor_visit(
    ui: Res<EconomyUi>,
    sim: SimView,
    mut roasts: ResMut<Roasts>,
    mut notices: MessageWriter<UiNotice>,
    mut open: Local<Option<EntityId>>,
) {
    let was = *open;
    *open = ui.shop;
    if ui.shop.is_none() || ui.shop == was {
        return;
    }
    if sim
        .player()
        .and_then(|p| sim.unit(p))
        .is_some_and(|u| u.sp == 0)
    {
        notices.write(UiNotice(roasts.pick(Pool::VendorEmpty).into()));
    }
}

/// One roast per level when it takes far longer than the XP curve says it should.
fn slow_level(
    time: Res<Time<Real>>,
    idle: Res<Idle>,
    sim: SimView,
    mut roasts: ResMut<Roasts>,
    mut log: MessageWriter<LogLineMsg>,
    // (level being timed, active seconds on it, already roasted)
    mut track: Local<(u8, f32, bool)>,
) {
    let Some(u) = sim.player().and_then(|p| sim.unit(p)) else {
        return;
    };
    if u.level != track.0 {
        *track = (u.level, 0.0, false);
    }
    if idle.0 < ACTIVE_IDLE_SECS && u.alive {
        track.1 += time.delta_secs();
    }
    if !track.2 && u.xp_to_next > 0 && track.1 > slow_threshold_secs(u.xp_to_next) {
        track.2 = true;
        log.write(LogLineMsg(roasts.slow_level(u.level), ROAST_AMBER));
    }
}

/// The game always starts offline; say so once, in chat, when the sim is up.
fn offline_hello(
    mode: Res<GameMode>,
    mut roasts: ResMut<Roasts>,
    mut log: MessageWriter<LogLineMsg>,
    mut done: Local<bool>,
) {
    if *done || mode.is_online() {
        return;
    }
    *done = true;
    log.write(LogLineMsg(roasts.pick(Pool::Offline).into(), ROAST_GREY));
}

/// Chat line for a dropped room connection (the notice banner already says what happened).
pub fn disconnect_line(roasts: &mut Roasts) -> LogLineMsg {
    LogLineMsg(roasts.pick(Pool::Disconnect).into(), ROAST_RED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn pools_are_non_empty_unique_and_big_enough() {
        for pool in Pool::ALL {
            let lines = pool.lines();
            assert!(!lines.is_empty(), "{pool:?}");
            let unique: HashSet<_> = lines.iter().collect();
            assert_eq!(unique.len(), lines.len(), "{pool:?} has a duplicate");
            for l in lines {
                assert_eq!(l.trim(), *l, "{pool:?}: {l}");
                assert!(!l.to_lowercase().contains("story point"), "{l}");
            }
        }
        let death: usize = [
            Pool::DeathBoss,
            Pool::DeathTrash,
            Pool::DeathEnv,
            Pool::DeathBurnout,
            Pool::DeathAny,
        ]
        .iter()
        .map(|p| p.lines().len())
        .sum();
        assert!(death >= 30);
        assert!(Pool::Loading.lines().len() >= 30);
        assert!(Pool::Afk.lines().len() >= 10);
        assert!(Pool::Wipe.lines().len() >= 10);
        assert!(Pool::Ping.lines().len() >= 8);
        assert!(Pool::VendorDeclined.lines().len() >= 10);
        for pool in [Pool::Boss3, Pool::Boss5, Pool::Boss10] {
            assert!((3..=5).contains(&pool.lines().len()));
        }
    }

    #[test]
    fn shuffle_bag_uses_the_whole_pool_before_repeating() {
        for pool in Pool::ALL {
            let n = pool.lines().len();
            let mut roasts = Roasts::new(0x9E37_79B9);
            let mut prev: Option<&str> = None;
            for round in 0..4 {
                let mut seen = HashSet::new();
                for _ in 0..n {
                    let line = roasts.pick(pool);
                    assert!(seen.insert(line), "{pool:?} repeated within round {round}");
                    if n > 1 {
                        assert_ne!(prev, Some(line), "{pool:?} repeated back to back");
                    }
                    prev = Some(line);
                }
            }
        }
    }

    #[test]
    fn loading_bag_mixes_tips_and_roasts_without_repeats() {
        let n = Pool::Loading.lines().len() + crate::copy::LOADING_TIPS.len();
        let mut roasts = Roasts::new(7);
        let seen: HashSet<_> = (0..n).map(|_| roasts.loading()).collect();
        assert_eq!(seen.len(), n);
    }

    #[test]
    fn boss_deaths_escalate_at_three_five_and_ten() {
        let mut deaths = BossDeaths::default();
        let tiers: Vec<_> = (1..=12).map(|_| escalation(deaths.record("ceo"))).collect();
        assert_eq!(tiers[0], None);
        assert_eq!(tiers[1], None);
        assert_eq!(tiers[2], Some(Pool::Boss3));
        assert_eq!(tiers[4], Some(Pool::Boss5));
        assert_eq!(tiers[9], Some(Pool::Boss10));
        assert_eq!(tiers[10], None);
        // Another boss counts on its own.
        assert_eq!(deaths.record("ciso"), 1);
    }

    #[test]
    fn death_line_adds_an_escalation_only_on_milestones() {
        let mut roasts = Roasts::new(3);
        let extras: Vec<bool> = (0..10)
            .map(|_| !roasts.death(Cause::Boss, Some("ceo")).1.is_empty())
            .collect();
        let hits: Vec<usize> = extras
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.then_some(i + 1))
            .collect();
        assert_eq!(hits, vec![3, 5, 10]);
        assert!(roasts.death(Cause::Trash, None).1.is_empty());
    }

    #[test]
    fn cooldown_blocks_until_it_has_passed() {
        let mut roasts = Roasts::new(1);
        assert!(roasts.ready(Pool::Ping, 900.0));
        roasts.now = 899.0;
        assert!(!roasts.ready(Pool::Ping, 900.0));
        roasts.now = 900.0;
        assert!(roasts.ready(Pool::Ping, 900.0));
    }

    #[test]
    fn slow_threshold_follows_the_xp_curve() {
        assert_eq!(slow_threshold_secs(450), SLOW_MIN_SECS);
        assert!(slow_threshold_secs(14_400) > slow_threshold_secs(2_400));
        assert!(Roasts::new(5).slow_level(4).len() > 10);
    }
}
