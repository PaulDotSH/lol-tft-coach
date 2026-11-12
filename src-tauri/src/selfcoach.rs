//! Self-coaching plumbing: feeds the per-game log every tick, finishes the
//! recap with the death review, summary and goal results, and keeps personal
//! trends from saved recaps plus the match-history cache.

use crate::account;
use model::*;
use std::sync::Arc;

/// Recap summaries kept for trends, newest first.
const KEEP: usize = 100;
/// A recap is saved this soon (s) after its game's end (creation + duration, loading aside).
const RECAP_AFTER: u64 = 900;

/// Live goal progress after logging this tick.
pub fn on_tick(log: &mut coach::GameLog, snap: &GameSnapshot, advice: &GameAdvice, sd: &StaticData, goals: &[Goal], sights: Option<&coach::SightLog>) -> Vec<GoalProgress> {
    log.observe(snap, advice, sd, sights);
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
    r.win_curve = log.win_curve.clone();
    r
}

#[derive(Default)]
pub struct SelfCoach {
    summaries: Option<Vec<GameSummary>>,
    /// Bumped when `summaries` changes.
    added: u64,
    trends: Option<Trends>,
    /// What `trends` was computed from: unchanged, idle ticks reuse it.
    from: Option<TrendsFrom>,
}

/// History games, `SelfCoach::added` and goals.
type TrendsFrom = (Option<Arc<Vec<PastGame>>>, u64, Vec<Goal>);

impl SelfCoach {
    /// Every tick: takes in a new recap's summary; recomputes trends while idle.
    pub fn tick(&mut self, phase: Phase, games: Option<Arc<Vec<PastGame>>>, recap: Option<&Recap>, goals: &[Goal]) -> Option<Trends> {
        let sums = self.summaries.get_or_insert_with(load_summaries);
        if let Some(s) = recap.and_then(|r| r.summary.as_ref())
            && !sums.iter().any(|x| x.ended == s.ended)
        {
            sums.insert(0, s.clone());
            sums.truncate(KEEP);
            self.added += 1;
        }
        let fresh = self.from.as_ref().is_some_and(|(g, added, gl)| {
            let same_games = match (g, &games) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (a, b) => a.is_none() && b.is_none(),
            };
            same_games && *added == self.added && gl.as_slice() == goals
        });
        let list = games.as_deref().map_or(&[][..], |g| g.as_slice());
        if matches!(phase, Phase::Idle | Phase::PostGame) && (!list.is_empty() || !sums.is_empty()) && !fresh {
            self.trends = Some(coach::trends(list, sums, goals));
            self.from = Some((games, self.added, goals.to_vec()));
        }
        self.trends.clone()
    }
}

/// The app's own recap of a history game: same champion and length, saved just after it ended.
pub fn recap_for(g: &PastGame) -> Option<Recap> {
    let end = g.created / 1000 + g.duration_s as u64;
    std::fs::read_dir(account::dir("recaps"))
        .ok()?
        .filter_map(|e| {
            let p = e.ok()?.path();
            let saved: u64 = p.file_stem()?.to_str()?.parse().ok()?;
            (saved >= end && saved - end <= RECAP_AFTER).then_some(p)
        })
        .filter_map(|p| serde_json::from_str::<Recap>(&std::fs::read_to_string(p).ok()?).ok())
        .find(|r| r.stats.champ == g.champ && r.stats.duration_s.abs_diff(g.duration_s) <= 2)
}

fn load_summaries() -> Vec<GameSummary> {
    let Ok(dir) = std::fs::read_dir(account::dir("recaps")) else { return vec![] };
    let mut v: Vec<GameSummary> = dir
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .filter_map(|s| serde_json::from_str::<Recap>(&s).ok()?.summary)
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.ended));
    v.truncate(KEEP);
    v
}
