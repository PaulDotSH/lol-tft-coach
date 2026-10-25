//! Community Dragon `cdragon/tft/en_us.json` (~24 MB) → [`TftSetData`] for one set.
//!
//! Findings (checked 2026-10-25 on the 16.21 file, Set 18):
//! - `setData[]` has one entry per set *and mode* (`TFTSet18`, `TFTSet16_PAIRS`, …); the standard
//!   set is the entry with `mutator == "TFTSet<n>"`. It holds full `champions` and `traits`, and
//!   only the API names of its `items` and `augments`; their objects are in the top-level `items[]`.
//! - Champion `traits` are display names ("Ravager"), not API names (`DA_18_Slayer`).
//! - Units with no traits or cost ≥ 6 are jungle monsters, dummies and anvils, not shop units.
//! - The set's lists also carry older sets' entries (`TFT_Item_*`, `TFT9_Augment_*`, often with
//!   the same display name as the `DA_` copy). Set 18 uses only `DA_` names (LCU history and
//!   MetaTFT agree), so only those are kept.
//! - Item kinds come from `tags` (hashed bin strings): `component`; `{6ef5c598}` radiant;
//!   `{44ace175}` artifact; `{5b609ae2}` the set mechanic's ~350 wands (dropped). Emblems have their
//!   icon under `…/traits/spatula/`. Craftable items, emblems included, have a two-item
//!   `composition`. Consumables (Reforger, duplicators, potions, boosters) are kept as plain items.
//! - Augment tier tags: `{d11fd6d5}` silver, `{ce1fd21c}` gold, `{cf1fd3af}` prismatic.
//! - Choncc or Treat's items (`TFT_Item_ChonccsCrown`, `…Chalice`, `…Spork`) belong to the event's
//!   own set entry, not Set 18; they are added so the Choncc hooks can name them.
//! - Descriptions use `@Var@` / `@Var*100@` placeholders; values are in `effects`, keyed by the
//!   name or by `{fnv1a32(lowercase name)}`.

use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tft_model::{AugmentTier, TftAugment, TftItem, TftSetData, TftTrait, TftUnit};

const URL: &str = "https://raw.communitydragon.org/latest/cdragon/tft/en_us.json";
const METADATA: &str = "https://raw.communitydragon.org/latest/content-metadata.json";
const TTL: Duration = Duration::from_secs(12 * 3600);
/// Item icons change with patches only.
const ICON_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// Set data for [`crate::SET`]. `version` is Community Dragon's game version ("16.21.8298620"),
/// or "latest" when its metadata didn't answer.
pub async fn set_data(http: &data::Http) -> anyhow::Result<TftSetData> {
    let version = http.get_parsed(METADATA, TTL, parse_version).await.unwrap_or_else(|e| {
        tracing::warn!("cdragon version: {e:#}");
        "latest".into()
    });
    let set = http.get_parsed(URL, TTL, |s| parse_set_data(s, crate::SET)).await?;
    Ok(TftSetData { version, ..set })
}

/// Browser URL of a Community Dragon asset path (the `icon` fields): lowercase, under
/// `latest/game/`, `.tex` → `.png`.
pub fn icon_url(path: &str) -> String {
    let p = path.to_lowercase();
    let p = p.strip_suffix(".tex").or_else(|| p.strip_suffix(".dds")).map(|s| format!("{s}.png")).unwrap_or(p);
    format!("https://raw.communitydragon.org/latest/game/{p}")
}

/// Every item's icon (PNG bytes, disk-cached), eight requests at a time; failures are left out.
pub async fn item_icons(http: &data::Http, set: &TftSetData) -> Vec<(String, Vec<u8>)> {
    let mut urls: Vec<String> = set.items.values().map(|i| icon_url(&i.icon)).collect();
    urls.sort();
    urls.dedup();
    let mut bodies: HashMap<String, Vec<u8>> = HashMap::new();
    for chunk in urls.chunks(8) {
        let mut tasks = tokio::task::JoinSet::new();
        for url in chunk {
            let (http, url) = (http.clone(), url.clone());
            tasks.spawn(async move { (data::icons::get_bytes(&http, &url, ICON_TTL).await, url) });
        }
        while let Some(Ok((res, url))) = tasks.join_next().await {
            match res {
                Ok(b) => {
                    bodies.insert(url, b);
                }
                Err(e) => tracing::warn!("tft item icon: {e:#}"),
            }
        }
    }
    set.items.values().filter_map(|i| Some((i.api.clone(), bodies.get(&icon_url(&i.icon))?.clone()))).collect()
}

pub fn parse_version(json: &str) -> anyhow::Result<String> {
    let v: Value = serde_json::from_str(json)?;
    let full = v["version"].as_str().context("no version")?;
    Ok(full.split('+').next().unwrap_or(full).to_owned())
}

#[derive(Deserialize)]
struct Root {
    items: Vec<RawItem>,
    #[serde(rename = "setData")]
    sets: Vec<RawSet>,
}

#[derive(Deserialize)]
struct RawSet {
    number: u32,
    mutator: String,
    champions: Vec<RawUnit>,
    traits: Vec<RawTrait>,
    items: Vec<String>,
    augments: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUnit {
    api_name: String,
    name: Option<String>,
    cost: u32,
    traits: Vec<String>,
    square_icon: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrait {
    api_name: String,
    name: String,
    effects: Vec<RawEffect>,
    icon: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEffect {
    min_units: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawItem {
    api_name: String,
    name: Option<String>,
    composition: Vec<String>,
    tags: Vec<String>,
    icon: String,
    desc: Option<String>,
    effects: HashMap<String, Option<f64>>,
}

impl RawItem {
    fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.api_name.clone())
    }

    fn tagged(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

const TAG_RADIANT: &str = "{6ef5c598}";
const TAG_ARTIFACT: &str = "{44ace175}";
const TAG_WAND: &str = "{5b609ae2}";
const AUGMENT_TIERS: [(&str, AugmentTier); 3] =
    [("{d11fd6d5}", AugmentTier::Silver), ("{ce1fd21c}", AugmentTier::Gold), ("{cf1fd3af}", AugmentTier::Prismatic)];

/// Units, traits, items and augments of set `set` (`version` left empty).
pub fn parse_set_data(json: &str, set: u32) -> anyhow::Result<TftSetData> {
    let root: Root = serde_json::from_str(json)?;
    let mutator = format!("TFTSet{set}");
    let s = root.sets.iter().find(|s| s.number == set && s.mutator == mutator).with_context(|| format!("no {mutator}"))?;
    let by_api: HashMap<&str, &RawItem> = root.items.iter().map(|i| (i.api_name.as_str(), i)).collect();

    let traits: HashMap<String, TftTrait> = s
        .traits
        .iter()
        .map(|t| {
            let breakpoints = t.effects.iter().filter_map(|e| e.min_units).collect();
            let tr = TftTrait { api: t.api_name.clone(), name: t.name.clone(), breakpoints, icon: t.icon.clone() };
            (t.api_name.clone(), tr)
        })
        .collect();
    let trait_api: HashMap<&str, &str> = s.traits.iter().map(|t| (t.name.as_str(), t.api_name.as_str())).collect();

    let units = s
        .champions
        .iter()
        .filter(|u| u.cost <= 5 && !u.traits.is_empty())
        .map(|u| {
            let traits = u.traits.iter().map(|t| trait_api.get(t.as_str()).copied().unwrap_or(t).to_owned()).collect();
            let name = u.name.clone().unwrap_or_else(|| u.api_name.clone());
            let unit = TftUnit { api: u.api_name.clone(), name, cost: u.cost, traits, icon: u.square_icon.clone() };
            (u.api_name.clone(), unit)
        })
        .collect();

    let choncc = root.items.iter().filter(|i| i.api_name.starts_with("TFT_Item_Choncc"));
    let items = s
        .items
        .iter()
        .filter(|a| a.starts_with("DA_"))
        .filter_map(|a| by_api.get(a.as_str()).copied())
        .filter(|i| !i.tagged(TAG_WAND))
        .chain(choncc)
        .map(|i| {
            let item = TftItem {
                api: i.api_name.clone(),
                name: i.name(),
                components: if i.composition.len() == 2 { i.composition.clone() } else { Vec::new() },
                component: i.tagged("component"),
                emblem: i.icon.contains("/spatula/"),
                radiant: i.tagged(TAG_RADIANT),
                artifact: i.tagged(TAG_ARTIFACT),
                icon: i.icon.clone(),
            };
            (i.api_name.clone(), item)
        })
        .collect();

    let augments = s
        .augments
        .iter()
        .filter(|a| a.starts_with("DA_"))
        .filter_map(|a| by_api.get(a.as_str()).copied())
        .map(|i| {
            let tier = AUGMENT_TIERS.iter().find(|(tag, _)| i.tagged(tag)).map_or_else(|| tier_from_icon(&i.icon), |t| t.1);
            let desc = render_desc(i.desc.as_deref().unwrap_or_default(), &i.effects);
            let aug = TftAugment { api: i.api_name.clone(), name: i.name(), tier, desc, icon: i.icon.clone() };
            (i.api_name.clone(), aug)
        })
        .collect();

    Ok(TftSetData { version: String::new(), set, units, traits, items, augments })
}

/// Fallback when an augment has no tier tag: icons end in `_i`/`_ii`/`_iii` or `1`/`2`/`3`.
fn tier_from_icon(icon: &str) -> AugmentTier {
    let stem = icon.rsplit('/').next().unwrap_or(icon).trim_end_matches(".tex").to_lowercase();
    if stem.ends_with("iii") || stem.ends_with('3') {
        AugmentTier::Prismatic
    } else if stem.ends_with("ii") || stem.ends_with('2') {
        AugmentTier::Gold
    } else {
        AugmentTier::Silver
    }
}

/// Plain-text description: `@Var@` / `@Var*100@` filled from `effects`, `<br>` → newline,
/// other tags dropped. Unknown variables stay as `?`.
pub fn render_desc(desc: &str, effects: &HashMap<String, Option<f64>>) -> String {
    let value = |var: &str| {
        let (name, scale) = match var.split_once('*') {
            Some((n, s)) => (n.trim(), s.trim().parse::<f64>().unwrap_or(1.0)),
            None => (var.trim(), 1.0),
        };
        let v = effects.get(name).or_else(|| effects.get(&format!("{{{:08x}}}", fnv1a32(name)))).copied().flatten()?;
        let v = v * scale;
        Some(if (v - v.round()).abs() < 1e-6 { format!("{}", v.round()) } else { format!("{v:.1}") })
    };
    let mut out = String::new();
    let mut parts = desc.split('@');
    out.push_str(parts.next().unwrap_or_default());
    // Odd pieces are variable names, even ones literal text.
    for (i, p) in parts.enumerate() {
        if i % 2 == 0 {
            out.push_str(&value(p).unwrap_or_else(|| "?".into()));
        } else {
            out.push_str(p);
        }
    }
    let out = out.replace("<br>", "\n").replace("&nbsp;", " ");
    let mut plain = String::with_capacity(out.len());
    let mut in_tag = false;
    for c in out.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => plain.push(c),
            _ => {}
        }
    }
    plain.trim().to_owned()
}

/// Riot's bin hash for names: 32-bit FNV-1a of the lowercase name.
fn fnv1a32(s: &str) -> u32 {
    s.to_lowercase().bytes().fold(0x811c_9dc5, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    fn set18() -> TftSetData {
        parse_set_data(&fixture("cdragon_set18.json"), 18).unwrap()
    }

    #[test]
    fn units_and_traits() {
        let s = set18();
        assert_eq!(s.set, 18);
        assert_eq!(s.traits.len(), 36);
        // 91 champions minus 11 traitless monsters/dummies and 6 anvils/chests (cost 8, 11).
        assert_eq!(s.units.len(), 74);
        let rengar = &s.units["DA_18_Rengar"];
        assert_eq!((rengar.name.as_str(), rengar.cost, rengar.traits.clone()), ("Rengar", 3, vec!["DA_18_Rival".into()]));
        let akali = &s.units["DA_18_Akali_AD"];
        assert_eq!(akali.traits, ["DA_18_Inferno", "DA_18_Adaptor", "DA_18_Slayer"], "display names → API names");
        assert_eq!(akali.icon, "assets/characters/tft18_akali/skins/base/images/tft18_akali_splash_tile_2.tex");
        assert!(!s.units.contains_key("TFT_TrainingDummy") && !s.units.contains_key("TFT_ArmoryKeyOrnn"));
        let defender = &s.traits["DA_18_Defender"];
        assert_eq!((defender.name.as_str(), defender.breakpoints.clone()), ("Defender", vec![2, 4, 6]));
        assert_eq!(s.traits["DA_18_Rival"].breakpoints, [1, 1, 2], "raw tiers, so tier indices line up");
        assert!(s.traits["DA_18_Eclipse"].breakpoints.is_empty());
        assert!(s.units.values().flat_map(|u| &u.traits).all(|t| s.traits.contains_key(t)));
    }

    #[test]
    fn items() {
        let s = set18();
        let g = &s.items["DA_GuinsoosRageblade"];
        assert_eq!(g.name, "Guinsoo's Rageblade");
        assert_eq!(g.components, ["DA_Component_RecurveBow", "DA_Component_NeedlesslyLargeRod"]);
        assert!(!g.component && !g.emblem && !g.radiant && !g.artifact);
        let bow = &s.items["DA_Component_RecurveBow"];
        assert!(bow.component && bow.components.is_empty());
        assert_eq!(s.items.values().filter(|i| i.component).count(), 10);

        let emblem = &s.items["DA_18_EmblemBlossom"];
        assert!(emblem.emblem && !emblem.component);
        assert_eq!(emblem.components, ["DA_Component_Spatula", "DA_Component_NeedlesslyLargeRod"]);
        assert!(s.items["DA_18_EmblemDefender"].emblem && s.items["DA_18_EmblemDefender"].components.is_empty());

        let radiant = &s.items["DA_GuinsoosRagebladeRadiant"];
        assert!(radiant.radiant && radiant.components.is_empty());
        assert!(s.items["DA_Artifact_Fishbones"].artifact);
        assert!(!s.items["DA_Reforger"].radiant && s.items["DA_Reforger"].components.is_empty(), "consumables are plain");
        assert!(s.items.contains_key("TFT_Item_ChonccsCrown") && s.items.contains_key("TFT_Item_ChonccsChalice"));

        assert!(!s.items.contains_key("TFT_Item_GuinsoosRageblade"), "older sets' copies dropped");
        assert!(!s.items.contains_key("DA_18_AfterShock") && !s.items.contains_key("DA_PhantomEmblem18"), "wands dropped");
        assert!(s.items.values().flat_map(|i| &i.components).all(|c| s.items[c].component));
        assert_eq!(
            icon_url(&g.icon),
            "https://raw.communitydragon.org/latest/game/assets/maps/tft/icons/items/hexcore/tft_item_guinsoosrageblade.png"
        );
    }

    #[test]
    fn augments() {
        let s = set18();
        assert_eq!(s.augments.len(), 253);
        assert!(s.augments.keys().all(|a| a.starts_with("DA_")));
        let tier = |a: &str| s.augments[a].tier;
        assert_eq!(tier("DA_BandOfThieves"), AugmentTier::Silver);
        assert_eq!(tier("DA_BronzeForLifeI"), AugmentTier::Gold);
        assert_eq!(tier("DA_BandOfThievesII"), AugmentTier::Prismatic);
        let band = &s.augments["DA_BandOfThievesII"];
        assert_eq!(band.name, "Band of Thieves II");
        assert_eq!(band.desc, "Gain 2 Thief's Gloves. After 5 player combats, gain another.");
        assert!(s.augments.values().all(|a| !a.desc.contains('@') && !a.desc.contains('<')));
    }

    #[test]
    fn desc_rendering() {
        let fx = HashMap::from([("Heal".to_owned(), Some(0.25)), (format!("{{{:08x}}}", fnv1a32("Gold")), Some(3.0))]);
        let desc = "Heal @Heal*100@%.<br>Gain @Gold@ <TFTKeyword>gold</TFTKeyword>, @Nope@";
        assert_eq!(render_desc(desc, &fx), "Heal 25%.\nGain 3 gold, ?");
        assert_eq!(tier_from_icon("x/jeweled-lotus-iii.tex"), AugmentTier::Prismatic);
        assert_eq!(tier_from_icon("x/missing-t2.tex"), AugmentTier::Gold);
    }

    #[test]
    fn other_set_and_version() {
        // The fixture keeps a trimmed TFTSet17 entry (3 traitless monsters) next to Set 18.
        let s17 = parse_set_data(&fixture("cdragon_set18.json"), 17).unwrap();
        assert_eq!((s17.units.len(), s17.traits.len()), (0, 2));
        assert!(parse_set_data(&fixture("cdragon_set18.json"), 99).is_err());
        let v = parse_version(r#"{"version": "16.21.8298620+branch.releases-16-21.content.release"}"#).unwrap();
        assert_eq!(v, "16.21.8298620");
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_set_data() {
        let s = set_data(&crate::live_http()).await.unwrap();
        assert!(s.units.len() > 50 && s.traits.len() > 20 && s.items.len() > 100 && s.augments.len() > 100);
        assert!(s.version.contains('.'), "{}", s.version);
    }
}
