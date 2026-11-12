//! When I jungle: clear pace, the lane board (which lanes are open to a gank and why), the objective back tip, and
//! the jungle section of the match review. Options with reasons, never orders; never the enemy jungler in fog.

use crate::analysis::clock;
use crate::brief::thousands;
use crate::consts::*;
use crate::tracker::Tracker;
use model::*;

mod lanes;
mod review;

pub use lanes::{Board, board, lanes};
pub(crate) use review::review;

pub fn is_jungler(p: &Player) -> bool {
    p.smite || p.position == Some(Lane::Jungle)
}

fn behind(levels: u32) -> String {
    match levels {
        1 => "one camp behind".into(),
        2 => "two camps behind".into(),
        n => format!("{n} camps behind"),
    }
}

pub(crate) fn pace(snap: &GameSnapshot, me: &SelfStats, tracker: &Tracker) -> Option<String> {
    let p = snap.players.get(snap.me)?;
    let t = snap.time;
    if t < JG_LEVELS_UNTIL {
        if snap.swiftplay {
            return None;
        }
        let (want, by) = if t >= JG_L4_BY || p.level >= 4 { (4, JG_L4_BY) } else { (3, JG_L3_BY) };
        if p.level >= want {
            let at = tracker.reached(snap.me, want).unwrap_or(t);
            let verdict = if at <= by { "on pace".to_string() } else { behind(1) };
            return Some(format!("Level {want} at {}: {verdict}", clock(at)));
        }
        return (t >= by).then(|| format!("Level {} at {}: {}", p.level, clock(t), behind(want - p.level)));
    }
    if t > JG_CS_UNTIL {
        return None;
    }
    let (avg, diff) = (me.cs_benchmark?, me.cs_diff?);
    Some(if diff <= -JG_CS_BEHIND {
        format!("CS {:.0} behind the average pace ({:.1}/min vs {avg:.1}): take your camps between ganks", -diff, me.cs_per_min)
    } else if diff >= JG_CS_BEHIND {
        format!("CS {diff:.0} ahead of the average pace ({:.1}/min vs {avg:.1})", me.cs_per_min)
    } else {
        format!("CS on pace ({:.1}/min vs {avg:.1})", me.cs_per_min)
    })
}

fn objective_tip(a: &GameAdvice, snap: &GameSnapshot, sd: &StaticData) -> Option<LiveTip> {
    if snap.players.get(snap.me).is_none_or(|p| p.dead) {
        return None;
    }
    let t = a.timers.iter().find(|t| t.name == "Dragon")?;
    let left = t.next_spawn? - snap.time;
    if !(0.0..=JG_OBJECTIVE_TIP_S).contains(&left) {
        return None;
    }
    let e = a.gold_eta.as_ref()?;
    let item = sd.item_name(e.item?);
    let gold = thousands(snap.my_gold.max(0.0) as u32);
    let text = if e.missing == 0 {
        format!("{} in {}: back first: {gold} g buys {item}", t.name, clock(left))
    } else {
        format!("{} in {}: no full item yet, {} g short of {item}", t.name, clock(left), thousands(e.missing))
    };
    Some(LiveTip { key: "jg-objective".into(), kind: "back".into(), text })
}

pub(crate) fn annotate(a: &mut GameAdvice, snap: &GameSnapshot, sd: &StaticData, tracker: &Tracker) {
    a.jungle = None;
    let Some(me) = snap.players.get(snap.me) else { return };
    if snap.mode != Mode::Rift || !is_jungler(me) {
        return;
    }
    if let Some(tip) = objective_tip(a, snap, sd) {
        a.tips.push(tip);
    }
    let lanes = board(snap, sd, &a.gold).map_or(vec![], |b| lanes(&b, None));
    a.jungle = Some(JungleAdvice { pace: pace(snap, &a.me, tracker), lanes, minimap: false, camps: vec![] });
}
