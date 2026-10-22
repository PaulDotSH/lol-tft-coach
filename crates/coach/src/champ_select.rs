//! Champ select: counter-picks, bans, build plan, ARAM bench ranking.

use crate::{ChampSelectInput, common::*, consts::*, items, needs, threat};
use model::*;

/// Percent from a percent or a fraction.
fn pct(x: f32) -> f32 {
    if x <= 1.0 { x * 100.0 } else { x }
}

pub fn advice(inp: &ChampSelectInput) -> ChampSelectAdvice {
    let cs = inp.cs;
    let mine = cs.my_pick.or(cs.my_hover);
    let rift = cs.mode == Mode::Rift;
    ChampSelectAdvice {
        mode: cs.mode,
        my_lane: cs.my_lane,
        my_champ: mine,
        picks: if rift { picks(inp, mine) } else { vec![] },
        bans: if rift { bans(inp, mine) } else { vec![] },
        build: mine.and_then(|c| build_plan(inp, c)),
        bench: if rift { vec![] } else { bench(inp, mine) },
        enemies: cs.enemies.clone(),
    }
}

fn lane_ok(inp: &ChampSelectInput, t: &TierEntry) -> bool {
    inp.cs.my_lane.is_none_or(|l| t.lane == l) && t.games >= TIER_MIN_GAMES
}

fn top(mut v: Vec<PickSuggestion>, n: usize) -> Vec<PickSuggestion> {
    v.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.champ.cmp(&b.champ)));
    v.dedup_by_key(|p| p.champ);
    v.truncate(n);
    v
}

fn picks(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Vec<PickSuggestion> {
    let (cs, sd) = (inp.cs, inp.sd);
    let taken = |c: ChampKey| cs.bans.contains(&c) || cs.enemies.contains(&c) || (cs.allies.contains(&c) && Some(c) != mine);
    let revealed: Vec<(&ChampStats, f32, bool)> = cs
        .enemies
        .iter()
        .filter_map(|e| inp.enemies.iter().find(|s| s.champ == *e))
        .map(|s| {
            let lane = cs.my_lane.is_some() && s.lane == cs.my_lane;
            (s, if lane { LANE_OPP_W } else { OTHER_ENEMY_W }, lane)
        })
        .collect();
    let lane_name = cs.my_lane.map_or("your role", Lane::as_str);
    let v = inp
        .tier_list
        .iter()
        .filter(|t| lane_ok(inp, t) && !taken(t.champ))
        .map(|t| {
            let base = shrink(pct(t.win_rate), t.games, TIER_SHRINK_K);
            let (mut sw, mut s, mut best) = (0.0, 0.0, None::<(f32, f32, &ChampStats, bool)>);
            for &(es, w, lane) in &revealed {
                let Some(m) = es.counters.iter().find(|m| m.champ == t.champ && m.games >= MATCHUP_MIN_GAMES) else {
                    continue;
                };
                let ours = 100.0 - shrink(pct(m.vs_wr), m.games, MATCHUP_SHRINK_K);
                sw += w;
                s += w * ours;
                let key = w * (ours - 50.0);
                if best.is_none_or(|b| key > b.0) {
                    best = Some((key, 100.0 - pct(m.vs_wr), es, lane));
                }
            }
            let score = if sw > 0.0 { MATCHUP_BLEND * s / sw + (1.0 - MATCHUP_BLEND) * base } else { base };
            let reason = match best {
                Some((k, wr, es, lane)) if k > 0.0 => {
                    format!("{wr:.1}% vs {}{}", sd.champ_name(es.champ), if lane { " (lane)" } else { "" })
                }
                _ => format!("{:.1}% win rate in {lane_name}", pct(t.win_rate)),
            };
            PickSuggestion { champ: t.champ, score, reason }
        })
        .collect();
    top(v, PICKS)
}

fn bans(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Vec<PickSuggestion> {
    let (cs, sd) = (inp.cs, inp.sd);
    let taken = |c: ChampKey| {
        cs.bans.contains(&c) || cs.enemies.contains(&c) || cs.allies.contains(&c) || Some(c) == mine
    };
    // champ → (score, best part, reason)
    let mut acc: Vec<(ChampKey, f32, f32, String)> = Vec::new();
    let mut add = |c: ChampKey, v: f32, why: String| match acc.iter_mut().find(|a| a.0 == c) {
        Some(a) => {
            a.1 += v;
            if v > a.2 {
                (a.2, a.3) = (v, why);
            }
        }
        None => acc.push((c, v, v, why)),
    };
    if let Some(m) = inp.mine.filter(|m| Some(m.champ) == mine) {
        let max_g = m.counters.iter().map(|c| c.games).max().unwrap_or(1).max(1) as f32;
        for c in m.counters.iter().filter(|c| c.games >= MATCHUP_MIN_GAMES) {
            let lane_w = match (cs.my_lane, c.default_lane) {
                (Some(a), Some(b)) if a != b => BAN_OFFLANE_W,
                _ => 1.0,
            };
            let wr = shrink(pct(c.vs_wr), c.games, MATCHUP_SHRINK_K);
            let v = BAN_COUNTER_W * (50.0 - wr) * (c.games as f32 / max_g).sqrt() * lane_w;
            if v > 0.0 {
                let why = format!(
                    "counters {}: {:.1}% for you over {} games",
                    sd.champ_name(m.champ),
                    pct(c.vs_wr),
                    fmt_games(c.games as u64)
                );
                add(c.champ, v, why);
            }
        }
    }
    let lane: Vec<&TierEntry> = inp.tier_list.iter().filter(|t| lane_ok(inp, t)).collect();
    let max_pr = lane.iter().map(|t| t.pick_rate).fold(0.0, f32::max).max(f32::EPSILON);
    for t in lane {
        let wr = shrink(pct(t.win_rate), t.games, TIER_SHRINK_K);
        let v = BAN_META_W * (wr - 50.0) * (t.pick_rate / max_pr).sqrt();
        if v > 0.0 {
            add(t.champ, v, format!("meta threat: {:.1}% win, {:.1}% pick", pct(t.win_rate), t.pick_rate));
        }
    }
    let v = acc
        .into_iter()
        .filter(|a| !taken(a.0))
        .map(|(champ, score, _, reason)| PickSuggestion { champ, score, reason })
        .collect();
    top(v, BANS)
}

fn build_plan(inp: &ChampSelectInput, champ_key: ChampKey) -> Option<BuildPlan> {
    let mine = inp.mine.filter(|m| m.champ == champ_key)?;
    let sd = inp.sd;
    let foes: Vec<threat::Foe> = inp.cs.enemies.iter().map(|&e| threat::generic_foe(champ(sd, e))).collect();
    let needs = needs::needs(&foes);
    let ctx = items::Ctx::new(sd, Some(mine), champ(sd, champ_key), inp.cs.mode);
    let b = items::build(&ctx, &[], &needs);
    let is_boots = |id: ItemId| sd.items.get(&id).is_some_and(|i| i.boots);
    let pick = |s: &items::Scored| items::to_pick(&ctx, s, &needs, &foes, None);
    let start = mine
        .early
        .iter()
        .max_by(|a, b| wilson_p(frac(a.win_rate), a.games).total_cmp(&wilson_p(frac(b.win_rate), b.games)))
        .map(|e| e.items.clone())
        .unwrap_or_default();
    Some(BuildPlan {
        start,
        core: b.path.iter().filter(|s| !is_boots(s.id)).take(CORE_ITEMS).map(pick).collect(),
        boots: b.path.iter().find(|s| is_boots(s.id)).map(pick),
    })
}

fn bench(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Vec<BenchRank> {
    let cs = inp.cs;
    let mut list: Vec<ChampKey> = Vec::with_capacity(cs.bench.len() + 1);
    for c in mine.into_iter().chain(cs.bench.iter().copied()) {
        if !list.contains(&c) {
            list.push(c);
        }
    }
    let entry = |c: ChampKey| inp.tier_list.iter().find(|t| t.champ == c);
    let tier = |c: ChampKey| match (cs.mode, inp.mayhem) {
        (Mode::Mayhem, Some(m)) => m.champ_tiers.get(&c).cloned(),
        _ => entry(c).and_then(|t| t.tier.clone()),
    };
    match (cs.mode, inp.mayhem) {
        (Mode::Mayhem, Some(_)) => list.sort_by_key(|&c| tier(c).map_or(100, |t| tier_rank(&t))),
        _ => list.sort_by(|&a, &b| {
            let wr = |c| entry(c).map_or(f32::NEG_INFINITY, |t| pct(t.win_rate));
            wr(b).total_cmp(&wr(a))
        }),
    }
    list.into_iter().map(|c| BenchRank { champ: c, tier: tier(c), mine: Some(c) == mine }).collect()
}
