//! Champ select: counter-picks, bans, build plan (ARAM bench and team comp: `team`).

use crate::{ChampSelectInput, EnemyVs, common::*, consts::*, draft, items, lane_matchup::{self, MIN_MATCHUP_GAMES}, needs, runes, team, threat, vs_build};
use model::*;

/// Percent from a percent or a fraction.
pub(crate) fn pct(x: f32) -> f32 {
    if x <= 1.0 { x * 100.0 } else { x }
}

pub fn advice(inp: &ChampSelectInput) -> ChampSelectAdvice {
    let cs = inp.cs;
    let mine = cs.my_pick.or(cs.my_hover);
    let rift = cs.mode == Mode::Rift;
    let draft = if rift { draft::report(inp) } else { None };
    let faced = if rift { faced(inp) } else { vec![] };
    // The enemy placed in my own lane.
    let opp = faced.iter().copied().find(|e| Some(e.lane) == cs.my_lane);
    let stats = mine.and_then(|c| inp.mine.filter(|m| m.champ == c));
    let pages: Vec<&MatchupBuild> = inp.pages.iter().copied().filter(|p| cs.enemies.contains(&p.vs)).collect();
    let lane_vs: Vec<ChampKey> = faced.iter().map(|e| e.champ).collect();
    let build = stats.and_then(|m| {
        let source = if pages.is_empty() { build_source(Some(m)) } else { format!("{} + lolalytics matchup pages", build_source(Some(m))) };
        build_plan(inp, &vs_build::game(m, &pages), &cs.enemies, pages.iter().map(|p| p.vs).collect(), source)
    });
    // Against my direct opponent: my own lane's, else (their support unknown) their bot laner.
    let page = opp.or(faced.first().copied()).and_then(|e| pages.iter().find(|p| p.vs == e.champ && p.games >= MIN_MATCHUP_GAMES));
    let lane_build = stats.zip(page).and_then(|(m, p)| {
        let s = vs_build::lane(m, p);
        let source = format!("{} · {} games", lolalytics_source(&s), fmt_games(p.games as u64));
        build_plan(inp, &s, &lane_vs, vec![p.vs], source)
    });
    ChampSelectAdvice {
        mode: cs.mode,
        my_lane: cs.my_lane,
        my_champ: mine,
        picks: if rift { picks(inp, mine) } else { vec![] },
        lane_picks: lane_picks(inp, mine, &faced),
        lane_vs,
        bans: if rift { bans(inp, mine) } else { vec![] },
        build,
        lane_build,
        runes: rune_pages(inp, None),
        bench: if rift { vec![] } else { team::bench(inp, mine) },
        enemies: cs.enemies.clone(),
        draft,
        team: if rift { None } else { team::side(inp, mine) },
        memory: vec![],
        matchup: None,
        tiers: if rift { tier_board(inp, mine, opp) } else { None },
    }
}

/// The enemies placed in the lanes I face (both bot lanes for a support or bot laner), bot laner first.
fn faced<'a>(inp: &'a ChampSelectInput) -> Vec<&'a EnemyVs<'a>> {
    let lanes = inp.cs.my_lane.map_or(&[][..], lane_matchup::opposing);
    lanes.iter().filter_map(|&l| inp.vs.iter().find(|e| e.lane == l)).collect()
}

fn taken(cs: &ChampSelect, mine: Option<ChampKey>, c: ChampKey) -> bool {
    cs.bans.contains(&c) || cs.enemies.contains(&c) || (cs.allies.contains(&c) && Some(c) != mine)
}

/// An enemy's row against champion `c`, with enough games to count.
fn row(table: &[Matchup], c: ChampKey) -> Option<&Matchup> {
    table.iter().find(|m| m.champ == c && m.games >= MATCHUP_MIN_GAMES)
}

/// My lane's tier list in its order, each champion with its win rate against the enemy whose
/// win rates against them are `theirs`; banned and taken champions dropped.
fn tier_rows(inp: &ChampSelectInput, mine: Option<ChampKey>, theirs: &[Matchup]) -> Vec<TierPick> {
    let pick = |t: &TierEntry| {
        let m = row(theirs, t.champ);
        TierPick {
            champ: t.champ,
            tier: t.tier.clone(),
            win_rate: pct(t.win_rate),
            pick_rate: t.pick_rate,
            vs_wr: m.map(|m| 100.0 - pct(m.vs_wr)),
            vs_games: m.map_or(0, |m| m.games),
            mine: Some(t.champ) == mine,
        }
    };
    inp.tier_list.iter().filter(|t| lane_ok(inp, t) && !taken(inp.cs, mine, t.champ)).map(pick).collect()
}

/// My lane's tier list: the first TIER_ROWS, and my champion after them when it is further down.
fn tier_board(inp: &ChampSelectInput, mine: Option<ChampKey>, opp: Option<&EnemyVs>) -> Option<TierBoard> {
    let lane = inp.cs.my_lane?;
    let mut best = tier_rows(inp, mine, opp.map_or(&[], |e| e.counters));
    if best.is_empty() {
        return None;
    }
    let me = best.iter().position(|p| p.mine).filter(|&i| i >= TIER_ROWS).map(|i| best.remove(i));
    best.truncate(TIER_ROWS);
    best.extend(me);
    Some(TierBoard { lane, vs: opp.map(|e| e.champ), best })
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

/// My points against the enemy whose table has row `m`, beyond what both champions' win rates
/// predict (lolalytics' delta 2; without it my win rate against them over my `usual`), shrunk by games.
fn edge(m: &Matchup, usual: f32) -> f32 {
    let d = m.delta.map_or(100.0 - pct(m.vs_wr) - usual, |d| -d);
    d * m.games as f32 / (m.games as f32 + MATCHUP_SHRINK_K)
}

/// For the game: the tier list win rate, my edge against every revealed enemy in their lane
/// (each counts the same; its size is in its own number), synergy and composition.
fn picks(inp: &ChampSelectInput, mine: Option<ChampKey>) -> Vec<PickSuggestion> {
    let (cs, sd) = (inp.cs, inp.sd);
    let lane_name = cs.my_lane.map_or("your role", Lane::as_str);
    let ctx = draft::pick_ctx(inp);
    let v = inp
        .tier_list
        .iter()
        .filter(|t| lane_ok(inp, t) && !taken(cs, mine, t.champ))
        .map(|t| {
            let base = shrink(pct(t.win_rate), t.games, TIER_SHRINK_K);
            let (mut sum, mut best) = (0.0, None::<(f32, f32, &EnemyVs)>);
            for e in inp.vs {
                let Some(m) = row(e.counters, t.champ) else { continue };
                let d = edge(m, pct(t.win_rate));
                sum += d;
                if best.is_none_or(|b| d > b.0) {
                    best = Some((d, 100.0 - pct(m.vs_wr), e));
                }
            }
            // Reasons: the terms that raised the score most (matchup, synergy, comp gap).
            let terms = draft::pick_terms(inp, &ctx, t.champ);
            let score = base + MATCHUP_BLEND * sum + terms.iter().map(|x| x.0).sum::<f32>();
            let mut why: Vec<(f32, String)> = terms.into_iter().filter(|x| x.0 > 0.0).collect();
            if let Some((d, wr, e)) = best.filter(|b| b.0 > 0.0) {
                why.push((MATCHUP_BLEND * d, format!("{wr:.1}% vs {}{}", sd.champ_name(e.champ), if Some(e.lane) == cs.my_lane { " (lane)" } else { "" })));
            }
            why.sort_by(|a, b| b.0.total_cmp(&a.0));
            let reason = match why.as_slice() {
                [] => format!("{:.1}% win rate in {lane_name}", pct(t.win_rate)),
                [a, b, ..] if b.0 >= REASON_SECOND_MIN => format!("{} · {}", a.1, b.1),
                [a, ..] => a.1.clone(),
            };
            PickSuggestion { champ: t.champ, score, reason }
        })
        .collect();
    top(v, PICKS)
}

/// For the lane: my lane's tier list by win rate against the enemies I face there (shrunk by
/// games; the mean over both in bot lane), nothing else.
fn lane_picks(inp: &ChampSelectInput, mine: Option<ChampKey>, faced: &[&EnemyVs]) -> Vec<PickSuggestion> {
    let v = inp
        .tier_list
        .iter()
        .filter(|t| lane_ok(inp, t) && !taken(inp.cs, mine, t.champ))
        .filter_map(|t| {
            let rows: Vec<(&EnemyVs, &Matchup)> = faced.iter().filter_map(|&e| Some((e, row(e.counters, t.champ)?))).collect();
            let score = rows.iter().map(|(_, m)| 100.0 - shrink(pct(m.vs_wr), m.games, MATCHUP_SHRINK_K)).sum::<f32>() / rows.len() as f32;
            let reason = |(e, m): &(&EnemyVs, &Matchup)| format!("{:.1}% vs {} · {} games", 100.0 - pct(m.vs_wr), inp.sd.champ_name(e.champ), fmt_games(m.games as u64));
            (!rows.is_empty()).then(|| PickSuggestion { champ: t.champ, score, reason: rows.iter().map(reason).collect::<Vec<_>>().join(", ") })
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

/// Rift / ARAM rune pages for my champion (Mayhem has no runes); `vs`: against my lane opponent.
pub fn rune_pages(inp: &ChampSelectInput, vs: Option<&runes::VsRunes>) -> Option<RuneAdvice> {
    let me = inp.cs.my_pick.or(inp.cs.my_hover).filter(|_| inp.cs.mode != Mode::Mayhem)?;
    let mine = inp.mine.filter(|m| m.champ == me)?;
    let foes: Vec<threat::Foe> = inp.cs.enemies.iter().map(|&e| threat::generic_foe(champ(inp.sd, e))).collect();
    runes::advice(inp.sd, mine, &foes, vs)
}

/// The item engine's build from `stats` (my champion's own, or `vs_build`'s against `vs`), with
/// the item needs counted from `enemies`.
fn build_plan(inp: &ChampSelectInput, stats: &ChampStats, enemies: &[ChampKey], vs: Vec<ChampKey>, source: String) -> Option<BuildPlan> {
    let sd = inp.sd;
    let foes: Vec<threat::Foe> = enemies.iter().map(|&e| threat::generic_foe(champ(sd, e))).collect();
    let needs = needs::needs(&foes);
    let mut ctx = items::Ctx::new(sd, Some(stats), champ(sd, stats.champ), inp.cs.mode);
    let is_boots = |id: ItemId| sd.items.get(&id).is_some_and(|i| i.boots);
    let start = opening(sd, stats);
    ctx.support = inp.cs.mode == Mode::Rift
        && (inp.cs.my_lane == Some(Lane::Support) || start.iter().any(|i| SUPPORT_QUEST.contains(i)));
    let b = items::build(&ctx, &[], &needs);
    let pick = |s: &items::Scored| items::to_pick(&ctx, s, &needs, &foes, None);
    Some(BuildPlan {
        source,
        vs,
        upgrade: items::support_upgrade(&ctx, &[]),
        start,
        core: b.path.iter().filter(|s| !is_boots(s.id)).take(CORE_ITEMS).map(pick).collect(),
        boots: b.path.iter().find(|s| is_boots(s.id)).map(pick),
    })
}
