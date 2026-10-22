//! lolalytics `mega` endpoints. See the spec for the query format.

use model::{ChampStats, Champion, Lane, Mode, TierEntry};

/// Item sets, early sets, counters and benchmarks for one champion.
/// `lane` is ignored for ARAM/Mayhem. `patch` is the current patch ("16.21");
/// falls back to 30 days when the sample is small (`ChampStats::patch` says which).
pub async fn champ_stats(
    http: &crate::Http,
    champ: &Champion,
    lane: Option<Lane>,
    mode: Mode,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<ChampStats> {
    let _ = (http, champ, lane, mode, tier, region, patch);
    todo!()
}

/// Tier list for one lane (meta bans, pick candidates).
pub async fn tier_list(http: &crate::Http, lane: Lane, tier: &str, region: &str, patch: &str) -> anyhow::Result<Vec<TierEntry>> {
    let _ = (http, lane, tier, region, patch);
    todo!()
}
