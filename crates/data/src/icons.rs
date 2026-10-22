//! Icons for reading the Tab scoreboard: Mayhem augment small icons
//! (CommunityDragon, mapped by the game's own kiwi mode data) and Data Dragon
//! item icons. Binary bodies are disk-cached next to the JSON cache.

use crate::Http;
use model::{AugmentId, ItemId, Rarity};
use std::collections::HashMap;
use std::time::Duration;

const CDRAGON: &str = "https://raw.communitydragon.org";
const TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// An enabled Mayhem augment: scoreboard (small) icon URL and rarity.
#[derive(Debug, Clone, PartialEq)]
pub struct KiwiAugment {
    pub icon: String,
    pub rarity: Rarity,
}

/// Mayhem augments by id. Tries the Data Dragon version's patch directory
/// first, then `latest`.
pub async fn mayhem_augments(http: &Http, ddragon_version: &str) -> anyhow::Result<HashMap<AugmentId, KiwiAugment>> {
    let mut it = ddragon_version.split('.');
    let patch = match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}.{b}"),
        _ => "latest".into(),
    };
    let mut last = anyhow::anyhow!("no kiwi data");
    for v in [patch.as_str(), "latest"] {
        let url = format!("{CDRAGON}/{v}/game/maps/modespecificdata/kiwi.bin.json");
        match http.get_parsed(&url, TTL, |s| parse_kiwi(s, v)).await {
            Ok(m) => return Ok(m),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Enabled `AugmentData` entries of the kiwi mode bin (CommunityDragon JSON).
/// The file is ~14 MB, so it is scanned rather than deserialised.
pub fn parse_kiwi(json: &str, version: &str) -> anyhow::Result<HashMap<AugmentId, KiwiAugment>> {
    let m: HashMap<_, _> = json
        .split("\"AugmentNameId\"")
        .skip(1)
        .filter_map(|obj| {
            let obj = &obj[..obj.find("\"AugmentData\"")?];
            if field(obj, "Enabled") == Some("false") {
                return None;
            }
            let id: AugmentId = field(obj, "AugmentPlatformId")?.parse().ok()?;
            let path = field(obj, "AugmentSmallIconPath")?.to_lowercase();
            let png = path.strip_suffix(".tex").map_or(path.clone(), |p| format!("{p}.png"));
            let rarity = match field(obj, "rarity") {
                Some("2") => Rarity::Prismatic,
                Some("1") => Rarity::Gold,
                _ => Rarity::Silver,
            };
            Some((id, KiwiAugment { icon: format!("{CDRAGON}/{version}/game/{png}"), rarity }))
        })
        .collect();
    anyhow::ensure!(m.len() > 50, "kiwi data has {} augments", m.len());
    Ok(m)
}

/// Raw value of `"key": value` (string unquoted) inside one flat JSON object.
fn field<'a>(obj: &'a str, key: &str) -> Option<&'a str> {
    let rest = obj[obj.find(&format!("\"{key}\""))? + key.len() + 2..].trim_start().strip_prefix(':')?.trim_start();
    match rest.strip_prefix('"') {
        Some(s) => s.split('"').next(),
        None => rest.split(|c: char| !c.is_ascii_alphanumeric()).next().filter(|s| !s.is_empty()),
    }
}

pub fn item_icon_url(ddragon_version: &str, id: ItemId) -> String {
    format!("https://ddragon.leagueoflegends.com/cdn/{ddragon_version}/img/item/{id}.png")
}

/// GET `url` as bytes, served from the disk cache if younger than `ttl`; a
/// stale entry is used when the network fails.
pub async fn get_bytes(http: &Http, url: &str, ttl: Duration) -> anyhow::Result<Vec<u8>> {
    let path = http.cache_path(url).with_extension("bin");
    let cached = tokio::fs::metadata(&path).await.ok().and_then(|m| m.modified().ok());
    let fresh = cached.and_then(|t| t.elapsed().ok()).is_some_and(|age| age < ttl);
    if fresh && let Ok(b) = tokio::fs::read(&path).await {
        return Ok(b);
    }
    let fetched =
        async { anyhow::Ok(http.client.get(url).send().await?.error_for_status()?.bytes().await?.to_vec()) }.await;
    match fetched {
        Ok(b) => {
            let tmp = path.with_extension("tmp");
            if tokio::fs::write(&tmp, &b).await.is_ok() {
                let _ = tokio::fs::rename(&tmp, &path).await;
            }
            Ok(b)
        }
        Err(e) => tokio::fs::read(&path).await.map_err(|_| e.context(url.to_owned())),
    }
}

/// Every distinct icon once, eight requests at a time; failures are skipped.
pub async fn fetch_icons(http: &Http, urls: &HashMap<AugmentId, &str>) -> Vec<(AugmentId, Vec<u8>)> {
    let mut unique: Vec<&str> = urls.values().copied().collect();
    unique.sort();
    unique.dedup();
    let mut bodies: HashMap<String, Vec<u8>> = HashMap::new();
    for chunk in unique.chunks(8) {
        let mut set = tokio::task::JoinSet::new();
        for &url in chunk {
            let (http, url) = (http.clone(), url.to_string());
            set.spawn(async move { (get_bytes(&http, &url, TTL).await, url) });
        }
        while let Some(Ok((res, url))) = set.join_next().await {
            match res {
                Ok(b) => {
                    bodies.insert(url, b);
                }
                Err(e) => tracing::warn!("augment icon: {e:#}"),
            }
        }
    }
    urls.iter().filter_map(|(&id, u)| Some((id, bodies.get(*u)?.clone()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIWI: &str = include_str!("../../../fixtures/data/cdragon-kiwi-augments-16_21.json");

    #[test]
    fn parses_augment_entries_only() {
        let m = parse_kiwi(KIWI, "16.21").unwrap();
        let base = "https://raw.communitydragon.org/16.21/game/assets/ux";
        assert_eq!(
            m[&1305],
            KiwiAugment { icon: format!("{base}/cherry/augments/icons/legday_small.png"), rarity: Rarity::Silver }
        );
        assert_eq!(
            m[&1001],
            KiwiAugment {
                icon: format!("{base}/kiwi/augments/icons/iamthejuggernaut_small.png"),
                rarity: Rarity::Prismatic
            }
        );
        assert_eq!(m[&1420].rarity, Rarity::Gold);
        // Disabled augments, stat anvils (no platform id) and other entries are left out.
        let disabled = KIWI.matches("\"Enabled\": false").count();
        assert!(disabled > 0 && !m.contains_key(&1066));
        assert!(m.values().all(|a| !a.icon.contains("statanvil")));
        assert_eq!(m.len(), KIWI.matches("\"AugmentPlatformId\"").count() - disabled);
    }

    #[test]
    fn field_values() {
        let o = r#""AugmentSmallIconPath": "a/b.tex", "AugmentPlatformId": 1305, "#;
        assert_eq!(field(o, "AugmentSmallIconPath"), Some("a/b.tex"));
        assert_eq!(field(o, "AugmentPlatformId"), Some("1305"));
        assert_eq!(field(r#""Enabled": false,"#, "Enabled"), Some("false"));
        assert_eq!(field(o, "Missing"), None);
    }
}
