//! Game-plan card: which team is stronger early and which late, from every champion's win rate
//! by game length (lolalytics build pages) against its own average, the scaling champions, what
//! each comp has and lacks, my lane and my record. Facts and comparisons, never orders.

use crate::{common::*, consts::*, draft::comp_side};
use model::*;
use std::collections::HashMap;

/// One champion's curve: the lane its page is for (None: main lane, or no lanes) and its games
/// and wins by game length.
pub type Curve = (Option<Lane>, Vec<LengthWin>);

pub struct PlanInput<'a> {
    /// My team, me first, with the lanes they play (None outside the Rift or when unknown).
    pub allies: &'a [(ChampKey, Option<Lane>)],
    pub enemies: &'a [(ChampKey, Option<Lane>)],
    pub length: &'a HashMap<ChampKey, Curve>,
    pub draft: Option<&'a DraftReport>,
    pub matchup: Option<&'a MatchupPlan>,
    pub memory: &'a [MatchupMemory],
    pub notes: &'a LaneNotes,
    pub sd: &'a StaticData,
    pub mode: Mode,
}

/// A game-length bucket after merging: `anchor` is the start of its biggest part.
#[derive(Clone, Copy)]
struct Bucket {
    anchor: u32,
    top: u32,
    games: u32,
    wins: u32,
}

/// Buckets under `PLAN_MIN_GAMES`, smallest first, merged into their smaller neighbour.
fn merged(buckets: &[LengthWin]) -> Vec<Bucket> {
    let mut v: Vec<Bucket> =
        buckets.iter().filter(|b| b.games > 0).map(|b| Bucket { anchor: b.from_min, top: b.games, games: b.games, wins: b.wins }).collect();
    while v.len() > 1 {
        let Some(i) = (0..v.len()).filter(|&i| v[i].games < PLAN_MIN_GAMES).min_by_key(|&i| v[i].games) else { break };
        let j = match (i.checked_sub(1), (i + 1 < v.len()).then_some(i + 1)) {
            (Some(a), Some(b)) => if v[a].games <= v[b].games { a } else { b },
            (Some(a), None) => a,
            (None, b) => b.unwrap_or(i),
        };
        let b = v.remove(i.max(j));
        let a = &mut v[i.min(j)];
        if b.top > a.top {
            (a.anchor, a.top) = (b.anchor, b.top);
        }
        (a.games, a.wins) = (a.games + b.games, a.wins + b.wins);
    }
    v
}

fn bounds(mode: Mode) -> [u32; 2] {
    if mode == Mode::Rift { PLAN_RIFT_PHASES } else { PLAN_ARAM_PHASES }
}

fn phase_of(min: u32, b: [u32; 2]) -> usize {
    if min < b[0] {
        0
    } else if min < b[1] {
        1
    } else {
        2
    }
}

/// Per phase: the games-weighted mean of the champion's bucket win rates minus its own, each
/// shrunk toward 0 by its games; None where no bucket falls in the phase.
fn deltas(buckets: &[LengthWin], b: [u32; 2]) -> [Option<f32>; 3] {
    let (games, wins) = buckets.iter().fold((0u32, 0u32), |(g, w), x| (g + x.games, w + x.wins));
    if games == 0 {
        return [None; 3];
    }
    let own = 100.0 * wins as f32 / games as f32;
    let mut acc = [(0.0f32, 0u32); 3];
    for x in merged(buckets) {
        let d = shrink(50.0 + 100.0 * x.wins as f32 / x.games as f32 - own, x.games, PLAN_SHRINK_K) - 50.0;
        let a = &mut acc[phase_of(x.anchor, b)];
        (a.0, a.1) = (a.0 + d * x.games as f32, a.1 + x.games);
    }
    acc.map(|(s, n)| (n > 0).then(|| s / n as f32))
}

struct Champ {
    key: ChampKey,
    ally: bool,
    d: [Option<f32>; 3],
    off_lane: bool,
}

fn mean(v: impl Iterator<Item = f32>) -> Option<f32> {
    let (s, n) = v.fold((0.0, 0), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| s / n as f32)
}

fn names(sd: &StaticData, champs: &[ChampKey]) -> String {
    champs.iter().map(|&c| sd.champ_name(c)).collect::<Vec<_>>().join(", ")
}

fn window(p: usize, b: [u32; 2]) -> String {
    match p {
        0 => format!("before {} min", b[0]),
        1 => format!("at {}–{} min", b[0], b[1]),
        _ => format!("after {} min", b[1]),
    }
}

const WORDS: [&str; 3] = ["early", "in the mid game", "late"];
const NAMES: [&str; 3] = ["Early", "Mid", "Late"];

/// None without a single curve on either side (lolalytics down).
pub fn game_plan(i: &PlanInput) -> Option<GamePlan> {
    let (sd, b) = (i.sd, bounds(i.mode));
    let champs: Vec<Champ> = i
        .allies
        .iter()
        .map(|x| (x, true))
        .chain(i.enemies.iter().map(|x| (x, false)))
        .filter_map(|(&(key, lane), ally)| {
            let (data_lane, curve) = i.length.get(&key)?;
            let d = deltas(curve, b);
            d.iter().any(Option::is_some).then_some(Champ { key, ally, d, off_lane: lane.is_some() && data_lane.is_none() })
        })
        .collect();
    let side = |ally: bool| champs.iter().filter(move |c| c.ally == ally);
    if side(true).next().is_none() || side(false).next().is_none() {
        return None;
    }
    let phases: Vec<PlanPhase> = (0..3)
        .map(|p| {
            let m = |ally: bool| mean(side(ally).filter_map(|c| c.d[p])).unwrap_or(0.0);
            let edge = m(true) - m(false);
            PlanPhase {
                name: NAMES[p].into(),
                from_min: if p == 0 { 0 } else { b[p - 1] },
                to_min: b.get(p).copied(),
                edge,
                even: edge.abs() < PLAN_EVEN,
            }
        })
        .collect();
    let best = |ally: bool, p: usize| {
        let mut v: Vec<(ChampKey, f32)> = side(ally).filter_map(|c| Some((c.key, c.d[p]?))).filter(|x| x.1 >= PLAN_SCALER_MIN).collect();
        v.sort_by(|x, y| y.1.total_cmp(&x.1));
        v.into_iter().map(|x| x.0).collect::<Vec<_>>()
    };
    let mut scalers: Vec<Scaler> = Vec::new();
    for ally in [true, false] {
        for p in [2, 0] {
            if let Some(&k) = best(ally, p).first()
                && !scalers.iter().any(|s| s.champ == k)
                && let Some(c) = champs.iter().find(|c| c.key == k)
            {
                scalers.push(Scaler { champ: k, ally, late: c.d[2].unwrap_or(0.0), early: c.d[0].unwrap_or(0.0), off_lane: c.off_lane });
            }
        }
    }

    let mut lines = Vec::new();
    let mut strong: Vec<usize> = (0..3).filter(|&p| !phases[p].even).collect();
    strong.sort_by(|&x, &y| phases[y].edge.abs().total_cmp(&phases[x].edge.abs()));
    let scaling = if strong.is_empty() {
        "No clear edge by game length.".to_string()
    } else {
        strong
            .iter()
            .map(|&p| {
                let mine = phases[p].edge > 0.0;
                let who: Vec<ChampKey> = best(mine, p).into_iter().take(PLAN_NAMES).collect();
                let who = if who.is_empty() { String::new() } else { format!(": {}", names(sd, &who)) };
                format!("{} stronger {} (+{:.1} {}{who}).", if mine { "You are" } else { "They are" }, WORDS[p], phases[p].edge.abs(), window(p, b))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let scaling = if i.mode == Mode::Mayhem { format!("ARAM data, augments not included. {scaling}") } else { scaling };
    lines.push(PlanLine { kind: "scaling".into(), text: scaling });
    if let Some(text) = i.draft.and_then(comp_line) {
        lines.push(PlanLine { kind: "comp".into(), text });
    }
    let spikes: Vec<ChampKey> = i
        .enemies
        .iter()
        .map(|e| e.0)
        .filter(|&c| sd.champs.get(&c).and_then(|ch| i.notes.champs.get(&ch.id)).is_some_and(|n| n.level6))
        .collect();
    if !spikes.is_empty() {
        lines.push(PlanLine { kind: "spikes".into(), text: format!("Level 6 spikes on their side: {}.", names(sd, &spikes)) });
    }
    if let Some(text) = i.matchup.filter(|_| i.mode == Mode::Rift).and_then(|m| lane_line(sd, m)) {
        lines.push(PlanLine { kind: "lane".into(), text });
    }
    if let Some(text) = record_line(sd, i.memory) {
        lines.push(PlanLine { kind: "record".into(), text });
    }

    let n = (i.allies.len() + i.enemies.len()) as u32;
    Some(GamePlan {
        fold: fold(sd, &phases, &champs),
        phases,
        scalers,
        lines,
        source: if i.mode == Mode::Mayhem { "lolalytics ARAM data".into() } else { "lolalytics".into() },
        partial: (champs.len() as u32) < n,
        curves: champs.len() as u32,
        champs: n,
        show_s: PLAN_SHOW_S,
    })
}

/// "Stronger early · they scale: Kayle".
fn fold(sd: &StaticData, phases: &[PlanPhase], champs: &[Champ]) -> String {
    let pick = |mine: bool| {
        let sign = if mine { 1.0 } else { -1.0 };
        (0..3).filter(|&p| !phases[p].even && phases[p].edge * sign > 0.0).max_by(|&x, &y| phases[x].edge.abs().total_cmp(&phases[y].edge.abs()))
    };
    let mut parts = Vec::new();
    if let Some(p) = pick(true) {
        parts.push(format!("Stronger {}", WORDS[p]));
    }
    if let Some(p) = pick(false) {
        let late = champs.iter().filter(|c| !c.ally && c.d[2].is_some_and(|d| d >= PLAN_SCALER_MIN)).max_by(|x, y| x.d[2].unwrap_or(0.0).total_cmp(&y.d[2].unwrap_or(0.0)));
        parts.push(match (p, late) {
            (2, Some(c)) => format!("they scale: {}", sd.champ_name(c.key)),
            _ => format!("they are stronger {}", WORDS[p]),
        });
    }
    if parts.is_empty() {
        return "No clear edge by game length".into();
    }
    let s = parts.join(" · ");
    let mut c = s.chars();
    c.next().map_or(s.clone(), |f| f.to_uppercase().chain(c).collect())
}

fn comp_line(d: &DraftReport) -> Option<String> {
    let (a, e) = (&d.allies, &d.enemies);
    if a.champs.is_empty() || e.champs.is_empty() {
        return None;
    }
    let mut parts = vec![format!("Engage: you {}, them {}", a.engage, e.engage), format!("Frontline: you {}, them {}", a.frontline, e.frontline)];
    for (who, s) in [("Their", e), ("Your", a)] {
        if s.physical >= ONE_SIDED {
            parts.push(format!("{who} damage is {:.0}% physical", s.physical * 100.0));
        } else if s.magic >= ONE_SIDED {
            parts.push(format!("{who} damage is {:.0}% magic", s.magic * 100.0));
        }
    }
    for (who, s) in [("You", a), ("Them", e)] {
        let rest: Vec<String> = s
            .gaps
            .iter()
            .filter(|g| !g.starts_with("Mostly") && *g != "No frontline" && *g != "No engage")
            .map(|g| {
                let mut c = g.chars();
                c.next().map_or(String::new(), |f| f.to_lowercase().chain(c).collect())
            })
            .collect();
        if !rest.is_empty() {
            parts.push(format!("{who}: {}", rest.join(", ")));
        }
    }
    Some(format!("{}.", parts.join(". ")))
}

/// My direct lane opponent: their support when I support, else the first card.
fn lane_line(sd: &StaticData, m: &MatchupPlan) -> Option<String> {
    let c = if m.lane == Lane::Support { m.cards.last() } else { m.cards.first() }?;
    let name = sd.champ_name(c.champ);
    let wins = c.note.as_ref().map(|n| n.wins.trim_end_matches('.')).filter(|w| !w.is_empty());
    let mut s = match c.vs_wr {
        Some(wr) => format!("Lane vs {name}: {wr:.1}% for you ({} games).", fmt_games(c.games as u64)),
        None if wins.is_some() => format!("Lane vs {name}."),
        None => return None,
    };
    if let Some(w) = wins {
        s.push_str(&format!(" {name}: {w}."));
    }
    Some(s)
}

fn record_line(sd: &StaticData, memory: &[MatchupMemory]) -> Option<String> {
    let mut m: Vec<&MatchupMemory> = memory.iter().filter(|m| m.games > 0).collect();
    if m.is_empty() {
        return None;
    }
    m.sort_by_key(|m| std::cmp::Reverse(m.games));
    let parts: Vec<String> =
        m.iter().take(PLAN_RECORD_MAX).map(|m| format!("{}–{} vs {}", m.wins, m.games - m.wins, sd.champ_name(m.champ))).collect();
    Some(format!("Your record: {}.", parts.join(", ")))
}

/// Both teams' comp from a roster alone (in game, where no draft data was fetched).
pub fn roster_draft(sd: &StaticData, allies: &[ChampKey], enemies: &[ChampKey]) -> DraftReport {
    DraftReport { allies: comp_side(sd, allies), enemies: comp_side(sd, enemies), lanes: vec![] }
}
