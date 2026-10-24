//! LP trend per ranked queue from rank snapshots the app saves after each game.
//! A game shows as a change in wins + losses; its LP change is the ladder
//! difference, so promotions and demotions count right.

use model::*;

const TIERS: [&str; 10] = ["IRON", "BRONZE", "SILVER", "GOLD", "PLATINUM", "EMERALD", "DIAMOND", "MASTER", "GRANDMASTER", "CHALLENGER"];
const DIVISIONS: [&str; 4] = ["IV", "III", "II", "I"];
/// Master and up share one LP scale on top of Diamond I.
const MASTER: usize = 7;
const MAX_POINTS: usize = 20;

/// Place on the whole ladder: 400 per tier, 100 per division, plus LP.
pub fn ladder(tier: &str, division: &str, lp: i32) -> Option<i32> {
    let t = TIERS.iter().position(|&x| x == tier)?;
    if t >= MASTER {
        return Some(MASTER as i32 * 400 + lp);
    }
    let d = DIVISIONS.iter().position(|&x| x == division)?;
    Some(t as i32 * 400 + d as i32 * 100 + lp)
}

/// Snapshots oldest first, any queue order; one trend per queue seen (solo first).
pub fn rank_trends(snaps: &[RankSnapshot]) -> Vec<RankTrend> {
    ["solo", "flex"]
        .into_iter()
        .filter_map(|queue| {
            let q: Vec<&RankSnapshot> = snaps.iter().filter(|s| s.queue == queue).collect();
            let last = *q.last()?;
            let mut games: Vec<RankPoint> = q
                .windows(2)
                .filter(|w| w[1].wins + w[1].losses > w[0].wins + w[0].losses)
                .filter_map(|w| {
                    let (a, b) = (ladder(&w[0].tier, &w[0].division, w[0].lp)?, ladder(&w[1].tier, &w[1].division, w[1].lp)?);
                    Some(RankPoint { time: w[1].time, ladder: b, delta: b - a, win: w[1].wins - w[0].wins.min(w[1].wins) > w[1].losses - w[0].losses.min(w[1].losses) })
                })
                .collect();
            if games.len() > MAX_POINTS {
                games.drain(..games.len() - MAX_POINTS);
            }
            let newest = games.last().map(|p| p.win);
            let run = games.iter().rev().take_while(|p| Some(p.win) == newest).count() as i32;
            Some(RankTrend {
                queue: queue.into(),
                tier: last.tier.clone(),
                division: last.division.clone(),
                lp: last.lp,
                streak: if newest == Some(true) { run } else { -run },
                games,
            })
        })
        .collect()
}
