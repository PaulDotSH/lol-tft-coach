//! lolalytics ARAM tier list for the bench (verified 2026-10-22): `ep=list`
//! with `queue=aram` and no `lane` answers `queue: 450` and the usual
//! `cid` → `{rank, tier, wr, pr, games}` map. ARAM win rates sit ~2 points
//! above 50 (`avgWr`), so compare them with each other, not with 50. The
//! current patch is thin for one rank bracket (emerald_plus: ~300 games per
//! champion), so the 30-day window is used until the median reaches MIN_MEDIAN.

use crate::lolalytics::parse_tier_list;
use model::{Lane, TierEntry};
use std::time::Duration;

const MEGA: &str = "https://a1.lolalytics.com/mega/";
const TTL: Duration = Duration::from_secs(12 * 3600);
const FALLBACK_PATCH: &str = "30";
/// Median games per champion below which the 30-day window is used.
pub const MIN_MEDIAN: u32 = 2000;

fn url(tier: &str, region: &str, patch: &str) -> String {
    format!("{MEGA}?ep=list&v=1&patch={patch}&tier={tier}&queue=aram&region={region}")
}

/// ARAM rows; `TierEntry::lane` has no meaning here and is set to Middle.
pub fn parse_aram_list(json: &str) -> anyhow::Result<Vec<TierEntry>> {
    parse_tier_list(json, Lane::Middle)
}

fn median_games(v: &[TierEntry]) -> u32 {
    let mut g: Vec<u32> = v.iter().map(|e| e.games).collect();
    g.sort_unstable();
    g.get(g.len() / 2).copied().unwrap_or(0)
}

/// ARAM (and Mayhem) tier list, all champions.
pub async fn aram_tier_list(
    http: &crate::Http,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<Vec<TierEntry>> {
    let v = http.get_parsed(&url(tier, region, patch), TTL, parse_aram_list).await?;
    if median_games(&v) < MIN_MEDIAN && patch != FALLBACK_PATCH {
        match http.get_parsed(&url(tier, region, FALLBACK_PATCH), TTL, parse_aram_list).await {
            Ok(alt) if median_games(&alt) > median_games(&v) => return Ok(alt),
            Ok(_) => {}
            Err(e) => tracing::warn!("lolalytics ARAM list 30-day fallback: {e:#}"),
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aram_list() {
        let json = std::fs::read_to_string(format!(
            "{}/../../fixtures/data/lolalytics-list-aram.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let v = parse_aram_list(&json).unwrap();
        assert_eq!(v.len(), 173);
        let ahri = v.iter().find(|e| e.champ == 103).unwrap();
        assert_eq!((ahri.win_rate, ahri.games, ahri.tier.as_deref()), (54.05, 18664, Some("A")));
        assert!(median_games(&v) > MIN_MEDIAN);
        assert!(parse_aram_list(r#"{"status":404}"#).is_err());
    }

    #[test]
    fn aram_url() {
        assert_eq!(
            url("emerald_plus", "all", "16.21"),
            "https://a1.lolalytics.com/mega/?ep=list&v=1&patch=16.21&tier=emerald_plus&queue=aram&region=all"
        );
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_aram_list() {
        let http = crate::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap();
        let v = aram_tier_list(&http, "emerald_plus", "all", "16.21").await.unwrap();
        assert!(v.len() > 150 && median_games(&v) > 500);
    }
}
