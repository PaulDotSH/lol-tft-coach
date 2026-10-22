//! Draft coach (Rift): lane matchups, duo synergy and team comp for both sides
//! as a transparent breakdown, plus the synergy / comp-gap terms that
//! `champ_select::picks` blends into the counter-pick score. Everything is in
//! win-rate points vs an even draft; nothing here names a single best pick.

use crate::{ChampSelectInput, champ_select::pct, common::*, consts::*};
use model::*;
use std::collections::HashMap;

/// lolalytics data for the draft, beyond what `ChampSelectInput` already has.
#[derive(Debug, Default, Clone, Copy)]
pub struct DraftData<'a> {
    /// `counter` stats per revealed champion in its assigned or likely lane.
    pub lane_stats: &'a [ChampStats],
    /// `build-team` per revealed champion in its lane (one side of a pair is enough).
    pub synergy: &'a [TeamSynergy],
}

/// Revealed champions with their lane (None = no lane data yet).
pub type Lineup = Vec<(ChampKey, Option<Lane>)>;

/// Both teams' lineups. Allies (you first) keep their assigned lanes; the rest
/// get the lane-share assignment that fits best. `known` supplies lane shares
/// (lolalytics `lanes`, percent).
pub fn lineups(cs: &ChampSelect, known: &[&ChampStats]) -> (Lineup, Lineup) {
    let shares = |c: ChampKey| known.iter().find(|s| s.champ == c && !s.lanes.is_empty()).map(|s| &s.lanes);
    let mut fixed: Vec<(ChampKey, Lane)> =
        cs.ally_lanes.iter().copied().filter(|(c, _)| cs.allies.contains(c)).collect();
    let mut free: Vec<ChampKey> = cs.allies.iter().copied().filter(|c| !fixed.iter().any(|f| f.0 == *c)).collect();
    match (cs.my_pick.or(cs.my_hover), cs.my_lane) {
        (Some(m), Some(l)) => fixed.insert(0, (m, l)),
        (Some(m), None) => free.insert(0, m),
        _ => {}
    }
    (assign(&fixed, &free, shares), assign(&[], &cs.enemies, shares))
}

fn assign<'a>(
    fixed: &[(ChampKey, Lane)],
    free: &[ChampKey],
    shares: impl Fn(ChampKey) -> Option<&'a HashMap<Lane, f32>>,
) -> Lineup {
    let open: Vec<Lane> = Lane::ALL.into_iter().filter(|l| !fixed.iter().any(|f| f.1 == *l)).collect();
    let rows: Vec<Vec<f32>> = free
        .iter()
        .map(|&c| shares(c).map_or(vec![], |m| open.iter().map(|l| m.get(l).copied().unwrap_or(0.0)).collect()))
        .collect();
    // Exhaustive: at most 5 champions × (5 lanes + none).
    fn go(
        i: usize,
        used: u32,
        rows: &[Vec<f32>],
        cur: &mut Vec<Option<usize>>,
        sum: f32,
        best: &mut (f32, Vec<Option<usize>>),
    ) {
        if i == rows.len() {
            if sum > best.0 {
                *best = (sum, cur.clone());
            }
            return;
        }
        for (j, &s) in rows[i].iter().enumerate() {
            if used & (1 << j) == 0 && s >= MIN_LANE_SHARE {
                cur.push(Some(j));
                go(i + 1, used | (1 << j), rows, cur, sum + s, best);
                cur.pop();
            }
        }
        cur.push(None);
        go(i + 1, used, rows, cur, sum, best);
        cur.pop();
    }
    let mut best = (f32::NEG_INFINITY, vec![]);
    go(0, 0, &rows, &mut Vec::with_capacity(free.len()), 0.0, &mut best);
    let placed = free.iter().zip(best.1).map(|(&c, j)| (c, j.map(|j| open[j])));
    fixed.iter().map(|&(c, l)| (c, Some(l))).chain(placed).collect()
}

fn known<'a>(inp: &'a ChampSelectInput) -> Vec<&'a ChampStats> {
    inp.mine.into_iter().chain(inp.enemies).chain(inp.draft.lane_stats).collect()
}

/// `a`'s win rate vs `e` with both in `lane`, percent, and its games.
fn matchup(stats: &[&ChampStats], a: ChampKey, e: ChampKey, lane: Lane) -> Option<(f32, u32)> {
    let find = |x: ChampKey, y: ChampKey| {
        stats
            .iter()
            .filter(|s| s.champ == x && s.lane == Some(lane))
            .find_map(|s| s.counters.iter().find(|m| m.champ == y && m.games >= MATCHUP_MIN_GAMES))
            .map(|m| (pct(m.vs_wr), m.games))
    };
    find(a, e).or_else(|| find(e, a).map(|(wr, g)| (100.0 - wr, g)))
}

/// Synergy delta of a pair (points) and its games, from either side's `build-team`.
fn synergy(syn: &[TeamSynergy], a: (ChampKey, Lane), b: (ChampKey, Lane)) -> Option<(f32, u32)> {
    let find = |x: (ChampKey, Lane), y: (ChampKey, Lane)| {
        syn.iter()
            .filter(|t| (t.champ, t.lane) == x)
            .find_map(|t| t.mates.iter().find(|m| (m.champ, m.lane) == y && m.games >= SYN_MIN_GAMES))
            .map(|m| (m.delta, m.games))
    };
    [find(a, b), find(b, a)].into_iter().flatten().max_by_key(|x| x.1)
}

fn duo_w(a: Lane, b: Lane) -> f32 {
    DUO_W.iter().find(|d| (d.0, d.1) == (a, b) || (d.1, d.0) == (a, b)).map_or(DUO_W_OTHER, |d| d.2)
}

fn shrunk_syn((delta, games): (f32, u32)) -> f32 {
    delta * games as f32 / (games as f32 + SYN_SHRINK_K)
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Comp {
    physical: f32,
    magic: f32,
    true_dmg: f32,
    frontline: u32,
    engage: u32,
    cc: f32,
    n: usize,
}

fn is_engage(t: &Traits) -> bool {
    t.cc >= ENGAGE_CC && t.tank >= ENGAGE_TANK
}

fn comp(sd: &StaticData, champs: impl Iterator<Item = ChampKey>) -> Comp {
    let mut c = Comp::default();
    for t in champs.filter_map(|k| sd.champs.get(&k)).map(|ch| &ch.traits) {
        let (w, tr) = (1.0 - DMG_TANK_DISCOUNT * t.tank, t.true_damage * TRUE_FRAC);
        c.physical += w * (1.0 - t.magic_share) * (1.0 - tr);
        c.magic += w * t.magic_share * (1.0 - tr);
        c.true_dmg += w * tr;
        c.frontline += (t.tank >= FRONTLINE_TANK) as u32;
        c.engage += is_engage(t) as u32;
        c.cc += t.cc;
        c.n += 1;
    }
    let sum = c.physical + c.magic + c.true_dmg;
    if sum > 0.0 {
        (c.physical, c.magic, c.true_dmg) = (c.physical / sum, c.magic / sum, c.true_dmg / sum);
    }
    c
}

/// (points lost, note) per gap; empty below `min` champions.
fn gaps(c: &Comp, min: usize) -> Vec<(f32, String)> {
    let mut g = Vec::new();
    if c.n < min {
        return g;
    }
    if c.frontline == 0 {
        g.push((GAP_FRONTLINE, "No frontline".to_string()));
    }
    if c.physical >= ONE_SIDED {
        g.push((GAP_DAMAGE, format!("Mostly physical ({:.0}%)", c.physical * 100.0)));
    } else if c.magic >= ONE_SIDED {
        g.push((GAP_DAMAGE, format!("Mostly magic ({:.0}%)", c.magic * 100.0)));
    }
    if c.cc < LOW_CC_PER_CHAMP * c.n as f32 {
        g.push((GAP_CC, "Little CC".to_string()));
    }
    if c.engage == 0 && c.n >= 4 {
        g.push((GAP_ENGAGE, "No engage".to_string()));
    }
    g
}

fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Top => "Top",
        Lane::Jungle => "Jungle",
        Lane::Middle => "Mid",
        Lane::Bottom => "Bot",
        Lane::Support => "Support",
    }
}

fn signed(x: f32) -> String {
    format!("{x:+.1}")
}

/// Breakdown for both teams; None until anyone is revealed.
pub fn report(inp: &ChampSelectInput) -> Option<DraftReport> {
    let (sd, stats) = (inp.sd, known(inp));
    let (al, en) = lineups(inp.cs, &stats);
    if al.is_empty() && en.is_empty() {
        return None;
    }
    let lane_of = |l: &Lineup, lane: Lane| l.iter().find(|x| x.1 == Some(lane)).map(|x| x.0);
    let lanes: Vec<DraftLane> = Lane::ALL
        .into_iter()
        .filter_map(|lane| {
            let (ally, enemy) = (lane_of(&al, lane), lane_of(&en, lane));
            if ally.is_none() && enemy.is_none() {
                return None;
            }
            let m = ally.zip(enemy).and_then(|(a, e)| matchup(&stats, a, e, lane));
            Some(DraftLane { lane, ally, enemy, ally_wr: m.map(|m| m.0), games: m.map_or(0, |m| m.1) })
        })
        .collect();
    let side = |me: &Lineup, ally: bool| {
        let c = comp(sd, me.iter().map(|x| x.0));
        let mut factors = Vec::new();
        // Lanes: this side's win rate in each known matchup.
        let mut lane_pts = (0.0, Vec::new());
        let mut matched = Vec::new();
        for l in lanes.iter().filter(|l| l.ally_wr.is_some()) {
            let wr = l.ally_wr.unwrap_or(50.0);
            let wr = if ally { wr } else { 100.0 - wr };
            lane_pts.0 += shrink(wr, l.games, MATCHUP_SHRINK_K) - 50.0;
            lane_pts.1.push(format!("{} {wr:.1}%", lane_name(l.lane)));
            matched.extend(if ally { l.ally } else { l.enemy });
        }
        if !lane_pts.1.is_empty() {
            factors.push(DraftFactor { label: "Lanes".into(), points: lane_pts.0, detail: lane_pts.1.join(", ") });
        }
        // Champions without a known matchup: their own win rate in lane.
        let mut champ_pts: Vec<(f32, String)> = me
            .iter()
            .filter(|x| !matched.contains(&x.0))
            .filter_map(|&(c, l)| {
                let s = stats.iter().find(|s| s.champ == c && l.is_some() && s.lane == l && s.games > 0)?;
                let d = shrink(pct(s.win_rate), s.games.min(u32::MAX as u64) as u32, TIER_SHRINK_K) - 50.0;
                Some((d, format!("{} {:.1}%", sd.champ_name(c), pct(s.win_rate))))
            })
            .collect();
        if !champ_pts.is_empty() {
            champ_pts.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
            let pts = champ_pts.iter().map(|x| x.0).sum();
            let detail = champ_pts.iter().take(3).map(|x| x.1.as_str()).collect::<Vec<_>>().join(", ");
            factors.push(DraftFactor { label: "Champions".into(), points: pts, detail });
        }
        // Duo synergy over every pair with lanes.
        let placed: Vec<(ChampKey, Lane)> = me.iter().filter_map(|&(c, l)| Some((c, l?))).collect();
        let mut duos: Vec<(f32, DraftDuo)> = Vec::new();
        for (i, &a) in placed.iter().enumerate() {
            for &b in &placed[i + 1..] {
                if let Some(s) = synergy(inp.draft.synergy, a, b) {
                    let pts = duo_w(a.1, b.1) * shrunk_syn(s);
                    duos.push((pts, DraftDuo { a: a.0, b: b.0, delta: s.0, games: s.1 }));
                }
            }
        }
        if !duos.is_empty() {
            duos.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
            let pts = duos.iter().map(|d| d.0).sum();
            let d = &duos[0].1;
            let detail = format!("{} + {} {}", sd.champ_name(d.a), sd.champ_name(d.b), signed(d.delta));
            factors.push(DraftFactor { label: "Synergy".into(), points: pts, detail });
        }
        let gaps = gaps(&c, GAP_MIN_CHAMPS);
        if !gaps.is_empty() {
            let detail = gaps.iter().map(|g| g.1.as_str()).collect::<Vec<_>>().join(", ");
            factors.push(DraftFactor {
                label: "Composition".into(),
                points: -gaps.iter().map(|g| g.0).sum::<f32>(),
                detail,
            });
        }
        DraftSide {
            champs: me.iter().map(|x| x.0).collect(),
            physical: c.physical,
            magic: c.magic,
            true_dmg: c.true_dmg,
            frontline: c.frontline,
            engage: c.engage,
            cc: c.cc,
            duos: duos.into_iter().map(|d| d.1).collect(),
            gaps: gaps.into_iter().map(|g| g.1).collect(),
            total: factors.iter().map(|f| f.points).sum(),
            factors,
        }
    };
    Some(DraftReport { allies: side(&al, true), enemies: side(&en, false), lanes })
}

/// Allies other than you (with lanes) and their comp, computed once per advice.
pub(crate) struct PickCtx {
    allies: Vec<(ChampKey, Lane)>,
    comp: Comp,
}

pub(crate) fn pick_ctx(inp: &ChampSelectInput) -> PickCtx {
    let (al, _) = lineups(inp.cs, &known(inp));
    let mine = inp.cs.my_pick.or(inp.cs.my_hover);
    let others: Vec<_> = al.into_iter().filter(|x| Some(x.0) != mine).collect();
    PickCtx {
        allies: others.iter().filter_map(|&(c, l)| Some((c, l?))).collect(),
        comp: comp(inp.sd, others.iter().map(|x| x.0)),
    }
}

/// Score terms (points, reason) for candidate `cand` in my lane: synergy with
/// revealed allies and filling the allies' comp gaps.
pub(crate) fn pick_terms(inp: &ChampSelectInput, ctx: &PickCtx, cand: ChampKey) -> Vec<(f32, String)> {
    let mut v = Vec::new();
    if let Some(lane) = inp.cs.my_lane {
        let mut best: Option<(f32, ChampKey, f32)> = None;
        let mut sum = 0.0;
        for &(a, la) in &ctx.allies {
            let Some(s) = synergy(inp.draft.synergy, (a, la), (cand, lane)) else { continue };
            let pts = SYN_PICK_W * duo_w(la, lane) * shrunk_syn(s);
            sum += pts;
            if best.is_none_or(|b| pts > b.0) {
                best = Some((pts, a, s.0));
            }
        }
        if let Some((_, a, d)) = best {
            v.push((sum, format!("synergy with {} {}%", inp.sd.champ_name(a), signed(d))));
        }
    }
    let Some(t) = inp.sd.champs.get(&cand).map(|c| &c.traits) else { return v };
    let c = &ctx.comp;
    if c.n < PICK_GAP_MIN_ALLIES {
        return v;
    }
    if c.physical >= ONE_SIDED && t.magic_share >= FILL_MAGIC {
        v.push((GAP_DAMAGE, format!("adds magic damage: team is {:.0}% physical", c.physical * 100.0)));
    } else if c.magic >= ONE_SIDED && t.magic_share <= FILL_PHYSICAL {
        v.push((GAP_DAMAGE, format!("adds physical damage: team is {:.0}% magic", c.magic * 100.0)));
    }
    if c.frontline == 0 && t.tank >= FRONTLINE_TANK {
        v.push((GAP_FRONTLINE, "adds frontline: team has none".into()));
    }
    if c.engage == 0 && is_engage(t) {
        v.push((GAP_ENGAGE, "adds engage: team has none".into()));
    } else if c.cc < LOW_CC_PER_CHAMP * c.n as f32 && t.cc >= FILL_CC {
        v.push((GAP_CC, "adds CC: team has little".into()));
    }
    v
}
