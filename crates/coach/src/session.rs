//! My latest run of games for the idle page: record, the mistakes that repeat
//! across them, and my goals over just these games. A session is the newest
//! game plus the ones before it with gaps under SESSION_GAP.

use crate::trends::goal_status;
use model::*;

/// A longer break between two games starts a new session.
const SESSION_GAP_MS: u64 = 2 * 3600 * 1000;
/// Recap summaries are saved just after a game ends.
const SUMMARY_LATE_S: u64 = 600;
/// A mistake repeats when it shows in at least this many games (and half of them).
const REPEAT_MIN: u32 = 2;
const EARLY_DEATH_S: u32 = 600;
const BEHIND_AT_15: i32 = -500;
const MANY_DEATHS: u32 = 6;

fn end_ms(g: &PastGame) -> u64 {
    g.created + g.duration_s as u64 * 1000
}

/// `games` newest first, remakes already left out.
pub fn session(games: &[&PastGame], summaries: &[GameSummary], goals: &[Goal]) -> Option<Session> {
    let first = *games.first()?;
    let mut run = vec![first];
    for g in &games[1..] {
        if run.last().unwrap().created.saturating_sub(end_ms(g)) >= SESSION_GAP_MS {
            break;
        }
        run.push(g);
    }
    let (start, end) = (run.last().unwrap().created, end_ms(first));

    // (text, happened, measurable) over the session's Rift games.
    let rift: Vec<&PastGame> = run.iter().copied().filter(|g| g.mode == Mode::Rift).collect();
    let count = |f: &dyn Fn(&PastGame) -> Option<bool>| {
        let v: Vec<bool> = rift.iter().filter_map(|g| f(g)).collect();
        (v.iter().filter(|&&b| b).count() as u32, v.len() as u32)
    };
    let checks: [(&str, (u32, u32)); 4] = [
        ("No Control Wards bought", count(&|g| g.control_wards.map(|n| n == 0))),
        ("Died before 10:00", count(&|g| g.death_times.as_ref().map(|d| d.iter().any(|&t| t < EARLY_DEATH_S)))),
        ("Behind your lane opponent at 15:00", count(&|g| g.gold_diff_15.map(|d| d <= BEHIND_AT_15))),
        ("Died 6 or more times", count(&|g| Some(g.deaths >= MANY_DEATHS))),
    ];
    let mut repeats: Vec<SessionRepeat> = checks
        .into_iter()
        .filter(|&(_, (n, of))| n >= REPEAT_MIN && 2 * n >= of)
        .map(|(text, (games, of))| SessionRepeat { text: text.into(), games, of })
        .collect();
    repeats.sort_by_key(|r| std::cmp::Reverse(r.games));

    let (from, to) = (start / 1000, end / 1000 + SUMMARY_LATE_S);
    let sums: Vec<&GameSummary> = summaries.iter().filter(|s| (from..=to).contains(&s.ended)).collect();
    let goals = goals.iter().map(|g| goal_status(g, &run, &sums, usize::MAX)).filter(|s| s.of > 0).collect();

    let wins = run.iter().filter(|g| g.win).count() as u32;
    Some(Session {
        start,
        end,
        game_ids: run.iter().map(|g| g.game_id).collect(),
        wins,
        losses: run.len() as u32 - wins,
        repeats,
        goals,
    })
}
