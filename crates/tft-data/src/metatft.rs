//! MetaTFT's unofficial JSON (`https://api-hc.metatft.com`, what metatft.com itself calls).
//!
//! Findings (checked 2026-10-25, Set 18, patch 18.5):
//! - `tft-comps-api/comps_data` ignores `queue`: one clustering (60 comps) for every queue.
//!   A cluster's `name_string` (e.g. "DA_18_Defender, DA_18_Rengar") is the curated name the site
//!   shows, falling back to `name[]`. `traits_string` entries are `<trait>_<tier index>`
//!   ("DA_18_Defender_3" = 6 Defenders), not unit counts. `top_augments` is always empty.
//! - `tft-comps-api/comps_stats` and `tft-stat-api/{units,items,traits}` do differ per queue:
//!   placement histograms (`places`, 1st..8th; comps add the total as a 9th entry). The site
//!   sends the rank filter only for ranked (1100); with it, Choncc (1230) has ~170 games instead
//!   of ~1M. Normal (1090) has a few dozen games, so small samples fall back to ranked.
//!   Trait stats are keyed `<trait>_<tier index>` too ("DA_18_Adaptor_2").
//! - There are no augment placement stats: `tft-stat-api/augments` answers 500 and the explorer's
//!   `tft-explorer-api/augments` returns `augments: null` (Riot's match data has no augments
//!   since Set 13). The site shows hand-made tier lists instead: `tft-comps-api/comp_augment_tiers`
//!   (per cluster, `{id, tier: S|A|B}`, 39 of 60 clusters) and `tft-stat-api/augments_tiers`
//!   (one S..D list, `content.content.tierList[{label, content[{id}]}]`).

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tft_model::{Api, CarryBuild, MetaComp, PlaceStat, QUEUE_RANKED, TftMeta, TftSetData};

const BASE: &str = "https://api-hc.metatft.com";
const TTL: Duration = Duration::from_secs(6 * 3600);
/// Below this many games in the best-sampled row, a queue's stats are ranked's instead.
pub const MIN_SAMPLE: u32 = 1000;
const RANKS: &str = "CHALLENGER,DIAMOND,EMERALD,GRANDMASTER,MASTER";
const TIER_ORDER: &str = "SABCD";

/// Comps (sorted by average placement), unit, item and trait stats for `queue`. Stats come from
/// that queue unless its sample is under [`MIN_SAMPLE`] (then ranked); comps without a row in the
/// stats keep the clustering's own `overall` numbers. `augments` stays empty (no source; see the
/// module docs); comp augments come from MetaTFT's per-comp tier lists, best first.
pub async fn meta(http: &data::Http, queue: i64, set: &TftSetData) -> anyhow::Result<TftMeta> {
    let url = format!("{BASE}/tft-comps-api/comps_data?queue={QUEUE_RANKED}");
    let (updated, mut comps) = http.get_parsed(&url, TTL, |s| parse_comps(s, set)).await?;

    match queue_stats(http, "/tft-comps-api/comps_stats", queue, parse_comp_stats).await {
        Ok(stats) => {
            let by_id: HashMap<&str, &PlaceStat> = stats.iter().map(|s| (s.api.as_str(), s)).collect();
            for c in &mut comps {
                if let Some(s) = by_id.get(c.id.as_str()).filter(|s| s.games > 0) {
                    (c.games, c.avg_place) = (s.games, s.avg_place);
                }
            }
        }
        Err(e) => tracing::warn!("metatft comp stats: {e:#}"),
    }
    match http.get_parsed(&format!("{BASE}/tft-comps-api/comp_augment_tiers"), TTL, parse_comp_augments).await {
        Ok(mut augs) => comps.iter_mut().for_each(|c| c.augments = augs.remove(&c.id).unwrap_or_default()),
        Err(e) => tracing::warn!("metatft comp augments: {e:#}"),
    }
    comps.sort_by(|a, b| a.avg_place.total_cmp(&b.avg_place));

    let stats = |path: &'static str| async move {
        queue_stats(http, path, queue, parse_stats).await.unwrap_or_else(|e| {
            tracing::warn!("metatft {path}: {e:#}");
            Vec::new()
        })
    };
    let (units, items, traits) =
        tokio::join!(stats("/tft-stat-api/units"), stats("/tft-stat-api/items"), stats("/tft-stat-api/traits"));
    Ok(TftMeta { queue, updated, comps, units, items, traits, augments: Vec::new() })
}

/// MetaTFT's overall augment tier list as (augment, "S" | "A" | "B" | "C" | "D"), best first.
pub async fn augment_tiers(http: &data::Http) -> anyhow::Result<Vec<(Api, String)>> {
    http.get_parsed(&format!("{BASE}/tft-stat-api/augments_tiers"), TTL, parse_augment_tiers).await
}

/// The filters metatft.com sends: current patch, 2 days, Emerald+ for ranked only.
fn filters(queue: i64) -> String {
    let rank = if queue == QUEUE_RANKED { format!("&rank={RANKS}") } else { String::new() };
    format!("queue={queue}&patch=current&days=2&permit_filter_adjustment=true{rank}")
}

async fn queue_stats(
    http: &data::Http,
    path: &str,
    queue: i64,
    parse: fn(&str) -> anyhow::Result<Vec<PlaceStat>>,
) -> anyhow::Result<Vec<PlaceStat>> {
    let url = |q: i64| format!("{BASE}{path}?{}", filters(q));
    let rows = http.get_parsed(&url(queue), TTL, parse).await;
    if queue == QUEUE_RANKED {
        return rows;
    }
    match rows {
        Ok(r) if r.iter().map(|s| s.games).max().unwrap_or(0) >= MIN_SAMPLE => Ok(r),
        other => {
            if let Err(e) = other {
                tracing::warn!("metatft {path} for queue {queue}: {e:#}");
            }
            tracing::info!("metatft {path}: queue {queue} sample too small, using ranked");
            http.get_parsed(&url(QUEUE_RANKED), TTL, parse).await
        }
    }
}

// ---- pure parsers ----

#[derive(Deserialize)]
struct CompsData {
    updated: u64,
    results: CompsResults,
}

#[derive(Deserialize)]
struct CompsResults {
    data: CompsInner,
}

#[derive(Deserialize)]
struct CompsInner {
    cluster_details: HashMap<String, Cluster>,
}

#[derive(Deserialize)]
struct Cluster {
    units_string: String,
    traits_string: String,
    #[serde(default)]
    name: Vec<NamePart>,
    #[serde(default)]
    name_string: Option<String>,
    overall: Overall,
    #[serde(default)]
    stars: Vec<String>,
    #[serde(default)]
    builds: Vec<Build>,
    #[serde(default, rename = "top_itemNames")]
    top_items: Vec<TopItem>,
    #[serde(default)]
    levelling: Option<String>,
}

#[derive(Deserialize)]
struct NamePart {
    name: String,
}

#[derive(Deserialize)]
struct Overall {
    count: u32,
    avg: f32,
}

#[derive(Deserialize)]
struct Build {
    unit: String,
    #[serde(rename = "buildName")]
    items: Vec<String>,
    avg: f32,
    count: u32,
}

#[derive(Deserialize)]
struct TopItem {
    #[serde(rename = "itemNames")]
    item: String,
}

fn list(s: &str) -> impl Iterator<Item = &str> {
    s.split(',').map(str::trim).filter(|x| !x.is_empty())
}

/// `comps_data` → (`updated` unix ms, comps in id order). Trait tiers become unit counts and
/// names become display names through `set`. `augments` is left empty.
pub fn parse_comps(json: &str, set: &TftSetData) -> anyhow::Result<(u64, Vec<MetaComp>)> {
    let data: CompsData = serde_json::from_str(json)?;
    let mut comps: Vec<MetaComp> = data
        .results
        .data
        .cluster_details
        .into_iter()
        .map(|(id, c)| {
            let traits = list(&c.traits_string)
                .filter_map(|t| {
                    let (api, tier) = t.rsplit_once('_')?;
                    let tier: usize = tier.parse().ok()?;
                    let units = set.traits.get(api).and_then(|t| t.breakpoints.get(tier.checked_sub(1)?).copied());
                    Some((api.to_owned(), units.unwrap_or(tier as u32)))
                })
                .collect();
            let parts: Vec<&str> = match c.name_string.as_deref().filter(|s| !s.trim().is_empty()) {
                Some(s) => list(s).collect(),
                None => c.name.iter().map(|n| n.name.as_str()).collect(),
            };
            let builds = c
                .builds
                .into_iter()
                .map(|b| CarryBuild { unit: b.unit, items: b.items, avg_place: b.avg, games: b.count })
                .collect();
            MetaComp {
                name: comp_name(&parts, set),
                id,
                units: list(&c.units_string).map(str::to_owned).collect(),
                stars: c.stars,
                traits,
                avg_place: c.overall.avg,
                games: c.overall.count,
                levelling: c.levelling.filter(|l| !l.is_empty()),
                builds,
                items: c.top_items.into_iter().map(|t| t.item).collect(),
                augments: Vec::new(),
            }
        })
        .collect();
    comps.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((data.updated, comps))
}

/// "Defender Rengar", "Juggernaut Zyra & Sejuani": traits (and anything else) first, then units.
/// Unknown API names are kept as they are.
pub fn comp_name(parts: &[&str], set: &TftSetData) -> String {
    let mut head = Vec::new();
    let mut units = Vec::new();
    for &p in parts {
        match set.units.get(p) {
            Some(u) => units.push(u.name.as_str()),
            None => head.push(
                set.traits
                    .get(p)
                    .map(|t| t.name.as_str())
                    .or_else(|| set.augments.get(p).map(|a| a.name.as_str()))
                    .or_else(|| set.items.get(p).map(|i| i.name.as_str()))
                    .unwrap_or(p),
            ),
        }
    }
    let units = units.join(" & ");
    head.into_iter().chain((!units.is_empty()).then_some(units.as_str())).collect::<Vec<_>>().join(" ")
}

/// `comps_stats` → one [`PlaceStat`] per cluster (`api` = cluster id); the total row is dropped.
pub fn parse_comp_stats(json: &str) -> anyhow::Result<Vec<PlaceStat>> {
    let v: Value = serde_json::from_str(json)?;
    let rows = v["results"].as_array().ok_or_else(|| anyhow::anyhow!("no results"))?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            let id = r["cluster"].as_str().filter(|c| !c.is_empty())?;
            Some(crate::place_stat(id, &places(&r["places"])))
        })
        .collect())
}

/// `tft-stat-api/{units,items,traits}` → stats keyed by `unit` / `itemName` / `trait`.
pub fn parse_stats(json: &str) -> anyhow::Result<Vec<PlaceStat>> {
    let v: Value = serde_json::from_str(json)?;
    let rows = v["results"].as_array().ok_or_else(|| anyhow::anyhow!("no results"))?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            let api = ["unit", "itemName", "trait"].iter().find_map(|k| r[*k].as_str())?;
            Some(crate::place_stat(api, &places(&r["places"])))
        })
        .collect())
}

fn places(v: &Value) -> Vec<u32> {
    v.as_array().into_iter().flatten().map(|n| n.as_u64().unwrap_or(0) as u32).collect()
}

/// `comp_augment_tiers` → cluster id → augments, best tier first (MetaTFT order within a tier).
pub fn parse_comp_augments(json: &str) -> anyhow::Result<HashMap<String, Vec<Api>>> {
    let v: Value = serde_json::from_str(json)?;
    let results = v["results"].as_object().ok_or_else(|| anyhow::anyhow!("no results"))?;
    Ok(results
        .iter()
        .map(|(id, c)| {
            let mut augs: Vec<(usize, Api)> = c["augments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| Some((tier_rank(a["tier"].as_str()?), a["id"].as_str()?.to_owned())))
                .collect();
            augs.sort_by_key(|a| a.0);
            (id.clone(), augs.into_iter().map(|a| a.1).collect())
        })
        .collect())
}

/// `augments_tiers` → (augment, tier label), best first.
pub fn parse_augment_tiers(json: &str) -> anyhow::Result<Vec<(Api, String)>> {
    let v: Value = serde_json::from_str(json)?;
    let tiers = v["content"]["content"]["tierList"].as_array().ok_or_else(|| anyhow::anyhow!("no tierList"))?;
    let mut out: Vec<(Api, String)> = tiers
        .iter()
        .flat_map(|t| {
            let label = t["label"].as_str().unwrap_or("?");
            let ids = t["content"].as_array().into_iter().flatten().filter_map(|a| a["id"].as_str());
            ids.map(move |id| (id.to_owned(), label.to_owned()))
        })
        .collect();
    out.sort_by_key(|a| tier_rank(&a.1));
    Ok(out)
}

fn tier_rank(tier: &str) -> usize {
    TIER_ORDER.find(tier).filter(|_| tier.len() == 1).unwrap_or(TIER_ORDER.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use std::sync::LazyLock;

    static SET18: LazyLock<TftSetData> =
        LazyLock::new(|| crate::parse_set_data(&fixture("cdragon_set18.json"), 18).unwrap());

    fn comps() -> (u64, Vec<MetaComp>) {
        parse_comps(&fixture("metatft_comps_data.json"), &SET18).unwrap()
    }

    #[test]
    fn comps_data() {
        let (updated, comps) = comps();
        assert_eq!(updated, 1792888544264);
        assert_eq!(comps.len(), 5);
        let c = &comps[0];
        assert_eq!((c.id.as_str(), c.name.as_str()), ("426000", "Defender Rengar"));
        assert_eq!(c.units.len(), 7);
        assert!(c.units.contains(&"DA_Fiddlesticks18".into()));
        assert_eq!(c.traits[0], ("DA_18_Defender".into(), 6), "tier 3 of 2/4/6");
        assert!(c.traits.contains(&("DA_18_Rival".into(), 1)));
        assert_eq!((c.games, c.avg_place, c.levelling.as_deref()), (10532, 4.2907, Some("lvl 7")));
        assert_eq!(c.stars[0], "DA_18_Rengar");
        let b = &c.builds[0];
        assert_eq!((b.unit.as_str(), b.games, b.avg_place), ("DA_18_Rengar", 3528, 4.0686));
        assert_eq!(b.items, ["DA_18_EmblemDefender", "DA_GuinsoosRageblade", "DA_TitansResolve"]);
        assert_eq!(c.items[..2], ["DA_18_EmblemDefender".to_owned(), "DA_ArchangelsStaff".to_owned()]);
        assert!(c.augments.is_empty());

        let names: Vec<_> = comps.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Defender Rengar", "Sprykin Teemo", "Riftbeast Pebbles", "Invoker Ahri", "Juggernaut Zyra & Sejuani"]);
        assert_eq!(comps[1].levelling.as_deref(), Some("lvl 6"));
        // Every unit, item and trait MetaTFT names exists in the Set 18 data.
        for c in &comps {
            assert!(c.units.iter().chain(&c.stars).all(|u| SET18.units.contains_key(u)), "{}", c.id);
            assert!(c.items.iter().chain(c.builds.iter().flat_map(|b| &b.items)).all(|i| SET18.items.contains_key(i)));
            assert!(c.traits.iter().all(|(t, _)| SET18.traits.contains_key(t)));
        }
    }

    #[test]
    fn comp_names() {
        assert_eq!(comp_name(&["DA_18_Lunar", "DA_18_Aphelios", "DA_Nidalee18_AP"], &SET18), "Lunar Aphelios & Nidalee");
        assert_eq!(comp_name(&["DA_18_Slayer", "Mystery"], &SET18), "Ravager Mystery");
        assert_eq!(comp_name(&["DA_18_Ahri"], &SET18), "Ahri");
    }

    #[test]
    fn comp_stats() {
        let ranked = parse_comp_stats(&fixture("metatft_comps_stats_1100.json")).unwrap();
        assert!(ranked.iter().all(|s| !s.api.is_empty()), "total row dropped");
        let s = ranked.iter().find(|s| s.api == "426000").unwrap();
        // places [1556, 1936, 1714, 1672, 1650, 1540, 1387, 1071] (+ total 12526)
        assert_eq!(s.games, 12526);
        assert!((s.avg_place - 53025.0 / 12526.0).abs() < 1e-4, "{}", s.avg_place);
        assert!((s.top4 - 6878.0 / 12526.0).abs() < 1e-4);

        let choncc = parse_comp_stats(&fixture("metatft_comps_stats_1230.json")).unwrap();
        let s = choncc.iter().find(|s| s.api == "426000").unwrap();
        assert_eq!(s.games, 982, "Choncc has its own comp stats");
        let normal = parse_comp_stats(&fixture("metatft_comps_stats_1090.json")).unwrap();
        assert!(normal.iter().map(|s| s.games).max().unwrap() < MIN_SAMPLE, "normals fall back to ranked");
    }

    #[test]
    fn stat_api() {
        let units = parse_stats(&fixture("metatft_units_1230.json")).unwrap();
        assert_eq!(units.len(), 69);
        let ahri = units.iter().find(|s| s.api == "DA_18_Ahri").unwrap();
        // places [47450, 42245, 45042, 49091, 51036, 50419, 45648, 34878]
        assert_eq!(ahri.games, 365809);
        assert!((ahri.top4 - 183828.0 / 365809.0).abs() < 1e-4);
        let items = parse_stats(&fixture("metatft_items_1230.json")).unwrap();
        assert!(items.iter().any(|s| s.api == "DA_18_EmblemBlossom"));
        let traits = parse_stats(&fixture("metatft_traits_1230.json")).unwrap();
        assert!(traits.iter().any(|s| s.api == "DA_18_Defender_3"), "trait stats keep the tier suffix");
        let normal = parse_stats(&fixture("metatft_units_1090.json")).unwrap();
        assert!(normal.iter().map(|s| s.games).max().unwrap() < MIN_SAMPLE);
        assert!(parse_stats("{}").is_err());
    }

    #[test]
    fn augment_lists() {
        let per_comp = parse_comp_augments(&fixture("metatft_comp_augment_tiers.json")).unwrap();
        assert_eq!(per_comp.len(), 4, "426006 has no list");
        assert_eq!(per_comp["426000"].len(), 65);
        assert_eq!(per_comp["426000"][0], "DA_BoxingLessons");
        assert_eq!(per_comp["426007"], ["DA_CursedCrown", "DA_Coronation", "DA_18_RiftbeastTraitAugment"]);

        let tiers = parse_augment_tiers(&fixture("metatft_augments_tiers.json")).unwrap();
        assert_eq!(tiers.len(), 28 + 85 + 125 + 21);
        assert_eq!(tiers[0], ("DA_PandorasBench".into(), "S".into()));
        assert_eq!(tiers.last().unwrap().1, "C");
        let known = tiers.iter().filter(|(a, _)| SET18.augments.contains_key(a)).count();
        assert!(known > tiers.len() - 15, "a few ids are newer than Community Dragon: {known}/{}", tiers.len());
    }

    #[test]
    fn urls() {
        assert_eq!(filters(1100), format!("queue=1100&patch=current&days=2&permit_filter_adjustment=true&rank={RANKS}"));
        assert_eq!(filters(1230), "queue=1230&patch=current&days=2&permit_filter_adjustment=true");
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_meta() {
        let http = crate::live_http();
        let set = crate::set_data(&http).await.unwrap();
        for queue in [1100, 1230] {
            let m = meta(&http, queue, &set).await.unwrap();
            assert!(m.comps.len() > 20 && !m.units.is_empty() && !m.items.is_empty() && !m.traits.is_empty(), "{queue}");
            assert!(m.comps.iter().any(|c| !c.augments.is_empty()));
            assert!(m.comps.windows(2).all(|w| w[0].avg_place <= w[1].avg_place));
        }
        assert!(augment_tiers(&http).await.unwrap().len() > 50);
    }
}
