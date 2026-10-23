//! Fights and objectives in a past game: kill chains with their result and my
//! part in them, and where I was when each epic monster died. Positions come
//! from one-minute frames, so "there" means in the fight or close to it.

use crate::analysis::place;
use model::*;

/// Kills at most this far apart (s) chain into one fight, as in the live review.
const FIGHT_GAP: f32 = 10.0;
/// A frame this close in time (s) says where I was.
const FRAME_NEAR: f32 = 35.0;
/// Within this distance of a fight or objective counts as there.
const NEAR: f32 = 3500.0;
/// An objective taken this soon (s) after a fight follows from it.
const THEN: f32 = 60.0;
/// Base respawn time by level (s), before the late-game increase.
const RESPAWN: [f32; 18] = [10.0, 10.0, 12.0, 12.0, 14.0, 16.0, 20.0, 25.0, 28.0, 32.5, 35.0, 37.5, 40.0, 42.5, 45.0, 47.5, 50.0, 52.5];

/// Seconds dead at `level` dying at game time `t`: the base grows after 15:00,
/// a bit faster after 30:00 and 45:00, up to +50 %.
pub(crate) fn respawn(level: u32, t: f32) -> f32 {
    let base = RESPAWN[(level.clamp(1, 18) - 1) as usize];
    let halves = |from: f32, to: f32| ((t.min(to) - from) / 30.0).max(0.0);
    let grow = 0.00425 * halves(900.0, 1800.0) + 0.003 * halves(1800.0, 2700.0) + 0.0145 * halves(2700.0, 3300.0);
    base * (1.0 + grow.min(0.5))
}

fn dist(a: (i32, i32), b: (i32, i32)) -> f32 {
    (((a.0 - b.0) as f32).powi(2) + ((a.1 - b.1) as f32).powi(2)).sqrt()
}

/// I was dead at game time `t`.
fn dead_at(d: &MatchDetail, t: f32) -> bool {
    d.kills.iter().filter(|k| k.victim == d.me && k.time <= t).any(|k| {
        let level = d.frames.iter().rev().find(|f| f.time <= k.time).map_or(1, |f| f.players[d.me].level);
        t < k.time + respawn(level, k.time)
    })
}

/// Near `pos` around `t` going by the closest frames before and after.
fn near(d: &MatchDetail, t: f32, pos: (i32, i32)) -> Option<bool> {
    let close: Vec<(i32, i32)> =
        d.frames.iter().filter(|f| (f.time - t).abs() <= FRAME_NEAR).filter_map(|f| f.players[d.me].pos).collect();
    (!close.is_empty()).then(|| close.iter().any(|p| dist(*p, pos) <= NEAR))
}

fn in_kill(k: &KillEvent, me: usize) -> bool {
    k.victim == me || k.killer == Some(me) || k.assisters.contains(&me)
}

/// Middle of a set of positions.
fn center(ps: &[(i32, i32)]) -> Option<(i32, i32)> {
    let n = ps.len() as i32;
    (n > 0).then(|| (ps.iter().map(|p| p.0).sum::<i32>() / n, ps.iter().map(|p| p.1).sum::<i32>() / n))
}

pub(crate) fn fights(d: &MatchDetail, marks: &[TimelineMark]) -> Vec<Fight> {
    let blue = d.players.first().is_some_and(|p| p.ally);
    let mut chains: Vec<Vec<&KillEvent>> = Vec::new();
    for k in &d.kills {
        match chains.last_mut() {
            Some(c) if k.time - c.last().map_or(f32::MIN, |l| l.time) <= FIGHT_GAP => c.push(k),
            _ => chains.push(vec![k]),
        }
    }
    chains
        .into_iter()
        .filter(|c| c.len() >= 2 || c.iter().any(|k| in_kill(k, d.me)))
        .map(|c| {
            let (start, end) = (c[0].time, c[c.len() - 1].time);
            let lost = c.iter().filter(|k| d.players[k.victim].ally).count() as u32;
            let at = center(&c.iter().filter_map(|k| k.pos).collect::<Vec<_>>());
            let presence = if c.iter().any(|k| in_kill(k, d.me)) {
                Presence::There
            } else if dead_at(d, start) {
                Presence::Dead
            } else if at.and_then(|p| near(d, start, p)) == Some(true) {
                Presence::There
            } else {
                Presence::Away
            };
            let died_first = c.iter().find(|k| d.players[k.victim].ally).is_some_and(|k| k.victim == d.me);
            Fight {
                start,
                end,
                won: c.len() as u32 - lost,
                lost,
                place: at.filter(|_| d.game.mode == Mode::Rift).map(|(x, y)| place(MapZone::at(x, y), blue).into()),
                presence,
                died_first,
                then: marks.iter().find(|m| m.kind != "Tower" && m.kind != "Inhibitor" && m.time >= start && m.time - end <= THEN).cloned(),
            }
        })
        .collect()
}

/// Epic monster marks with where I was: helped kill it, dead, near it, or elsewhere.
pub(crate) fn with_presence(d: &MatchDetail) -> Vec<TimelineMark> {
    d.marks
        .iter()
        .map(|m| {
            let epic = m.kind != "Tower" && m.kind != "Inhibitor";
            let presence = if !epic {
                None
            } else if m.mine {
                Some(Presence::There)
            } else if dead_at(d, m.time) {
                Some(Presence::Dead)
            } else {
                let fought = m.pos.is_some_and(|p| {
                    d.kills.iter().any(|k| in_kill(k, d.me) && (k.time - m.time).abs() <= 20.0 && k.pos.is_some_and(|q| dist(p, q) <= NEAR))
                });
                if fought {
                    Some(Presence::There)
                } else {
                    m.pos.and_then(|p| near(d, m.time, p)).map(|n| if n { Presence::There } else { Presence::Away })
                }
            };
            TimelineMark { presence, ..m.clone() }
        })
        .collect()
}
