//! u.gg ARAM: Mayhem data from static.bigbrain.gg.

use model::{AugmentId, ChampAugments, ChampKey, MayhemData};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

const BASE: &str = "https://static.bigbrain.gg/custom-aram-mayhem";
const TTL: Duration = Duration::from_secs(12 * 3600);

/// `ddragon_version` like "16.21.1" → u.gg patch "16_21".
pub async fn mayhem(http: &crate::Http, ddragon_version: &str) -> anyhow::Result<MayhemData> {
    let mut last = anyhow::anyhow!("bad Data Dragon version {ddragon_version}");
    for p in patches(ddragon_version) {
        let (manifest, tiers) = (
            format!("{BASE}/{p}/aram-mayhem-augment-manifest-{p}.json"),
            format!("{BASE}/{p}/tierlist-champions-{p}.json"),
        );
        match tokio::try_join!(
            http.get_parsed(&manifest, TTL, parse_manifest),
            http.get_parsed(&tiers, TTL, parse_champ_tiers)
        ) {
            Ok((augment_names, champ_tiers)) => return Ok(MayhemData { patch: p, augment_names, champ_tiers }),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub async fn champ_augments(http: &crate::Http, patch: &str, champ: ChampKey) -> anyhow::Result<ChampAugments> {
    let url =
        |p: &str| format!("{BASE}/{p}/tierlist-per-champion-augments-rarity-{p}/tierlist-augments-{champ}-{p}.json");
    match http.get_parsed(&url(patch), TTL, |s| parse_augments(s, champ)).await {
        Err(e) => match previous(patch) {
            Some(prev) => http.get_parsed(&url(&prev), TTL, |s| parse_augments(s, champ)).await.map_err(|_| e),
            None => Err(e),
        },
        ok => ok,
    }
}

/// u.gg patch for a Data Dragon version, then the previous one (current may not be published yet).
pub fn patches(ddragon_version: &str) -> Vec<String> {
    let mut it = ddragon_version.split('.');
    let (Some(major), Some(minor)) = (it.next(), it.next()) else { return Vec::new() };
    let p = format!("{major}_{minor}");
    let prev = previous(&p);
    std::iter::once(p).chain(prev).collect()
}

/// "16_21" → "16_20"; "16_1" → "15_24".
pub fn previous(patch: &str) -> Option<String> {
    let (major, minor) = patch.split_once('_')?;
    let (major, minor): (u32, u32) = (major.parse().ok()?, minor.parse().ok()?);
    Some(if minor > 1 { format!("{major}_{}", minor - 1) } else { format!("{}_24", major.checked_sub(1)?) })
}

pub fn parse_manifest(json: &str) -> anyhow::Result<HashMap<AugmentId, String>> {
    let raw: HashMap<String, String> = serde_json::from_str(json)?;
    let m: HashMap<_, _> = raw.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect();
    anyhow::ensure!(!m.is_empty(), "empty augment manifest");
    Ok(m)
}

pub fn parse_champ_tiers(json: &str) -> anyhow::Result<HashMap<ChampKey, String>> {
    #[derive(Deserialize)]
    struct File {
        tiers: HashMap<String, Vec<ChampKey>>,
    }
    let f: File = serde_json::from_str(json)?;
    Ok(f.tiers.into_iter().flat_map(|(tier, keys)| keys.into_iter().map(move |k| (k, tier.clone()))).collect())
}

pub fn parse_augments(json: &str, champ: ChampKey) -> anyhow::Result<ChampAugments> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Rarities {
        #[serde(default)]
        k_prismatic: Vec<AugmentId>,
        #[serde(default)]
        k_gold: Vec<AugmentId>,
        #[serde(default)]
        k_silver: Vec<AugmentId>,
    }
    #[derive(Deserialize)]
    struct File {
        rarities: Rarities,
    }
    let r = serde_json::from_str::<File>(json)?.rarities;
    Ok(ChampAugments { champ, prismatic: r.k_prismatic, gold: r.k_gold, silver: r.k_silver })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../fixtures/data/ugg-augment-manifest-16_21.json");
    const TIERS: &str = include_str!("../../../fixtures/data/ugg-tierlist-champions-16_21.json");
    const AHRI: &str = include_str!("../../../fixtures/data/ugg-augments-103-16_21.json");

    #[test]
    fn patch_math() {
        assert_eq!(patches("16.21.1"), ["16_21", "16_20"]);
        assert_eq!(previous("16_1").as_deref(), Some("15_24"));
        assert!(patches("garbage").is_empty());
    }

    #[test]
    fn parse_all() {
        let names = parse_manifest(MANIFEST).unwrap();
        assert_eq!(names[&1], "Accelerating Sorcery");
        assert!(names.len() > 500);

        let tiers = parse_champ_tiers(TIERS).unwrap();
        assert_eq!(tiers[&875], "S+");
        assert_eq!(tiers[&103], "S");

        let a = parse_augments(AHRI, 103).unwrap();
        assert_eq!(a.champ, 103);
        assert_eq!(a.prismatic[0], 1030);
        assert!(!a.gold.is_empty() && !a.silver.is_empty());
        assert!(a.prismatic.iter().chain(&a.gold).chain(&a.silver).all(|id| names.contains_key(id)));
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_mayhem() {
        let http = crate::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap();
        let m = mayhem(&http, "16.21.1").await.unwrap();
        assert!(m.augment_names.len() > 100 && m.champ_tiers.len() > 100);
        let a = champ_augments(&http, &m.patch, 103).await.unwrap();
        assert!(!a.prismatic.is_empty());
    }
}
