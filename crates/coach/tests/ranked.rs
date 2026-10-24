//! LP trend from saved rank snapshots.

use coach::{ladder, rank_trends};
use model::*;

fn snap(time: u64, queue: &str, tier: &str, division: &str, lp: i32, wins: u32, losses: u32) -> RankSnapshot {
    RankSnapshot { time, queue: queue.into(), tier: tier.into(), division: division.into(), lp, wins, losses }
}

#[test]
fn ladder_counts_tiers_divisions_and_master_lp() {
    assert_eq!(ladder("IRON", "IV", 0), Some(0));
    assert_eq!(ladder("GOLD", "II", 7), Some(3 * 400 + 2 * 100 + 7));
    assert_eq!(ladder("MASTER", "NA", 50), Some(2850));
    assert_eq!(ladder("GRANDMASTER", "NA", 350), Some(3150));
    assert_eq!(ladder("NONE", "NA", 0), None);
}

#[test]
fn games_promotions_and_streaks_per_queue() {
    let snaps = [
        snap(1, "solo", "GOLD", "III", 80, 10, 1),
        snap(1, "flex", "GOLD", "II", 31, 13, 4),
        // Same game count again (app restart): no point.
        snap(2, "solo", "GOLD", "III", 80, 10, 1),
        // Win across the division line: +27.
        snap(3, "solo", "GOLD", "II", 7, 11, 1),
        snap(4, "solo", "GOLD", "II", 26, 12, 1),
        snap(5, "flex", "GOLD", "II", 12, 13, 5),
    ];
    let t = rank_trends(&snaps);
    assert_eq!(t.len(), 2);
    let solo = &t[0];
    assert_eq!((solo.queue.as_str(), solo.tier.as_str(), solo.division.as_str(), solo.lp), ("solo", "GOLD", "II", 26));
    let deltas: Vec<(i32, bool)> = solo.games.iter().map(|p| (p.delta, p.win)).collect();
    assert_eq!(deltas, [(27, true), (19, true)]);
    assert_eq!(solo.streak, 2);
    let flex = &t[1];
    assert_eq!(flex.games.iter().map(|p| p.delta).collect::<Vec<_>>(), [-19]);
    assert_eq!(flex.streak, -1);

    // One snapshot: the rank, no games yet.
    let only = rank_trends(&snaps[..2]);
    assert!(only.iter().all(|t| t.games.is_empty() && t.streak == 0));
}
