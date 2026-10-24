//! Personal trends across games (V11): per mode, from the match-history cache
//! (CS at 10, early deaths, vision) and saved recap summaries (gold held at
//! death, Buy next followed). Plus goal status and the Mayhem augment slice
//! (games and damage only: no augment win rates, even personal ones).

use crate::selfcoach::goal_applies;
use model::*;

/// Fewest games for a trend line; games averaged as "recent"; points kept.
const MIN_POINTS: usize = 3;
const RECENT: usize = 5;
const MAX_POINTS: usize = 20;
/// Recent vs before must differ by this share to count as better / worse.
const FLAT: f32 = 0.05;
/// Games a goal is checked over.
const GOAL_GAMES: usize = 10;
const MAX_AUGMENTS: usize = 12;

fn minutes(duration_s: u32) -> f32 {
    (duration_s as f32 / 60.0).max(1.0)
}

fn mean(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

fn lower_better(kind: GoalKind) -> bool {
    matches!(kind, GoalKind::EarlyDeaths | GoalKind::Unspent)
}

/// One game's value of a metric, in the goal's unit (`Unspent`: mean gold held at death).
fn game_value(kind: GoalKind, g: &PastGame) -> Option<f32> {
    match kind {
        GoalKind::Cs10 => g.cs_10.map(|c| c as f32 / 10.0),
        GoalKind::EarlyDeaths => g.death_times.as_ref().map(|d| d.iter().filter(|&&t| t < 840).count() as f32),
        GoalKind::Vision => Some(g.vision as f32 / minutes(g.duration_s)),
        GoalKind::Unspent | GoalKind::BuyNext => None,
    }
}

fn summary_value(kind: GoalKind, s: &GameSummary) -> Option<f32> {
    match kind {
        GoalKind::Unspent => (!s.unspent_at_death.is_empty())
            .then(|| s.unspent_at_death.iter().sum::<u32>() as f32 / s.unspent_at_death.len() as f32),
        GoalKind::BuyNext => (s.buys > 0).then(|| 100.0 * s.followed as f32 / s.buys as f32),
        GoalKind::Cs10 => s.cs_10.map(|c| c as f32 / 10.0),
        GoalKind::EarlyDeaths => s.deaths_before_14.map(|n| n as f32),
        GoalKind::Vision => Some(s.vision as f32 / minutes(s.duration_s)),
    }
}

/// History covers the Rift metrics; summaries (only games the app watched) the rest.
fn from_history(kind: GoalKind) -> bool {
    matches!(kind, GoalKind::Cs10 | GoalKind::EarlyDeaths | GoalKind::Vision)
}

const KINDS: [GoalKind; 5] = [GoalKind::Cs10, GoalKind::EarlyDeaths, GoalKind::Vision, GoalKind::Unspent, GoalKind::BuyNext];

/// `games` and `summaries` newest first.
pub fn trends(games: &[PastGame], summaries: &[GameSummary], goals: &[Goal]) -> Trends {
    let games: Vec<&PastGame> = games.iter().filter(|g| !g.remake).collect();
    // Values newest first for a mode and metric.
    let values = |mode: Mode, kind: GoalKind| -> Vec<f32> {
        let h: Vec<f32> = games.iter().filter(|g| g.mode == mode).filter_map(|g| game_value(kind, g)).collect();
        if !h.is_empty() {
            h
        } else {
            summaries.iter().filter(|s| s.mode == mode).filter_map(|s| summary_value(kind, s)).collect()
        }
    };

    let mut trends = Vec::new();
    let mut follow = Vec::new();
    for mode in [Mode::Rift, Mode::Aram, Mode::Mayhem] {
        for kind in KINDS.into_iter().filter(|&k| goal_applies(k, mode)) {
            let v = values(mode, kind);
            if v.len() < MIN_POINTS {
                continue;
            }
            let recent = mean(&v[..RECENT.min(v.len())]);
            let rest = &v[RECENT.min(v.len())..v.len().min(MAX_POINTS)];
            let before = (rest.len() >= MIN_POINTS).then(|| mean(rest));
            let better = before.and_then(|b| {
                let d = recent - b;
                (d.abs() > FLAT * b.abs().max(1e-3)).then_some((d < 0.0) == lower_better(kind))
            });
            let series = v.iter().take(MAX_POINTS).rev().copied().collect();
            trends.push(Trend { mode, kind, series, recent, before, better });
        }

        let split = |win: bool| {
            let v: Vec<f32> = summaries
                .iter()
                .filter(|s| s.mode == mode && s.win == win)
                .filter_map(|s| summary_value(GoalKind::BuyNext, s))
                .collect();
            (!v.is_empty()).then(|| mean(&v))
        };
        let n = summaries.iter().filter(|s| s.mode == mode && s.buys > 0).count();
        if n >= MIN_POINTS {
            follow.push(FollowSplit { mode, wins: split(true), losses: split(false), games: n as u32 });
        }
    }

    let mut augments: Vec<AugmentUse> = Vec::new();
    for g in games.iter().filter(|g| g.mode == Mode::Mayhem) {
        let dpm = g.damage_to_champs as f32 / minutes(g.duration_s);
        for &id in &g.augments {
            match augments.iter_mut().find(|a| a.id == id) {
                Some(a) => {
                    a.damage_per_min = (a.damage_per_min * a.games as f32 + dpm) / (a.games + 1) as f32;
                    a.games += 1;
                }
                None => augments.push(AugmentUse { id, games: 1, damage_per_min: dpm }),
            }
        }
    }
    augments.sort_by(|a, b| b.games.cmp(&a.games).then(b.damage_per_min.total_cmp(&a.damage_per_min)));
    augments.truncate(MAX_AUGMENTS);

    let session = crate::session::session(&games, summaries, goals);
    let all: Vec<&GameSummary> = summaries.iter().collect();
    let goals = goals.iter().map(|g| goal_status(g, &games, &all, GOAL_GAMES)).collect();

    Trends { trends, follow, augments, goals, session }
}

/// How many of the newest `limit` measurable games met `goal`.
pub(crate) fn goal_status(goal: &Goal, games: &[&PastGame], summaries: &[&GameSummary], limit: usize) -> GoalStatus {
    let met = |v: f32| if lower_better(goal.kind) { v <= goal.target } else { v >= goal.target };
    let checks: Vec<bool> = if from_history(goal.kind) {
        games.iter().filter(|g| g.mode == Mode::Rift).filter_map(|g| game_value(goal.kind, g)).take(limit).map(met).collect()
    } else if goal.kind == GoalKind::Unspent {
        // Met when no death happened at or above the target.
        summaries
            .iter()
            .filter(|s| !s.unspent_at_death.is_empty())
            .take(limit)
            .map(|s| s.unspent_at_death.iter().all(|&u| (u as f32) < goal.target))
            .collect()
    } else {
        summaries.iter().filter_map(|s| summary_value(goal.kind, s)).take(limit).map(met).collect()
    };
    GoalStatus { goal: goal.clone(), met: checks.iter().filter(|&&m| m).count() as u32, of: checks.len() as u32 }
}
