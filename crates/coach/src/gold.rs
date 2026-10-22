//! Item-gold diff: team, lanes, players, and its sampled history over the game.
//! Item gold = cost of held items (what the scoreboard shows); enemy gold itself
//! is hidden by the game and never estimated.

use crate::common::*;
use model::*;
use std::cmp::Reverse;

/// Game seconds between history samples (doubles each time the history fills up).
pub const SAMPLE_STEP: f32 = 20.0;
/// Most samples kept; the history covers the whole game at halving resolution.
pub const HISTORY_MAX: usize = 90;

pub fn gold_diff(
    snap: &GameSnapshot,
    sd: &StaticData,
    lane_opp: Option<usize>,
    my_lane: Option<Lane>,
    lane_now: Option<i32>,
    history: &[GoldSample],
) -> GoldDiff {
    let my_team = snap.players.get(snap.me).map_or(Team::Order, |p| p.team);
    let gold: Vec<u32> = snap.players.iter().map(|p| inventory(sd, &p.items).gold).collect();
    let (mut tg, mut kills, mut lvl, mut n) = ([0u32; 2], [0u32; 2], [0f32; 2], [0f32; 2]);
    for (p, g) in snap.players.iter().zip(&gold) {
        let t = usize::from(p.team != my_team);
        tg[t] += g;
        kills[t] += p.kills;
        lvl[t] += p.level as f32;
        n[t] += 1.0;
    }
    let diff = tg[0] as i32 - tg[1] as i32;
    let lanes = match snap.mode {
        Mode::Rift => pair_lanes(snap, my_team, lane_opp, my_lane)
            .into_iter()
            .map(|(lane, a, e)| {
                let (pa, pe) = (&snap.players[a], &snap.players[e]);
                LaneDiff {
                    lane,
                    ally: pa.champ,
                    enemy: pe.champ,
                    gold_diff: gold[a] as i32 - gold[e] as i32,
                    level_diff: pa.level as i32 - pe.level as i32,
                    mine: a == snap.me,
                }
            })
            .collect(),
        _ => vec![],
    };
    let mut players: Vec<PlayerGold> = snap
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| PlayerGold { champ: p.champ, ally: p.team == my_team, item_gold: gold[i], level: p.level, mine: i == snap.me })
        .collect();
    players.sort_by_key(|p| (!p.ally, Reverse(p.item_gold)));
    let mut history = history.to_vec();
    history.push(GoldSample { time: snap.time, team: diff, lane: lane_now });
    GoldDiff {
        team_item_gold: (tg[0], tg[1]),
        diff,
        kills: (kills[0], kills[1]),
        level_diff: lvl[0] / n[0].max(1.0) - lvl[1] / n[1].max(1.0),
        lanes,
        players,
        history,
    }
}

/// (lane, ally, enemy) player indices: same `position` first, then me vs my
/// lane opponent (coach's fallback), then the rest in scoreboard order, which
/// is how the in-game scoreboard lines the teams up side by side.
fn pair_lanes(snap: &GameSnapshot, my_team: Team, lane_opp: Option<usize>, my_lane: Option<Lane>) -> Vec<(Option<Lane>, usize, usize)> {
    let ps = &snap.players;
    let mut used = vec![false; ps.len()];
    let free = |used: &[bool], ally: bool, lane: Option<Lane>| {
        (0..ps.len()).find(|&i| !used[i] && (ps[i].team == my_team) == ally && (lane.is_none() || ps[i].position == lane))
    };
    let mut pairs = Vec::with_capacity(5);
    let mut take = |used: &mut [bool], lane, a: usize, e: usize| {
        used[a] = true;
        used[e] = true;
        pairs.push((lane, a, e));
    };
    for lane in Lane::ALL {
        if let (Some(a), Some(e)) = (free(&used, true, Some(lane)), free(&used, false, Some(lane))) {
            take(&mut used, Some(lane), a, e);
        }
    }
    if let Some(o) = lane_opp
        && snap.me < ps.len()
        && o < ps.len()
        && !used[snap.me]
        && !used[o]
    {
        take(&mut used, ps[snap.me].position.or(my_lane).or(ps[o].position), snap.me, o);
    }
    while let (Some(a), Some(e)) = (free(&used, true, None), free(&used, false, None)) {
        take(&mut used, None, a, e);
    }
    pairs.sort_by_key(|p| p.0.map_or(Lane::ALL.len(), |l| l as usize));
    pairs
}

/// Per-game sampled history: one sample per `step` game seconds, at most
/// `HISTORY_MAX`; when full, every other sample is dropped and `step` doubles.
#[derive(Debug, Clone, Default)]
pub struct GoldHistory {
    pub samples: Vec<GoldSample>,
    step: f32,
}

impl GoldHistory {
    pub fn push(&mut self, s: GoldSample) {
        if self.samples.last().is_some_and(|l| s.time < l.time) {
            *self = GoldHistory::default(); // clock went back: a different game
        }
        if self.step == 0.0 {
            self.step = SAMPLE_STEP;
        }
        if self.samples.last().is_some_and(|l| s.time - l.time < self.step) {
            return;
        }
        if self.samples.len() >= HISTORY_MAX {
            let mut keep = false;
            self.samples.retain(|_| {
                keep = !keep;
                keep
            });
            self.step *= 2.0;
        }
        self.samples.push(s);
    }
}
