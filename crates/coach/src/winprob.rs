//! Win probability from the scoreboard: a logistic regression whose coefficients change with
//! game time (`winprob.toml`, fitted offline by `tools/winprob`). Every input is something both
//! teams can see: items, levels, CS, kills, structures, objectives, who is dead, and the
//! champions' win rates by game length. Nothing estimates enemy gold or cooldowns.

use crate::{common::*, consts::*, fights, plan::Curve};
use model::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Feature keys in `State::features` order; the tables in `winprob.toml` use the same names.
pub const KEYS: [&str; 14] =
    ["gold", "level", "cs", "kills", "towers", "inhibs", "drakes", "soul", "elder", "baron", "grubs", "dead", "outlook", "side"];
const LABELS: [&str; 14] = [
    "item gold",
    "levels",
    "CS",
    "kills",
    "towers",
    "inhibitors",
    "dragons",
    "dragon soul",
    "Elder",
    "Baron",
    "grubs",
    "players dead",
    "draft and scaling",
    "side",
];
const DEAD: usize = 11;
const OUTLOOK: &str = KEYS[12];

/// Added to each side's item gold before the ratio, so the first buys do not swing it.
const GOLD_PAD: f32 = 3000.0;
/// A side's dragons count up to here; the next one is the soul.
const DRAKE_CAP: u32 = SOUL_DRAGONS - 1;
/// A death timer counts up to this many seconds.
const DEAD_CAP: f32 = 45.0;
/// The outlook is read now and this many minutes on.
const OUTLOOK_AHEAD: f32 = 10.0;
/// `trend` compares with the sample this old (s), give or take the slack.
const TREND_S: f32 = 120.0;
const TREND_SLACK: f32 = 60.0;
/// Factors under this many points are left out.
const FACTOR_MIN: f32 = 1.0;
/// Baron is a stake from this long before it first spawns (s).
const STAKE_LEAD: f32 = 180.0;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Model {
    /// Patch of the fitted games.
    pub patch: String,
    /// Games fitted; 0 = no fit yet, and no win probability.
    pub games: u32,
    /// Minutes. A coefficient runs straight between knots and stays flat after the last.
    pub knots: Vec<f32>,
    #[serde(default)]
    pub rift: Table,
    /// Also read for Mayhem.
    #[serde(default)]
    pub aram: Table,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Table {
    pub metrics: Option<Metrics>,
    /// Feature key → one value per knot; a key left out counts 0.
    #[serde(flatten)]
    pub beta: HashMap<String, Vec<f32>>,
}

/// Held-out results of the fit; `accuracy` at 10, 15, 20 and 25 minutes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Metrics {
    pub log_loss: f32,
    pub brier: f32,
    pub ece: f32,
    #[serde(default)]
    pub accuracy: Vec<f32>,
}

static SHIPPED: LazyLock<Model> = LazyLock::new(|| Model::parse(include_str!("../winprob.toml")).expect("winprob.toml is valid"));

impl Model {
    pub fn parse(toml: &str) -> Result<Model, toml::de::Error> {
        toml::from_str(toml)
    }

    /// `winprob.toml` as built into the app.
    pub fn shipped() -> &'static Model {
        &SHIPPED
    }

    /// Why a table from outside the build (downloaded) cannot be used, if so.
    pub fn fault(&self) -> Option<String> {
        if self.games == 0 {
            return Some("no games fitted".into());
        }
        if self.knots.is_empty() || self.knots.iter().any(|k| !k.is_finite()) || self.knots.windows(2).any(|w| w[0] >= w[1]) {
            return Some(format!("knots {:?} are not rising minutes", self.knots));
        }
        if self.rift.beta.is_empty() && self.aram.beta.is_empty() {
            return Some("no table".into());
        }
        [("rift", &self.rift), ("aram", &self.aram)].into_iter().flat_map(|(mode, t)| t.beta.iter().map(move |(k, v)| (mode, k, v))).find_map(|(mode, k, v)| {
            if !KEYS.contains(&k.as_str()) {
                Some(format!("{mode}.{k}: a feature this build does not know"))
            } else if v.len() != self.knots.len() || v.iter().any(|x| !x.is_finite()) {
                Some(format!("{mode}.{k}: not one number per knot"))
            } else {
                None
            }
        })
    }

    /// A later patch, or the same patch fitted on more games.
    pub fn newer_than(&self, other: &Model) -> bool {
        let rank = |m: &Model| (m.patch.split('.').map(|p| p.parse().unwrap_or(0)).collect::<Vec<u32>>(), m.games);
        rank(self) > rank(other)
    }

    pub(crate) fn table(&self, mode: Mode) -> Option<&Table> {
        let t = if mode == Mode::Rift { &self.rift } else { &self.aram };
        (self.games > 0 && !t.beta.is_empty()).then_some(t)
    }
}

/// What the model reads off a game at one moment. Pairs are [mine, theirs].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct State {
    pub minute: f32,
    pub item_gold: [f32; 2],
    /// Mean level.
    pub level: [f32; 2],
    pub cs: [f32; 2],
    pub kills: [f32; 2],
    /// Towers each side has taken.
    pub towers: [u32; 2],
    /// Inhibitors down right now: [theirs, ours].
    pub inhibs: [u32; 2],
    /// Elemental dragons taken, uncapped.
    pub drakes: [u32; 2],
    /// +1 mine, −1 theirs, 0 nobody's; `elder` and `baron` while the buff lasts.
    pub soul: i32,
    pub elder: i32,
    pub baron: i32,
    pub grubs: [u32; 2],
    /// Each side's dead players, a death timer of `DEAD_CAP` or more counting 1.
    pub dead: [f32; 2],
    /// Win-rate points of my champions over theirs in games at least this long.
    pub outlook: f32,
    /// +1 blue, −1 red, 0 unknown.
    pub side: i32,
}

fn soul(drakes: [u32; 2]) -> i32 {
    i32::from(drakes[0] >= SOUL_DRAGONS) - i32::from(drakes[1] >= SOUL_DRAGONS)
}

fn is_elder(kind: &str) -> bool {
    kind.to_ascii_lowercase().contains("elder")
}

fn is_grub(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("grub") || n.contains("horde")
}

/// Win rate (percent, shrunk by its games) over the length buckets still ahead at `minute`:
/// the one `minute` falls in and every later one.
fn wr_from(buckets: &[LengthWin], minute: f32) -> Option<f32> {
    let ahead = |i: usize| buckets.get(i + 1).is_none_or(|n| n.from_min as f32 > minute);
    let (games, wins) = buckets.iter().enumerate().filter(|(i, _)| ahead(*i)).fold((0u32, 0u32), |(g, w), (_, b)| (g + b.games, w + b.wins));
    (games > 0).then(|| shrink(100.0 * wins as f32 / games as f32, games, PLAN_SHRINK_K))
}

/// The outlook feature at `minute`, and how many players had a curve.
fn outlook(snap: &GameSnapshot, curves: &HashMap<ChampKey, Curve>, my_team: Team, minute: f32) -> (f32, usize) {
    snap.players.iter().filter_map(|p| Some((p, wr_from(&curves.get(&p.champ)?.1, minute)?))).fold((0.0, 0), |(sum, n), (p, wr)| {
        (sum + if p.team == my_team { wr - 50.0 } else { 50.0 - wr }, n + 1)
    })
}

impl State {
    /// From a live snapshot, seen from `my_team`. Champions without a curve add nothing to the outlook.
    pub fn from_snapshot(snap: &GameSnapshot, sd: &StaticData, curves: &HashMap<ChampKey, Curve>, my_team: Team) -> State {
        let minute = snap.time / 60.0;
        let side = |t: Team| usize::from(t != my_team);
        let sign = |t: Team| if t == my_team { 1 } else { -1 };
        let mut s = State { minute, side: if my_team == Team::Order { 1 } else { -1 }, outlook: outlook(snap, curves, my_team, minute).0, ..Default::default() };
        let mut n = [0f32; 2];
        for p in &snap.players {
            let i = side(p.team);
            s.item_gold[i] += inventory(sd, &p.items).gold as f32;
            s.level[i] += p.level as f32;
            s.cs[i] += p.cs as f32;
            s.kills[i] += p.kills as f32;
            if p.dead {
                s.dead[i] += p.respawn_in.clamp(0.0, DEAD_CAP) / DEAD_CAP;
            }
            n[i] += 1.0;
        }
        for i in 0..2 {
            s.level[i] /= n[i].max(1.0);
        }
        let mut down: Vec<(Team, Option<Lane>)> = Vec::new();
        for e in &snap.events {
            match e {
                GameEvent::Tower { team, .. } => s.towers[1 - side(*team)] += 1,
                GameEvent::Inhib { team, lane, respawned, .. } => {
                    down.retain(|d| *d != (*team, *lane));
                    if !respawned {
                        down.push((*team, *lane));
                    }
                }
                GameEvent::Dragon { time, team: Some(t), kind } if is_elder(kind) => {
                    if snap.time < time + ELDER_BUFF {
                        s.elder = sign(*t);
                    }
                }
                GameEvent::Dragon { team: Some(t), .. } => s.drakes[side(*t)] += 1,
                GameEvent::Baron { time, team: Some(t) } if snap.time < time + BARON_BUFF => s.baron = sign(*t),
                GameEvent::Objective { team: Some(t), name, .. } if is_grub(name) => s.grubs[side(*t)] += 1,
                _ => {}
            }
        }
        for (team, _) in down {
            s.inhibs[1 - side(team)] += 1;
        }
        s.soul = soul(s.drakes);
        s.on(snap.mode)
    }

    /// Howling Abyss keeps the ARAM features only: no CS, side, dragons, Baron or grubs.
    fn on(self, mode: Mode) -> State {
        if mode == Mode::Rift { self } else { State { cs: [0.0; 2], drakes: [0; 2], soul: 0, elder: 0, baron: 0, grubs: [0; 2], side: 0, ..self } }
    }

    /// The same features at one frame of a past game's timeline, with gold spent
    /// (`total_gold − current_gold`) standing in for item gold. The timeline has no inhibitor
    /// respawns (timed here) and the review no curves (outlook 0). Participants 1–5 are blue.
    pub fn from_frame(d: &MatchDetail, frame_index: usize) -> State {
        let Some(f) = d.frames.get(frame_index) else { return State::default() };
        let side = |i: usize| usize::from(!d.players.get(i).is_some_and(|p| p.ally));
        let blue = d.players.first().is_some_and(|p| p.ally);
        let mut s = State { minute: f.time / 60.0, side: if blue { 1 } else { -1 }, ..Default::default() };
        let mut n = [0f32; 2];
        for (i, p) in f.players.iter().enumerate() {
            let t = side(i);
            s.item_gold[t] += p.total_gold.saturating_sub(p.current_gold) as f32;
            s.level[t] += p.level as f32;
            s.cs[t] += (p.cs + p.jungle_cs) as f32;
            n[t] += 1.0;
        }
        for i in 0..2 {
            s.level[i] /= n[i].max(1.0);
        }
        for k in d.kills.iter().filter(|k| k.time <= f.time) {
            if let Some(killer) = k.killer {
                s.kills[side(killer)] += 1.0;
            }
            let level = d.frames.iter().rev().find(|x| x.time <= k.time).and_then(|x| x.players.get(k.victim)).map_or(1, |p| p.level);
            let left = k.time + fights::respawn(level, k.time) - f.time;
            if left > 0.0 {
                s.dead[side(k.victim)] += left.min(DEAD_CAP) / DEAD_CAP;
            }
        }
        let inhib_back = if d.game.mode == Mode::Rift { INHIB_RESPAWN } else { INHIB_RESPAWN_ARAM };
        for m in d.marks.iter().filter(|m| m.time <= f.time) {
            let (t, sign) = if m.ours { (0, 1) } else { (1, -1) };
            match m.kind.as_str() {
                "Tower" => s.towers[t] += 1,
                "Inhibitor" if f.time < m.time + inhib_back => s.inhibs[t] += 1,
                "Dragon" if m.detail.as_deref().is_some_and(is_elder) => {
                    if f.time < m.time + ELDER_BUFF {
                        s.elder = sign;
                    }
                }
                "Dragon" => s.drakes[t] += 1,
                "Baron" if f.time < m.time + BARON_BUFF => s.baron = sign,
                "Grubs" => s.grubs[t] += m.detail.as_deref().and_then(|d| d.trim_start_matches('×').parse().ok()).unwrap_or(1),
                _ => {}
            }
        }
        s.soul = soul(s.drakes);
        s.on(d.game.mode)
    }

    /// Mine minus theirs for every key, so swapping the teams flips every sign.
    pub fn features(&self) -> [f32; KEYS.len()] {
        let diff = |v: [u32; 2]| v[0] as f32 - v[1] as f32;
        let (mine, theirs) = (self.kills[0], self.kills[1]);
        [
            ((self.item_gold[0] + GOLD_PAD) / (self.item_gold[1] + GOLD_PAD)).ln(),
            self.level[0] - self.level[1],
            (self.cs[0] - self.cs[1]) / 100.0,
            (mine - theirs) / (mine + theirs + 4.0).sqrt(),
            diff(self.towers),
            diff(self.inhibs),
            diff(self.drakes.map(|n| n.min(DRAKE_CAP))),
            self.soul as f32,
            self.elder as f32,
            self.baron as f32,
            diff(self.grubs) / GRUBS_COUNT as f32,
            self.dead[1] - self.dead[0],
            self.outlook,
            self.side as f32,
        ]
    }
}

/// A coefficient at `minute`: straight between knots, flat before the first and after the last.
pub fn beta(knots: &[f32], values: &[f32], minute: f32) -> f32 {
    let n = knots.len().min(values.len());
    let Some(i) = (0..n).rposition(|i| knots[i] <= minute) else { return values.first().copied().unwrap_or(0.0) };
    if i + 1 < n { values[i] + (values[i + 1] - values[i]) * (minute - knots[i]) / (knots[i + 1] - knots[i]) } else { values[i] }
}

/// Each feature's share of the logit at `minute`.
fn terms(knots: &[f32], table: &Table, s: &State, minute: f32) -> [f32; KEYS.len()] {
    let x = s.features();
    std::array::from_fn(|k| table.beta.get(KEYS[k]).map_or(0.0, |v| beta(knots, v, minute) * x[k]))
}

fn sigmoid(z: f32) -> f32 {
    1.0 / (1.0 + (-z).exp())
}

/// My team's chance to win in `s`, with the coefficients of `minute`.
pub fn eval(knots: &[f32], table: &Table, s: &State, minute: f32) -> f32 {
    sigmoid(terms(knots, table, s, minute).iter().sum())
}

pub struct Input<'a> {
    pub model: &'a Model,
    pub snap: &'a GameSnapshot,
    pub sd: &'a StaticData,
    /// Games and wins by game length for the champions in the game, as far as fetched.
    pub curves: &'a HashMap<ChampKey, Curve>,
    /// Samples so far this game (`GoldHistory::samples`), for the trend.
    pub history: &'a [GoldSample],
}

/// None without a fit, without a table for the mode, or without me in the snapshot.
pub fn win_prob(i: &Input) -> Option<WinProb> {
    let Input { model, snap, sd, curves, history } = *i;
    let table = model.table(snap.mode)?;
    let me = snap.players.get(snap.me)?;
    let s = State::from_snapshot(snap, sd, curves, me.team);
    let t = terms(&model.knots, table, &s, s.minute);
    let z: f32 = t.iter().sum();
    let (p, base) = (sigmoid(z), sigmoid(z - t[DEAD]));

    let mut factors: Vec<WinFactor> = (0..KEYS.len())
        .map(|k| WinFactor { key: KEYS[k].into(), label: LABELS[k].into(), points: 100.0 * (p - sigmoid(z - t[k])) })
        .filter(|f| f.points.abs() >= FACTOR_MIN)
        .collect();
    factors.sort_by(|a, b| b.points.abs().total_cmp(&a.points.abs()));

    let age = |h: &GoldSample| (snap.time - h.time - TREND_S).abs();
    let trend = history
        .iter()
        .filter(|h| h.win.is_some() && age(h) <= TREND_SLACK)
        .min_by(|a, b| age(a).total_cmp(&age(b)))
        .and_then(|h| h.win)
        .map(|w| 100.0 * (base - w));

    let (_, found) = outlook(snap, curves, me.team, s.minute);
    let later = s.minute + OUTLOOK_AHEAD;
    let fit = format!("fit on {} ({} games)", model.patch, fmt_games(model.games as u64));
    Some(WinProb {
        p,
        base,
        trend,
        factors,
        stakes: stakes(snap, me, &s, |n| eval(&model.knots, table, n, s.minute)),
        outlook: (found > 0).then(|| Outlook { now: s.outlook, later: outlook(snap, curves, me.team, later).0, later_min: later as u32 }),
        rough: snap.mode == Mode::Mayhem || snap.swiftplay || (table.beta.contains_key(OUTLOOK) && found < snap.players.len()),
        source: match snap.mode {
            Mode::Mayhem => format!("ARAM model, augments not counted · {fit}"),
            Mode::Rift if snap.swiftplay => format!("Rift model on Swiftplay · {fit}"),
            _ => format!("from the scoreboard · {fit}"),
        },
    })
}

/// What the next objectives are worth: my chance with it on my side minus with it on theirs,
/// in points, largest first. Rift adds what dying now costs.
fn stakes(snap: &GameSnapshot, me: &Player, s: &State, p: impl Fn(&State) -> f32) -> Vec<Stake> {
    let mut out: Vec<Stake> = Vec::new();
    let mut push = |name: &str, mine: State, theirs: State, note: Option<String>| {
        out.push(Stake { name: name.into(), swing: 100.0 * (p(&mine) - p(&theirs)), note });
    };
    let rift = snap.mode == Mode::Rift;
    if rift && s.soul == 0 {
        let take = |i: usize| {
            let mut n = *s;
            n.drakes[i] += 1;
            n.soul = soul(n.drakes);
            n
        };
        let note = match s.drakes.map(|d| d + 1 == SOUL_DRAGONS) {
            [true, true] => Some("the soul"),
            [true, false] => Some("your soul"),
            [false, true] => Some("their soul"),
            _ => None,
        };
        push("Dragon", take(0), take(1), note.map(str::to_string));
    } else if rift {
        push("Elder", State { elder: 1, ..*s }, State { elder: -1, ..*s }, None);
    }
    if rift && snap.time + STAKE_LEAD >= BARON_FIRST {
        push("Baron", State { baron: 1, ..*s }, State { baron: -1, ..*s }, None);
    }
    let plus = |v: [u32; 2], i: usize| if i == 0 { [v[0] + 1, v[1]] } else { [v[0], v[1] + 1] };
    push("Tower", State { towers: plus(s.towers, 0), ..*s }, State { towers: plus(s.towers, 1), ..*s }, None);
    let open = if rift { 3 } else { 2 };
    if s.inhibs != [0, 0] || snap.events.iter().any(|e| matches!(e, GameEvent::Tower { tier, .. } if *tier >= open)) {
        push("Inhibitor", State { inhibs: plus(s.inhibs, 0), ..*s }, State { inhibs: plus(s.inhibs, 1), ..*s }, None);
    }
    if rift && !me.dead {
        let secs = fights::respawn(me.level, snap.time);
        let dead = State { dead: [s.dead[0] + secs.min(DEAD_CAP) / DEAD_CAP, s.dead[1]], ..*s };
        push("Your death", *s, dead, Some(format!("{secs:.0} s dead")));
    }
    out.sort_by(|a, b| b.swing.total_cmp(&a.swing));
    out
}
