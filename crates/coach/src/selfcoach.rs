//! Self-coaching within one game: death review (V10), deaths holding gold (Q9)
//! and goal progress. `GameLog` is fed every in-game tick and keeps bounded
//! state: the previous tick, level spikes, one note per death.

use crate::common::inventory;
use model::*;

pub const SPIKE_LEVELS: [u32; 3] = [6, 11, 16];
/// A level spike counts for deaths up to this long after it (s).
const SPIKE_WINDOW: f32 = 60.0;
/// Ticks are ~2 s apart: a spike seen just after the death may have happened just before it.
const TICK_SLACK: f32 = 3.0;
/// "Died holding gold" threshold for the review note.
pub const HOLDING_GOLD: u32 = 1000;
const CS_AT: f32 = 600.0;
const EARLY_AT: f32 = 840.0;
/// Average item-gold gap to the killer worth a note.
const GAP_NOTE: i32 = -500;
/// Deaths to one enemy worth a note.
const REPEAT_KILLER: usize = 3;
/// Death places worth a note from two deaths on (elsewhere from three).
const RISKY_PLACES: [&str; 3] = ["enemy jungle", "enemy base", "river"];
/// Kills less than this far apart (s) chain into one fight.
const FIGHT_GAP: f32 = 10.0;

/// What I had and what the coach said at one tick.
#[derive(Clone, Default)]
struct Moment {
    time: f32,
    gold: f32,
    owned: Vec<ItemId>,
    /// Item gold per player index.
    item_gold: Vec<u32>,
    top_threat: Option<ChampKey>,
    buy_next: Option<ItemPick>,
    alts: Vec<ItemId>,
}

#[derive(Default)]
pub struct GameLog {
    /// Game time of the first tick seen.
    first_seen: Option<f32>,
    /// ARAM/Mayhem: shopping only happens while dead, so gold held at death is normal.
    shop_on_death: bool,
    prev: Option<Moment>,
    levels: Vec<u32>,
    /// (player index, level, game time first seen).
    spikes: Vec<(usize, u32, f32)>,
    last_death: f32,
    pub deaths: Vec<DeathNote>,
    /// Every champion kill so far: (game time, my team got it).
    kills: Vec<(f32, bool)>,
    pub cs_10: Option<u32>,
    pub early_deaths: Option<u32>,
    pub buys: u32,
    pub followed: u32,
}

/// Items in `now` that were not in `before` (multiset difference).
fn new_items(before: &[ItemId], now: &[ItemId]) -> Vec<ItemId> {
    let mut left = before.to_vec();
    now.iter()
        .filter(|id| match left.iter().position(|b| b == *id) {
            Some(i) => {
                left.swap_remove(i);
                false
            }
            None => true,
        })
        .copied()
        .collect()
}

impl GameLog {
    pub fn observe(&mut self, snap: &GameSnapshot, advice: &GameAdvice, sd: &StaticData) {
        let Some(me) = snap.players.get(snap.me) else { return };
        self.shop_on_death = snap.mode != Mode::Rift;
        let now = Moment {
            time: snap.time,
            gold: snap.my_gold,
            owned: me.items.clone(),
            item_gold: snap.players.iter().map(|p| inventory(sd, &p.items).gold).collect(),
            top_threat: advice.threats.first().map(|t| t.champ),
            buy_next: advice.buy_next.clone(),
            alts: advice.alternatives.iter().map(|p| p.item).collect(),
        };
        let my_deaths = || {
            snap.events.iter().filter_map(move |e| match e {
                GameEvent::Kill { time, killer, victim, assisters } if *victim == snap.me => Some((*time, *killer, assisters)),
                _ => None,
            })
        };

        self.kills = snap
            .events
            .iter()
            .filter_map(|e| match e {
                GameEvent::Kill { time, victim, .. } => Some((*time, snap.players.get(*victim)?.team != me.team)),
                _ => None,
            })
            .collect();

        let Some(prev) = self.prev.take() else {
            // First tick (possibly mid-game): baselines only, nothing before it is judged.
            self.first_seen = Some(snap.time);
            self.levels = snap.players.iter().map(|p| p.level).collect();
            self.last_death = my_deaths().map(|d| d.0).fold(0.0, f32::max);
            self.prev = Some(now);
            return;
        };
        if self.levels.len() == snap.players.len() {
            for (i, p) in snap.players.iter().enumerate() {
                for l in SPIKE_LEVELS.into_iter().filter(|&l| self.levels[i] < l && p.level >= l) {
                    self.spikes.push((i, l, snap.time));
                }
                self.levels[i] = p.level;
            }
        }
        if prev.time < CS_AT && snap.time >= CS_AT {
            self.cs_10 = Some(me.cs);
        }
        if prev.time < EARLY_AT && snap.time >= EARLY_AT {
            self.early_deaths = Some(me.deaths);
        }
        for id in new_items(&prev.owned, &me.items) {
            if sd.items.get(&id).is_some_and(|i| i.completed) {
                self.buys += 1;
                self.followed += (prev.buy_next.as_ref().is_some_and(|p| p.item == id) || prev.alts.contains(&id)) as u32;
            }
        }

        let deaths: Vec<_> = my_deaths().filter(|d| d.0 > self.last_death).collect();
        for (time, killer, assisters) in deaths {
            // The tick before the death, unless that is somehow later than it.
            let m = if prev.time <= time { &prev } else { &now };
            let champ = |i: usize| snap.players.get(i).map(|p| p.champ);
            let killer_champ = killer.and_then(champ);
            let involved: Vec<usize> = killer.into_iter().chain(assisters.iter().copied()).collect();
            let mut spikes: Vec<(ChampKey, u32)> = self
                .spikes
                .iter()
                .filter(|(i, _, t)| involved.contains(i) && (-TICK_SLACK..=SPIKE_WINDOW).contains(&(time - t)))
                .filter_map(|&(i, l, _)| Some((champ(i)?, l)))
                .collect();
            spikes.dedup();
            let unspent = m.gold.max(0.0) as u32;
            self.deaths.push(DeathNote {
                time,
                killer: killer_champ,
                assisters: assisters.iter().filter_map(|&i| champ(i)).collect(),
                gold_gap: killer.and_then(|k| Some(*m.item_gold.get(snap.me)? as i32 - *m.item_gold.get(k)? as i32)),
                killer_top_threat: killer_champ.is_some() && killer_champ == m.top_threat,
                unspent,
                buy_next: m.buy_next.as_ref().map(|p| p.item),
                could_buy: m.buy_next.as_ref().is_some_and(|p| p.cost <= unspent || p.component_now.is_some()),
                owned: m.owned.clone(),
                spikes,
                traded: false,
                place: None,
                pos: None,
            });
            self.last_death = self.last_death.max(time);
        }
        self.prev = Some(now);
    }

    /// Whether any tick was seen this game.
    pub fn seen(&self) -> bool {
        self.first_seen.is_some()
    }

    /// Whether the log saw the game from (near) its start.
    fn saw_start(&self) -> bool {
        self.first_seen.is_some_and(|t| t < 90.0)
    }
}

/// My team won the fight around this death: at least as many kills as it lost,
/// this death included. The fight is every kill chained to it less than
/// FIGHT_GAP apart, so a long teamfight is judged whole.
fn traded(kills: &[(f32, bool)], time: f32) -> bool {
    let (mut lo, mut hi) = (time, time);
    loop {
        let near = |t: f32| t >= lo - FIGHT_GAP && t <= hi + FIGHT_GAP;
        let (l, h) = kills.iter().filter(|k| near(k.0)).fold((lo, hi), |(l, h), k| (l.min(k.0), h.max(k.0)));
        if (l, h) == (lo, hi) {
            break;
        }
        (lo, hi) = (l, h);
    }
    let fight = kills.iter().filter(|k| (lo..=hi).contains(&k.0));
    let (won, lost) = fight.fold((0, 0), |(w, l), k| if k.1 { (w + 1, l) } else { (w, l + 1) });
    won > 0 && won >= lost
}

/// In-game patterns from the death notes. Traded deaths (a support peeling,
/// a tank soaking for a won fight) are listed but kept out of the blame notes.
pub fn death_review(log: &GameLog, sd: &StaticData) -> DeathReview {
    review_deaths(&log.deaths, &log.kills, log.shop_on_death, sd)
}

/// `kills`: every champion kill, (game time, my team got it). `shop_on_death`:
/// ARAM / Mayhem, where gold held at death is normal.
pub(crate) fn review_deaths(deaths: &[DeathNote], kills: &[(f32, bool)], shop_on_death: bool, sd: &StaticData) -> DeathReview {
    let all: Vec<DeathNote> = deaths.iter().map(|n| DeathNote { traded: traded(kills, n.time), ..n.clone() }).collect();
    let d: Vec<&DeathNote> = all.iter().filter(|n| !n.traded).collect();
    let mut notes = Vec::new();
    let trades = all.len() - d.len();
    let aside = if trades > 0 { " (trades aside)" } else { "" };

    // Deaths per enemy who had just spiked, and the levels involved.
    let mut by_champ: Vec<(ChampKey, u32, Vec<u32>)> = Vec::new();
    for n in &d {
        let mut counted: Vec<ChampKey> = Vec::new();
        for &(c, l) in &n.spikes {
            let i = by_champ.iter().position(|x| x.0 == c).unwrap_or_else(|| {
                by_champ.push((c, 0, vec![]));
                by_champ.len() - 1
            });
            let e = &mut by_champ[i];
            if !e.2.contains(&l) {
                e.2.push(l);
            }
            if !counted.contains(&c) {
                e.1 += 1;
                counted.push(c);
            }
        }
    }
    by_champ.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (c, n, mut levels) in by_champ.into_iter().filter(|x| x.1 >= 2) {
        levels.sort_unstable();
        levels.dedup();
        let lv = levels.iter().map(u32::to_string).collect::<Vec<_>>().join("/");
        notes.push(format!("Died {n}× within 60 s of {} reaching level {lv}", sd.champ_name(c)));
    }

    // Gold held is worth fixing whether or not the fight was won.
    let holding = all.iter().filter(|n| n.unspent >= HOLDING_GOLD).count();
    if holding > 0 && !shop_on_death {
        let times = if holding == 1 { "once".to_string() } else { format!("{holding}×") };
        notes.push(format!("Died {times} with {HOLDING_GOLD}+ g unspent"));
    }

    // Match history: where on the map, when one place keeps coming up.
    let mut places: Vec<(&str, usize)> = Vec::new();
    for p in d.iter().filter_map(|n| n.place.as_deref()) {
        match places.iter_mut().find(|x| x.0 == p) {
            Some(x) => x.1 += 1,
            None => places.push((p, 1)),
        }
    }
    places.sort_by_key(|p| std::cmp::Reverse(p.1));
    if let Some(&(p, n)) = places.first()
        && (n >= 3 || (n >= 2 && RISKY_PLACES.contains(&p)))
    {
        notes.push(format!("Died {n}× in {}{aside}", the(p)));
    }

    // One enemy doing most of the killing.
    let mut killers: Vec<(ChampKey, usize)> = Vec::new();
    for k in d.iter().filter_map(|n| n.killer) {
        match killers.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 += 1,
            None => killers.push((k, 1)),
        }
    }
    if let Some(&(k, n)) = killers.iter().max_by_key(|x| (x.1, std::cmp::Reverse(x.0)))
        && n >= REPEAT_KILLER
    {
        notes.push(format!("{} killed you {n}×{aside}", sd.champ_name(k)));
    }

    let top = d.iter().filter(|n| n.killer_top_threat).count();
    if top >= 2 {
        notes.push(format!("{top} of {} deaths{aside} were to the enemy on top of the threat board", d.len()));
    }

    let gaps: Vec<i32> = d.iter().filter_map(|n| n.gold_gap).collect();
    if gaps.len() >= 2 {
        let avg = gaps.iter().sum::<i32>() / gaps.len() as i32;
        if avg <= GAP_NOTE {
            notes.push(format!("You had {:.1}k less item gold than your killer on average", -avg as f32 / 1000.0));
        }
    }
    if trades > 0 {
        let n = all.len();
        notes.push(format!("{trades} of {n} deaths were trades: your team won the fight"));
    }
    DeathReview { deaths: all, notes }
}

/// "the enemy jungle", "bot lane".
fn the(place: &str) -> String {
    if place.ends_with("lane") { place.to_string() } else { format!("the {place}") }
}

/// The game's numbers for trends. `ended` is unix seconds.
pub fn summary(log: &GameLog, eog: &EogStats, ended: u64) -> GameSummary {
    let early = log.early_deaths.or_else(|| {
        (eog.duration_s as f32 <= EARLY_AT && log.saw_start()).then(|| log.deaths.iter().filter(|n| n.time < EARLY_AT).count() as u32)
    });
    GameSummary {
        ended,
        mode: eog.mode,
        champ: eog.champ,
        win: eog.win,
        duration_s: eog.duration_s,
        cs_10: log.cs_10,
        deaths_before_14: early,
        vision: eog.vision,
        unspent_at_death: log.deaths.iter().map(|n| n.unspent).collect(),
        buys: log.buys,
        followed: log.followed,
    }
}

/// Where a game stands for goal progress.
pub struct Now {
    pub mode: Mode,
    pub time: f32,
    pub cs: u32,
    pub deaths: u32,
    pub vision: f32,
    /// The game is over: undecided goals get decided.
    pub done: bool,
}

/// Rift-only goals are skipped elsewhere.
pub fn goal_applies(kind: GoalKind, mode: Mode) -> bool {
    mode == Mode::Rift || matches!(kind, GoalKind::Unspent | GoalKind::BuyNext)
}

pub fn goal_progress(goals: &[Goal], log: &GameLog, now: &Now) -> Vec<GoalProgress> {
    let min = (now.time / 60.0).max(1.0 / 60.0);
    goals
        .iter()
        .filter(|g| goal_applies(g.kind, now.mode))
        .map(|g| {
            let t = g.target;
            let (value, met) = match g.kind {
                GoalKind::Cs10 => match log.cs_10 {
                    Some(c) => (Some(c as f32 / 10.0), Some(c as f32 / 10.0 >= t)),
                    None if now.time < CS_AT && !now.done => (Some(now.cs as f32 / min), None),
                    None => (None, None),
                },
                GoalKind::EarlyDeaths => {
                    let n = if now.time < EARLY_AT { Some(now.deaths) } else { log.early_deaths };
                    let n = n.or_else(|| (now.done && log.saw_start()).then_some(now.deaths));
                    let met = n.and_then(|n| {
                        if n as f32 > t {
                            Some(false)
                        } else {
                            (now.time >= EARLY_AT || now.done).then_some(true)
                        }
                    });
                    (n.map(|n| n as f32), met)
                }
                GoalKind::Vision => {
                    let v = now.vision / min;
                    (Some(v), now.done.then_some(v >= t))
                }
                GoalKind::Unspent => {
                    let n = log.deaths.iter().filter(|d| d.unspent as f32 >= t).count();
                    (Some(n as f32), if n > 0 { Some(false) } else { now.done.then_some(true) })
                }
                GoalKind::BuyNext => match log.buys {
                    0 => (None, None),
                    b => {
                        let pct = 100.0 * log.followed as f32 / b as f32;
                        (Some(pct), now.done.then_some(pct >= t))
                    }
                },
            };
            GoalProgress { kind: g.kind, target: t, value, met }
        })
        .collect()
}
