//! Self-coaching plumbing: feeds the per-game log every tick, finishes the
//! recap with the death review, summary and goal results, and keeps personal
//! trends from saved recaps plus the match-history cache.

use crate::settings;
use model::*;
use std::sync::Arc;

/// Recap summaries kept for trends, newest first.
const KEEP: usize = 100;

/// Live goal progress after logging this tick.
pub fn on_tick(log: &mut coach::GameLog, snap: &GameSnapshot, advice: &GameAdvice, sd: &StaticData, goals: &[Goal]) -> Vec<GoalProgress> {
    log.observe(snap, advice, sd);
    let Some(me) = snap.players.get(snap.me) else { return vec![] };
    let now = coach::Now { mode: snap.mode, time: snap.time, cs: me.cs, deaths: me.deaths, vision: me.ward_score, done: false };
    coach::goal_progress(goals, log, &now)
}

/// Adds what the log saw (nothing when the app missed the game). `ended`: unix seconds.
pub fn finish(mut r: Recap, log: &coach::GameLog, sd: &StaticData, goals: &[Goal], ended: u64) -> Recap {
    if !log.seen() {
        return r;
    }
    let e = &r.stats;
    let now = coach::Now { mode: e.mode, time: e.duration_s as f32, cs: e.cs, deaths: e.deaths, vision: e.vision as f32, done: true };
    r.goals = coach::goal_progress(goals, log, &now);
    r.summary = Some(coach::summary(log, e, ended));
    r.review = Some(coach::death_review(log, sd));
    r
}

#[derive(Default)]
pub struct SelfCoach {
    summaries: Option<Vec<GameSummary>>,
    trends: Option<Trends>,
}

impl SelfCoach {
    /// Every tick: takes in a new recap's summary; recomputes trends while idle.
    pub fn tick(&mut self, phase: Phase, games: Option<Arc<Vec<PastGame>>>, recap: Option<&Recap>, goals: &[Goal]) -> Option<Trends> {
        let sums = self.summaries.get_or_insert_with(load_summaries);
        if let Some(s) = recap.and_then(|r| r.summary.as_ref())
            && !sums.iter().any(|x| x.ended == s.ended)
        {
            sums.insert(0, s.clone());
            sums.truncate(KEEP);
        }
        let games = games.as_deref().map_or(&[][..], |g| g.as_slice());
        if matches!(phase, Phase::Idle | Phase::PostGame) && (!games.is_empty() || !sums.is_empty()) {
            self.trends = Some(coach::trends(games, sums, goals));
        }
        self.trends.clone()
    }
}

fn load_summaries() -> Vec<GameSummary> {
    let Ok(dir) = std::fs::read_dir(settings::local_dir("recaps")) else { return vec![] };
    let mut v: Vec<GameSummary> = dir
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .filter_map(|s| serde_json::from_str::<Recap>(&s).ok()?.summary)
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.ended));
    v.truncate(KEEP);
    v
}
