//! One-per-inventory item groups (Last Whisper, Lifeline, Boots, …) from the
//! game's own item data on CommunityDragon. Data Dragon has no such field.

use model_static::ItemId;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Per-patch file; the bin is ~16 MB, so it is cached for a week.
const TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// Item → indices of the limit-1 groups it belongs to, keeping only groups with
/// at least two of `known` items (`version` like "16.21.1").
pub async fn load(http: &crate::Http, version: &str, known: &HashSet<ItemId>) -> anyhow::Result<HashMap<ItemId, Vec<u32>>> {
    let patch: String = version.split('.').take(2).collect::<Vec<_>>().join(".");
    let url = |p: &str| format!("https://raw.communitydragon.org/{p}/game/items.cdtb.bin.json");
    let parse_known = |s: &str| parse(s, known);
    match http.get_parsed(&url(&patch), TTL, parse_known).await {
        Ok(g) => Ok(g),
        Err(e) => {
            tracing::warn!("item groups for {patch}: {e:#}; trying latest");
            http.get_parsed(&url("latest"), Duration::from_secs(24 * 3600), parse_known).await
        }
    }
}

pub fn parse(json: &str, known: &HashSet<ItemId>) -> anyhow::Result<HashMap<ItemId, Vec<u32>>> {
    let root: Value = serde_json::from_str(json)?;
    let obj = root.as_object().ok_or_else(|| anyhow::anyhow!("items bin is not an object"))?;
    let limited = |key: &str| obj.get(key).and_then(|g| g["mMaxGroupOwnable"].as_u64()) == Some(1);

    let mut members: HashMap<&str, Vec<ItemId>> = HashMap::new();
    for v in obj.values() {
        let Some(id) = v["itemID"].as_u64().map(|i| i as ItemId).filter(|i| known.contains(i)) else { continue };
        for g in v["mItemGroups"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if limited(g) {
                members.entry(g).or_default().push(id);
            }
        }
    }
    let mut keys: Vec<&str> = members.iter().filter(|(_, m)| m.len() > 1).map(|(k, _)| *k).collect();
    keys.sort_unstable();
    anyhow::ensure!(!keys.is_empty(), "no item groups found");
    let mut out: HashMap<ItemId, Vec<u32>> = HashMap::new();
    for (i, k) in keys.iter().enumerate() {
        for &id in &members[k] {
            out.entry(id).or_default().push(i as u32);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_whisper_and_singletons() {
        let json = r#"{
            "Items/3071": {"itemID": 3071, "mItemGroups": ["Items/ItemGroups/Default", "{24adb712}", "Items/ItemGroups/LastWhisper"]},
            "Items/3036": {"itemID": 3036, "mItemGroups": ["Items/ItemGroups/Default", "Items/ItemGroups/LastWhisper"]},
            "Items/3157": {"itemID": 3157, "mItemGroups": ["Items/ItemGroups/Default"]},
            "Items/ItemGroups/Default": {"mItemGroupID": "Default", "__type": "ItemGroup"},
            "Items/ItemGroups/LastWhisper": {"mItemGroupID": "LastWhisper", "mMaxGroupOwnable": 1, "__type": "ItemGroup"},
            "{24adb712}": {"mItemGroupID": "{24adb712}", "mMaxGroupOwnable": 1, "__type": "ItemGroup"}
        }"#;
        let known: HashSet<ItemId> = [3071, 3036, 3157].into();
        let g = parse(json, &known).unwrap();
        assert_eq!(g[&3071], g[&3036], "cleaver and LDR share Last Whisper");
        assert_eq!(g[&3071].len(), 1, "single-item and unlimited groups are dropped");
        assert!(!g.contains_key(&3157));
    }
}
