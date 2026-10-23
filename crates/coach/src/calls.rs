//! Live calls on top of the advice: who is worth a shutdown (kill streaks and
//! item gold, never hidden gold), whether my team should fight right now, and
//! short calls tying objective timers to my gold and the death timers. Facts
//! the scoreboard shows; nothing about cooldowns.

use crate::common::inventory;
use model::*;

/// Kill streak / deaths in a row worth a bounty line.
const STREAK: u32 = 3;
const COLD: u32 = 3;
/// Item gold over the team average worth a shutdown line.
const ITEM_LEAD: i32 = 1500;
/// Back for an objective spawning this far ahead (s).
const BACK_FROM: f32 = 45.0;
const BACK_TO: f32 = 100.0;
/// Gold that makes a back worth it when Buy next is still out of reach.
const BACK_GOLD: f32 = 1100.0;
/// Objective spawning this soon (s) counts as up for "take it".
const TAKE_SOON: f32 = 20.0;
/// Dead players count for fights and objectives with at least this long left (s).
const DEAD_FOR: f32 = 12.0;
/// Readiness: per dead player, and the score that calls a fight or avoids one.
const DEAD_W: f32 = 0.7;
const DECISIVE: f32 = 1.5;
/// Readiness waits for the first levels and items.
const READY_FROM: f32 = 180.0;
/// My next item counts as a spike to wait for when it is this close (s).
const SPIKE_SOON: f32 = 60.0;

pub fn annotate(a: &mut GameAdvice, snap: &GameSnapshot, sd: &StaticData) {
    a.bounties = bounties(snap, sd);
    a.readiness = readiness(a, snap, sd);
    a.calls = calls(a, snap, sd);
}

/// (kills since last death, deaths since last kill) per player.
fn streaks(snap: &GameSnapshot) -> Vec<(u32, u32)> {
    let mut s = vec![(0, 0); snap.players.len()];
    for e in &snap.events {
        if let GameEvent::Kill { killer, victim, .. } = e {
            if let Some(k) = killer.and_then(|k| s.get_mut(k)) {
                *k = (k.0 + 1, 0);
            }
            if let Some(v) = s.get_mut(*victim) {
                *v = (0, v.1 + 1);
            }
        }
    }
    s
}

fn bounties(snap: &GameSnapshot, sd: &StaticData) -> Vec<Bounty> {
    let Some(me) = snap.players.get(snap.me) else { return vec![] };
    let gold: Vec<i32> = snap.players.iter().map(|p| inventory(sd, &p.items).gold as i32).collect();
    let avg = |team: Team| {
        let g: Vec<i32> = snap.players.iter().zip(&gold).filter(|(p, _)| p.team == team).map(|(_, g)| *g).collect();
        g.iter().sum::<i32>() / g.len().max(1) as i32
    };
    let mut out: Vec<Bounty> = snap
        .players
        .iter()
        .zip(streaks(snap))
        .enumerate()
        .filter_map(|(i, (p, (streak, cold)))| {
            let item_lead = gold[i] - avg(p.team);
            let mine = i == snap.me;
            let text = if streak >= STREAK {
                format!("{streak}-kill streak: big shutdown")
            } else if item_lead >= ITEM_LEAD {
                format!("{:.1}k item gold over their team: shutdown", item_lead as f32 / 1000.0)
            } else if cold >= COLD {
                format!("{cold} deaths in a row: worth little")
            } else {
                return None;
            };
            let text = if mine { format!("You: {text}{}", if streak >= STREAK { ", don't die alone" } else { "" }) } else { text };
            Some(Bounty { champ: p.champ, ally: p.team == me.team, me: mine, streak, cold, item_lead, text })
        })
        .collect();
    out.sort_by_key(|b| (b.ally, b.me, std::cmp::Reverse(b.streak), std::cmp::Reverse(b.item_lead)));
    out
}

fn kilo(g: i32) -> String {
    let s = format!("{:.1}k", g.abs() as f32 / 1000.0);
    if g >= 0 { format!("+{s}") } else { format!("−{s}") }
}

fn readiness(a: &GameAdvice, snap: &GameSnapshot, sd: &StaticData) -> Option<Readiness> {
    if snap.time < READY_FROM {
        return None;
    }
    let dead = |v: &[Respawn]| v.iter().filter(|r| r.in_s >= DEAD_FOR).map(|r| r.champ).collect::<Vec<_>>();
    let (enemies, allies) = (dead(&a.deaths.enemies), dead(&a.deaths.allies));
    let g = &a.gold;
    let score = g.diff as f32 / 1000.0 + g.level_diff + DEAD_W * (enemies.len() as f32 - allies.len() as f32);
    let mut reasons = Vec::new();
    if g.diff.abs() >= 500 {
        reasons.push(format!("{} item gold", kilo(g.diff)));
    }
    if g.level_diff.abs() >= 0.4 {
        reasons.push(format!("{}{:.1} levels on average", if g.level_diff > 0.0 { "+" } else { "−" }, g.level_diff.abs()));
    }
    let names = |v: &[ChampKey]| v.iter().map(|&c| sd.champ_name(c)).collect::<Vec<_>>().join(", ");
    match enemies.len() {
        0 => {}
        1 => reasons.push(format!("{} is dead", names(&enemies))),
        n => reasons.push(format!("{n} enemies dead ({})", names(&enemies))),
    }
    match allies.len() {
        0 => {}
        1 => reasons.push(format!("{} on your team is dead", names(&allies))),
        n => reasons.push(format!("{n} of your team dead")),
    }
    let verdict = if score >= DECISIVE {
        Verdict::Fight
    } else if score <= -DECISIVE {
        Verdict::Avoid
    } else {
        Verdict::Even
    };
    let mut text = match verdict {
        Verdict::Fight => "Your team is stronger right now: look for a fight or an objective".to_string(),
        Verdict::Even => "Even: fight only with numbers or near your side".to_string(),
        Verdict::Avoid => "Their team is stronger right now: avoid fights, farm safely".to_string(),
    };
    // My next item close: worth waiting for when the fight is not already good.
    if verdict != Verdict::Fight
        && let Some(e) = &a.gold_eta
        && let (Some(item), Some(eta)) = (e.item, e.eta_s)
        && e.missing > 0
        && eta <= SPIKE_SOON
    {
        let name = sd.item_name(item);
        reasons.push(format!("you can afford {name} in ~{eta:.0} s"));
        text = format!("{text}; or back for {name} first");
    }
    Some(Readiness { verdict, text, reasons })
}

fn clock(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn calls(a: &GameAdvice, snap: &GameSnapshot, sd: &StaticData) -> Vec<Call> {
    let Some(me) = snap.players.get(snap.me) else { return vec![] };
    let mut out = Vec::new();
    let enemies_dead = a.deaths.enemies.iter().filter(|r| r.in_s >= DEAD_FOR).count();
    let allies_dead = a.deaths.allies.iter().filter(|r| r.in_s >= DEAD_FOR).count() + (me.dead && me.respawn_in >= DEAD_FOR) as usize;
    for t in &a.timers {
        let left = t.next_spawn.map(|s| s - snap.time);
        let at = t.next_spawn.unwrap_or(snap.time) as u32;
        // Back now to be on time for it.
        if let Some(dt) = left.filter(|dt| (BACK_FROM..=BACK_TO).contains(dt))
            && !me.dead
        {
            let buy = a.gold_eta.as_ref().filter(|e| e.missing == 0).and_then(|e| e.item);
            if buy.is_some() || snap.my_gold >= BACK_GOLD {
                let what = match buy {
                    Some(i) => format!("buy {}", sd.item_name(i)),
                    None => format!("spend {:.0} gold", snap.my_gold),
                };
                out.push(Call {
                    key: format!("back-{}-{at}", t.name),
                    text: format!("{} in {}: back now, {what}, and be there", t.name, clock(dt)),
                    say: format!("{} in {} seconds. Back and buy now.", t.name, (dt / 10.0).round() * 10.0),
                });
            }
        }
        // Up (or nearly) with enemies dead: take it.
        if left.is_none_or(|dt| dt <= TAKE_SOON) && enemies_dead >= 2 && enemies_dead >= allies_dead + 2 {
            let when = match left {
                Some(dt) if dt > 0.0 => format!("spawns in {}", clock(dt)),
                _ => "is up".to_string(),
            };
            out.push(Call {
                key: format!("take-{}-{at}-{enemies_dead}", t.name),
                text: format!("{} {when} and {enemies_dead} enemies are dead: take it", t.name),
                say: format!("{enemies_dead} enemies dead. Take {}.", t.name),
            });
        }
    }
    if let Some(b) = a.bounties.iter().find(|b| b.me && b.streak >= STREAK) {
        out.push(Call {
            key: format!("me-streak-{}", b.streak),
            text: format!("You're on a {}-kill streak: worth a big shutdown, don't die alone", b.streak),
            say: "You're a big shutdown. Play safe.".into(),
        });
    }
    out
}
