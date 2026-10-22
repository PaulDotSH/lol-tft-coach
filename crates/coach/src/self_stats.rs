//! The player's own numbers vs benchmarks and the lane opponent.

use crate::{common::*, consts::*};
use model::*;

fn role_vision(lane: Lane) -> f32 {
    match lane {
        Lane::Top => VISION_TOP,
        Lane::Jungle => VISION_JUNGLE,
        Lane::Middle => VISION_MID,
        Lane::Bottom => VISION_BOT,
        Lane::Support => VISION_SUPPORT,
    }
}

pub fn self_stats(snap: &GameSnapshot, sd: &StaticData, stats: Option<&ChampStats>, lane_opp: Option<usize>) -> SelfStats {
    let Some(me) = snap.players.get(snap.me) else {
        return SelfStats {
            cs_per_min: 0.0,
            cs_benchmark: None,
            cs_diff: None,
            item_gold_vs_opp: None,
            level_vs_opp: None,
            deaths: 0,
            killers: vec![],
            vision: 0.0,
            vision_benchmark: None,
        };
    };
    let minutes = (snap.time / 60.0).max(1.0 / 60.0);
    let bench = stats.and_then(|s| s.benchmarks.cs_per_min);
    let opp = lane_opp.and_then(|i| snap.players.get(i));
    let mut killers: Vec<(ChampKey, u32)> = Vec::new();
    for e in &snap.events {
        if let GameEvent::Kill { killer: Some(k), victim, .. } = e
            && *victim == snap.me
            && let Some(p) = snap.players.get(*k)
        {
            match killers.iter_mut().find(|x| x.0 == p.champ) {
                Some(x) => x.1 += 1,
                None => killers.push((p.champ, 1)),
            }
        }
    }
    killers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let vision_pm = match snap.mode {
        Mode::Rift => stats
            .and_then(|s| s.benchmarks.vision_per_min)
            .or_else(|| me.position.or(stats.and_then(|s| s.lane)).map(role_vision)),
        _ => None,
    };
    let gold = |p: &Player| inventory(sd, &p.items).gold as i32;
    SelfStats {
        cs_per_min: me.cs as f32 / minutes,
        cs_benchmark: bench,
        cs_diff: bench.map(|b| me.cs as f32 - b * minutes),
        item_gold_vs_opp: opp.map(|o| gold(me) - gold(o)),
        level_vs_opp: opp.map(|o| me.level as i32 - o.level as i32),
        deaths: me.deaths,
        killers,
        vision: me.ward_score,
        vision_benchmark: vision_pm.map(|v| v * minutes),
    }
}
