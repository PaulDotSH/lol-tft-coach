//! lolalytics `mega` endpoints. See the spec for the query format.
//!
//! Findings (verified 2026-10-22 against the website):
//! - `build-itemset` rows are `[ids_joined_by_"_", games, wins]`. `itemSet<k>`
//!   holds games whose *final* build had exactly k completed non-boot items, with
//!   the purchase order; `itemBootSet<k>` the same with boots counted. Summed over
//!   k they give the champion's games and overall win rate, which is why
//!   `itemSet1` alone looks like ~35% (short, lost games). Per-slot stats = sum of
//!   rows with ≥ j+1 items by the j-th id; this reproduces the website's
//!   "Item 4/5/6" tables exactly.
//! - `counter` ignores `queue`: it always returns Rift ranked data, so ARAM gets
//!   no counters and takes its win/pick rate from the build page.
//! - The build page's Qwik state has a header (`n`, `wr`, `pr`), per-game
//!   averages (`stats` rows `[name, int_flag, value, percentile, rank]`: gold,
//!   minions, jungleCS, damage, …; no vision) and a game-length histogram
//!   (`time`/`timeWin`, buckets 1..7 = 0-15, 15-20, …, 35-40, 40+ minutes).
//! - Runes (build page): `{pick, win}` summaries hold the most played and the
//!   highest win rate page (`runes.set` = `{pri: [keystone, 3], sec: [2], mod: [3]}`,
//!   `wr`, `n`); a `{stats}` object maps every rune id to `[[pick %, wr, games]]`,
//!   one row as a primary-tree pick and one as secondary (shards: one row; an
//!   `f` suffix, "5008f", is the shard taken in the flex row).
//! - Tier list: `ep=list` (`cid` → `{rank, tier, wr, pr, br, games}`), tier
//!   1..15 = S+ … D-, rank/tier 0 = not ranked in that lane.

use crate::num;
use model_static::{
    Benchmarks, ChampKey, ChampStats, Champion, EarlySet, ItemId, ItemStat, Lane, Matchup, MatchupBuild, Mode, RuneData, RunePage,
    RuneStat, TierEntry,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const MEGA: &str = "https://a1.lolalytics.com/mega/";
const SITE: &str = "https://lolalytics.com/lol";
const TTL: Duration = Duration::from_secs(12 * 3600);
/// Below this many games on the current patch, use the last 30 days.
pub const MIN_GAMES: u64 = 3000;
const FALLBACK_PATCH: &str = "30";
const MAX_EARLY: usize = 20;
const TIERS: [&str; 15] = ["S+", "S", "S-", "A+", "A", "A-", "B+", "B", "B-", "C+", "C", "C-", "D+", "D", "D-"];

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
    let lane = if mode == Mode::Rift { lane } else { None };
    let q = |patch: &str| Query { slug: &champ.slug, lane, mode, tier, region, patch: patch.to_owned() };

    let mut query = q(patch);
    let mut sets = http.get_parsed(&query.mega("build-itemset"), TTL, parse_itemsets).await?;
    if sets.games < MIN_GAMES && patch != FALLBACK_PATCH {
        let alt = q(FALLBACK_PATCH);
        match http.get_parsed(&alt.mega("build-itemset"), TTL, parse_itemsets).await {
            Ok(s) if s.games > sets.games => (sets, query) = (s, alt),
            Ok(_) => {}
            Err(e) => tracing::warn!("lolalytics 30-day fallback for {}: {e:#}", champ.slug),
        }
    }

    let early = http.get_parsed(&query.mega("build-earlyset"), TTL, parse_earlysets).await.unwrap_or_else(|e| {
        tracing::warn!("lolalytics earlyset {}: {e:#}", champ.slug);
        Vec::new()
    });
    let counters = match mode {
        Mode::Rift => Some(http.get_parsed(&query.mega("counter"), TTL, parse_counters).await?),
        _ => None,
    };
    let page = http.get_parsed(&query.page(), TTL, parse_build_page).await.unwrap_or_else(|e| {
        tracing::warn!("lolalytics build page {}: {e:#}", champ.slug);
        PageStats::default()
    });

    let set_wr = if sets.games > 0 { 100.0 * sets.wins as f32 / sets.games as f32 } else { 0.0 };
    let (win_rate, pick_rate, lane, lanes, counters) = match counters {
        Some(c) => (c.win_rate, c.pick_rate, lane.or(c.lane), c.lanes, c.counters),
        None => (page.win_rate.unwrap_or(set_wr), page.pick_rate.unwrap_or(0.0), None, HashMap::new(), Vec::new()),
    };
    Ok(ChampStats {
        champ: champ.key,
        lane,
        mode,
        patch: query.patch,
        win_rate,
        pick_rate,
        games: page.games.unwrap_or(sets.games),
        lanes,
        slots: sets.slots,
        boots: sets.boots,
        early,
        counters,
        benchmarks: page.benchmarks,
        runes: page.runes,
        coachless_runes: None,
        coachless: None,
        summoners: vec![],
        skill_order: page.skill_order,
        skill_order_games: page.skill_order_games,
        skill_priority: page.skill_priority,
    })
}

/// My win rate against every champion in another lane: `vslane` (support vs bot laners).
pub async fn counters_vs(
    http: &crate::Http,
    champ: &Champion,
    lane: Lane,
    vs_lane: Lane,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<Vec<Matchup>> {
    let q = Query { slug: &champ.slug, lane: Some(lane), mode: Mode::Rift, tier, region, patch: patch.to_owned() };
    let url = format!("{}&vslane={}", q.mega("counter"), vs_lane.as_str());
    Ok(http.get_parsed(&url, TTL, parse_counters).await?.counters)
}

/// The matchup page (`/lol/<me>/vs/<them>/build/`): win rate, starting and first items.
#[allow(clippy::too_many_arguments)]
pub async fn matchup(
    http: &crate::Http,
    me: &Champion,
    vs: &Champion,
    lane: Lane,
    vs_lane: Lane,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<MatchupBuild> {
    let (l, vl) = (lane.as_str(), vs_lane.as_str());
    // The page 404s on "region=all" (its default): only a real region goes in.
    let region = if region == "all" { String::new() } else { format!("&region={region}") };
    let url = format!("{SITE}/{}/vs/{}/build/?lane={l}&vslane={vl}&tier={tier}&patch={patch}{region}", me.slug, vs.slug);
    http.get_parsed(&url, TTL, |h| parse_matchup_page(h, vs.key)).await
}

/// Tier list for one lane (meta bans, pick candidates).
pub async fn tier_list(
    http: &crate::Http,
    lane: Lane,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<Vec<TierEntry>> {
    let url =
        format!("{MEGA}?ep=list&v=1&patch={patch}&lane={}&tier={tier}&queue=ranked&region={region}", lane.as_str());
    http.get_parsed(&url, TTL, |s| parse_tier_list(s, lane)).await
}

struct Query<'a> {
    slug: &'a str,
    lane: Option<Lane>,
    mode: Mode,
    tier: &'a str,
    region: &'a str,
    patch: String,
}

impl Query<'_> {
    /// ARAM: `queue=aram` and no lane. Mayhem has no data of its own, so it uses ARAM.
    fn mega(&self, ep: &str) -> String {
        let Query { slug, tier, region, patch, .. } = self;
        let queue = if self.mode == Mode::Rift { "ranked" } else { "aram" };
        let lane = self.lane.map(|l| format!("&lane={}", l.as_str())).unwrap_or_default();
        format!("{MEGA}?ep={ep}&v=1&patch={patch}&c={slug}{lane}&tier={tier}&queue={queue}&region={region}")
    }

    fn page(&self) -> String {
        let Query { slug, tier, region, patch, .. } = self;
        match (self.mode, self.lane) {
            (Mode::Rift, Some(l)) => {
                format!("{SITE}/{slug}/build/?lane={}&tier={tier}&patch={patch}&region={region}", l.as_str())
            }
            (Mode::Rift, None) => format!("{SITE}/{slug}/build/?tier={tier}&patch={patch}&region={region}"),
            _ => format!("{SITE}/{slug}/aram/build/?tier={tier}&patch={patch}&region={region}"),
        }
    }
}

// ---- pure parsers ----

#[derive(Debug, Default)]
pub struct ItemSets {
    /// `slots[j]` = items bought as the (j+1)-th completed item, most games first.
    pub slots: Vec<Vec<ItemStat>>,
    pub boots: Vec<ItemStat>,
    /// Games / wins with at least one completed item.
    pub games: u64,
    pub wins: u64,
}

type Row = (Vec<ItemId>, u32, u32);

fn set_rows(sets: &Value, key: &str) -> Vec<Row> {
    let Some(rows) = sets.get(key).and_then(Value::as_array) else { return Vec::new() };
    rows.iter()
        .filter_map(|r| {
            let ids = match &r[0] {
                Value::String(s) => s.split('_').filter_map(|x| x.parse().ok()).collect(),
                v => vec![v.as_u64()? as ItemId],
            };
            Some((ids, num(&r[1])? as u32, num(&r[2])? as u32))
        })
        .collect()
}

fn ranked(agg: HashMap<ItemId, (u32, u32)>) -> Vec<ItemStat> {
    let mut v: Vec<_> =
        agg.into_iter().filter(|(_, (g, _))| *g > 0).map(|(id, (games, wins))| ItemStat { id, games, wins }).collect();
    v.sort_unstable_by(|a, b| b.games.cmp(&a.games).then(a.id.cmp(&b.id)));
    v
}

pub fn parse_itemsets(json: &str) -> anyhow::Result<ItemSets> {
    let root: Value = serde_json::from_str(json)?;
    let sets = root.get("itemSets").ok_or_else(|| anyhow::anyhow!("no itemSets"))?;
    let mut out = ItemSets::default();
    let mut slots: Vec<HashMap<ItemId, (u32, u32)>> = vec![HashMap::new(); 5];
    let mut core_ids = HashSet::new();
    for k in 1..=5 {
        for (ids, n, w) in set_rows(sets, &format!("itemSet{k}")) {
            out.games += n as u64;
            out.wins += w as u64;
            for (j, id) in ids.into_iter().enumerate().take(5) {
                let e = slots[j].entry(id).or_default();
                (e.0, e.1) = (e.0 + n, e.1 + w);
                core_ids.insert(id);
            }
        }
    }
    // Boot ids are whatever the boot sets contain that the plain sets never do
    // (seen within the first five buys, so a rare 6th legendary isn't mistaken).
    let mut boots: HashMap<ItemId, (u32, u32)> = HashMap::new();
    let mut early = HashSet::new();
    for k in 1..=6 {
        for (ids, n, w) in set_rows(sets, &format!("itemBootSet{k}")) {
            for (j, id) in ids.into_iter().enumerate().filter(|(_, i)| !core_ids.contains(i)) {
                let e = boots.entry(id).or_default();
                (e.0, e.1) = (e.0 + n, e.1 + w);
                if j < 5 {
                    early.insert(id);
                }
            }
        }
    }
    boots.retain(|id, _| early.contains(id));
    out.slots = slots.into_iter().map(ranked).take_while(|s| !s.is_empty()).collect();
    out.boots = ranked(boots);
    Ok(out)
}

pub fn parse_earlysets(json: &str) -> anyhow::Result<Vec<EarlySet>> {
    let root: Value = serde_json::from_str(json)?;
    let rows = root.get("earlySet").and_then(Value::as_array).ok_or_else(|| anyhow::anyhow!("no earlySet"))?;
    let mut v: Vec<EarlySet> = rows
        .iter()
        .filter_map(|r| {
            Some(EarlySet {
                items: r[0].as_str()?.split('_').filter_map(|x| x.parse().ok()).collect(),
                win_rate: num(&r[1])? as f32,
                pick_rate: num(&r[2])? as f32,
                games: num(&r[3])? as u32,
            })
        })
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.games));
    v.truncate(MAX_EARLY);
    Ok(v)
}

#[derive(Debug)]
pub struct Counters {
    pub win_rate: f32,
    pub pick_rate: f32,
    /// Lane lolalytics answered for (its default when none was asked).
    pub lane: Option<Lane>,
    pub lanes: HashMap<Lane, f32>,
    pub counters: Vec<Matchup>,
}

pub fn parse_counters(json: &str) -> anyhow::Result<Counters> {
    let root: Value = serde_json::from_str(json)?;
    let stats = root.get("stats").ok_or_else(|| anyhow::anyhow!("no stats"))?;
    let lane = |v: &Value| v.as_str().and_then(Lane::from_riot);
    let lanes = stats["lanes"]
        .as_object()
        .map(|m| m.iter().filter_map(|(k, v)| Some((Lane::from_riot(k)?, num(v)? as f32))).collect())
        .unwrap_or_default();
    let counters = root["counters"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Matchup {
                        champ: c["cid"].as_u64()? as u32,
                        vs_wr: num(&c["vsWr"])? as f32,
                        games: num(&c["n"])? as u32,
                        default_lane: lane(&c["defaultLane"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Counters {
        win_rate: num(&stats["wr"]).unwrap_or(0.0) as f32,
        pick_rate: num(&stats["pr"]).unwrap_or(0.0) as f32,
        lane: lane(&stats["lane"]),
        lanes,
        counters,
    })
}

#[derive(Debug, Default, PartialEq)]
pub struct PageStats {
    pub games: Option<u64>,
    pub win_rate: Option<f32>,
    pub pick_rate: Option<f32>,
    pub benchmarks: Benchmarks,
    pub runes: Option<RuneData>,
    pub skill_order: String,
    pub skill_order_games: u32,
    pub skill_priority: String,
}

/// Header and per-minute benchmarks from a build page; missing parts are None.
/// Errors only when the page has no Qwik state at all (error/challenge page).
pub fn parse_build_page(html: &str) -> anyhow::Result<PageStats> {
    let q = crate::qwik::parse(html).ok_or_else(|| anyhow::anyhow!("no qwik/json state"))?;
    let header =
        q.find_map(2, &["queue", "cid", "n", "wr", "pr"], |h| Some((num(&h["n"])?, num(&h["wr"]), num(&h["pr"]))));
    let averages: HashMap<String, f64> = q
        .find_map(4, &["stats", "count"], |v| {
            let rows = v["stats"].as_array()?;
            let m: HashMap<_, _> = rows.iter().filter_map(|r| Some((r[0].as_str()?.to_owned(), num(&r[2])?))).collect();
            (!m.is_empty()).then_some(m)
        })
        .unwrap_or_default();
    let minutes = q.find_map(3, &["time", "timeWin"], |v| avg_minutes(&v["time"]));
    let pick = q.find_map(3, &["pick", "win"], |v| v["pick"]["skillorder"].is_object().then(|| v["pick"].clone())).unwrap_or_default();
    let (skill_order, skill_order_games) = skill_order(&pick);
    let per_min = |keys: &[&str]| {
        let total: Option<f64> = keys.iter().map(|k| averages.get(*k).copied()).sum();
        Some((total? / minutes?) as f32)
    };
    Ok(PageStats {
        games: header.map(|h| h.0 as u64),
        win_rate: header.and_then(|h| h.1).map(|v| v as f32),
        pick_rate: header.and_then(|h| h.2).map(|v| v as f32),
        benchmarks: Benchmarks {
            cs_per_min: per_min(&["minions", "jungleCS"]),
            gold_per_min: per_min(&["gold"]),
            damage_per_min: per_min(&["damage"]),
            vision_per_min: per_min(&["visionScore"]),
        },
        runes: parse_runes(&q),
        skill_order,
        skill_order_games,
        skill_priority: pick["skillpriority"]["id"].as_str().unwrap_or_default().to_owned(),
    })
}

/// Matchup page: the header (`vs`, `vsLane`, `n`, `wr`) and the most played
/// `summary.pick`: `items` (`start` set, then `item1`..`item3` with `id`), `runes.set`,
/// `sums.ids` and `skillpriority.id`.
pub fn parse_matchup_page(html: &str, vs: ChampKey) -> anyhow::Result<MatchupBuild> {
    let q = crate::qwik::parse(html).ok_or_else(|| anyhow::anyhow!("no qwik/json state"))?;
    let (games, win_rate) = q
        .find_map(2, &["vs", "vsLane", "n", "wr"], |h| Some((num(&h["n"])? as u32, num(&h["wr"])? as f32)))
        .ok_or_else(|| anyhow::anyhow!("no matchup header"))?;
    let pick = q.find_map(6, &["pick", "win"], |v| v["pick"]["items"]["start"].is_object().then(|| v["pick"].clone())).unwrap_or_default();
    let ids = |v: &Value| -> Vec<u32> { v.as_array().into_iter().flatten().filter_map(|x| num(x).map(|n| n as u32)).collect() };
    // item1..3 are single rows; later slots are lists of rows.
    let first_id = |v: &Value| num(if v.is_array() { &v[0]["id"] } else { &v["id"] }).map(|n| n as ItemId);
    let (it, set) = (&pick["items"], &pick["runes"]["set"]);
    let s = &it["start"];
    let (skill_order, skill_order_games) = skill_order(&pick);
    Ok(MatchupBuild {
        vs,
        games,
        win_rate,
        start: ids(&s["set"]),
        start_games: num(&s["n"]).unwrap_or(0.0) as u32,
        start_wr: num(&s["wr"]).unwrap_or(0.0) as f32,
        core: ["item1", "item2", "item3"].iter().filter_map(|k| first_id(&it[*k])).collect(),
        runes: ids(&set["pri"]).into_iter().chain(ids(&set["sec"])).collect(),
        rune_page: rune_page(&pick),
        summoners: ids(&pick["sums"]["ids"]),
        skills: pick["skillpriority"]["id"].as_str().unwrap_or_default().to_owned(),
        skill_order,
        skill_order_games,
    })
}

/// `skillorder` of a `pick` summary: `id` has one digit per level, 1..4 = Q W E R.
/// Empty with 0 games when a digit is outside 1..4 or the order is shorter than 6 levels.
fn skill_order(summary: &Value) -> (String, u32) {
    let row = &summary["skillorder"];
    let id = row["id"].as_u64().map(|n| n.to_string()).or_else(|| row["id"].as_str().map(str::to_owned)).unwrap_or_default();
    let order: Option<String> = id.chars().map(|c| "QWER".chars().nth((c.to_digit(10)? as usize).checked_sub(1)?)).collect();
    match order {
        Some(o) if o.len() >= 6 => (o, num(&row["n"]).unwrap_or(0.0) as u32),
        _ => (String::new(), 0),
    }
}

/// `{runes: {set: {pri, sec, mod}, wr, n}}` of a `pick` or `win` summary.
fn rune_page(r: &Value) -> Option<RunePage> {
    let ids = |v: &Value| -> Option<Vec<u32>> { v.as_array()?.iter().map(|x| num(x).map(|n| n as u32)).collect() };
    let set = &r["runes"]["set"];
    Some(RunePage {
        primary: ids(&set["pri"])?,
        secondary: ids(&set["sec"])?,
        shards: ids(&set["mod"])?,
        win_rate: num(&r["runes"]["wr"])? as f32,
        games: num(&r["runes"]["n"])? as u32,
    })
}

fn parse_runes(q: &crate::qwik::Qwik) -> Option<RuneData> {
    let (popular, best) = q.find_map(5, &["pick", "win"], |v| Some((rune_page(&v["pick"])?, rune_page(&v["win"])?)))?;
    // Keyed by rune id; other `{stats}` objects (per-game averages) are arrays.
    let stats = q
        .find_map(4, &["stats"], |v| {
            let m = v["stats"].as_object()?;
            let rows = |r: &Value| -> Option<Vec<RuneStat>> {
                r.as_array()?
                    .iter()
                    .map(|x| Some(RuneStat { pick_rate: num(&x[0])? as f32, win_rate: num(&x[1])? as f32, games: num(&x[2])? as u32 }))
                    .collect()
            };
            // "5008f": a shard taken in the flex (second) row; plain ids are their own row.
            let s: HashMap<u32, Vec<RuneStat>> = m.iter().filter_map(|(k, r)| Some((k.parse().ok()?, rows(r)?))).collect();
            (s.len() * 2 > m.len()).then_some(s)
        })
        .unwrap_or_default();
    Some(RuneData { popular, best, stats })
}

/// Mean game length from the `time` histogram (bucket midpoints).
fn avg_minutes(hist: &Value) -> Option<f64> {
    const MID: [f64; 7] = [12.5, 17.5, 22.5, 27.5, 32.5, 37.5, 44.0];
    let (n, sum) = hist.as_object()?.iter().fold((0.0, 0.0), |(n, sum), (k, v)| {
        match (k.parse::<usize>().ok().and_then(|i| MID.get(i.wrapping_sub(1))), num(v)) {
            (Some(m), Some(c)) => (n + c, sum + c * m),
            _ => (n, sum),
        }
    });
    (n > 0.0).then(|| sum / n)
}

pub fn parse_tier_list(json: &str, lane: Lane) -> anyhow::Result<Vec<TierEntry>> {
    let root: Value = serde_json::from_str(json)?;
    let rows = root.get("cid").and_then(Value::as_object).ok_or_else(|| anyhow::anyhow!("no cid map"))?;
    let mut v: Vec<(u64, TierEntry)> = rows
        .iter()
        .filter_map(|(cid, r)| {
            let rank = r["rank"].as_u64().filter(|&r| r > 0)?;
            let tier = r["tier"].as_u64().and_then(|t| TIERS.get((t as usize).checked_sub(1)?)).map(|s| s.to_string());
            Some((
                rank,
                TierEntry {
                    champ: cid.parse().ok()?,
                    lane,
                    win_rate: num(&r["wr"])? as f32,
                    pick_rate: num(&r["pr"]).unwrap_or(0.0) as f32,
                    ban_rate: num(&r["br"]).unwrap_or(0.0) as f32,
                    games: num(&r["games"]).unwrap_or(0.0) as u32,
                    tier,
                },
            ))
        })
        .collect();
    v.sort_by_key(|(rank, _)| *rank);
    Ok(v.into_iter().map(|(_, e)| e).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn itemsets_rift() {
        let s = parse_itemsets(&fixture("lolalytics-itemset-ahri-middle.json")).unwrap();
        assert_eq!(s.slots.len(), 5);
        // Matches the website's "Item 4" table (Rabadon's 3366 games, 54.75%).
        let rabadon = s.slots[2].iter().find(|i| i.id == 3157).unwrap();
        assert_eq!((rabadon.games, rabadon.wins), (3366, 1843));
        assert_eq!(s.slots[0][0].id, 3118, "Malignance first");
        let wr = 100.0 * s.wins as f64 / s.games as f64;
        assert!((51.0..54.0).contains(&wr), "overall wr {wr}");
        assert!(s.games > 15_000);
        let boots: Vec<_> = s.boots.iter().map(|b| b.id).collect();
        assert_eq!(boots[0], 3020, "Sorcerer's Shoes");
        assert!(boots.iter().all(|b| [3006, 3009, 3020, 3047, 3111, 3158, 3008].contains(b)), "{boots:?}");
    }

    #[test]
    fn itemsets_aram_and_garbage() {
        let s = parse_itemsets(&fixture("lolalytics-itemset-ahri-aram.json")).unwrap();
        assert!(s.games > 1000 && s.slots[0].iter().any(|i| i.id == 6655));
        assert!(parse_itemsets("invalid end point").is_err());
        assert!(parse_itemsets(r#"{"response":{"valid":true}}"#).is_err());
    }

    #[test]
    fn earlysets() {
        let e = parse_earlysets(&fixture("lolalytics-earlyset-ahri-middle.json")).unwrap();
        assert_eq!(e.len(), MAX_EARLY);
        assert_eq!(e[0].items, vec![1056, 3802, 1001, 1026]);
        assert_eq!((e[0].win_rate, e[0].pick_rate, e[0].games), (51.46, 0.62, 103));
        assert!(e.windows(2).all(|w| w[0].games >= w[1].games));
    }

    #[test]
    fn counters_against_another_lane() {
        let c = parse_counters(&fixture("lolalytics-counter-braum-support-vs-bottom.json")).unwrap();
        let ezreal = c.counters.iter().find(|m| m.champ == 81).unwrap();
        assert_eq!(ezreal.default_lane, Some(Lane::Bottom));
        assert!(ezreal.games > 1000, "{ezreal:?}");
        assert!(c.counters.len() > 30);
    }

    #[test]
    fn matchup_page() {
        let m = parse_matchup_page(&fixture("lolalytics-vs-braum-ezreal.html"), 81).unwrap();
        assert_eq!((m.vs, m.games, m.win_rate), (81, 1807, 57.78));
        assert_eq!((m.start.as_slice(), m.start_games, m.start_wr), ([3865, 2003, 2003].as_slice(), 1776, 57.83));
        assert_eq!(m.core, [3876, 3047, 3190]);
        assert_eq!(m.runes, [8465, 8463, 8473, 8242, 8347, 8345]);
        let p = m.rune_page.unwrap();
        assert_eq!((p.shards.as_slice(), p.win_rate, p.games), ([5005, 5001, 5001].as_slice(), 57.95, 1648));
        assert_eq!((m.summoners.as_slice(), m.skills.as_str()), ([4, 7].as_slice(), "QEW"));
        assert_eq!((m.skill_order.as_str(), m.skill_order_games), ("QEWQQRQEQEREEWW", 301));
    }

    #[test]
    fn counters() {
        let c = parse_counters(&fixture("lolalytics-counter-ahri-middle.json")).unwrap();
        assert_eq!((c.win_rate, c.pick_rate, c.lane), (52.36, 9.8, Some(Lane::Middle)));
        assert_eq!(c.lanes[&Lane::Middle], 94.9);
        assert_eq!(c.lanes.len(), 5);
        let diana = c.counters.iter().find(|m| m.champ == 131).unwrap();
        assert_eq!((diana.vs_wr, diana.games, diana.default_lane), (59.04, 271, Some(Lane::Middle)));
        assert!(c.counters.len() > 30);
    }

    #[test]
    fn build_page_rift() {
        let p = parse_build_page(&fixture("lolalytics-build-ahri-middle.html")).unwrap();
        assert_eq!(p.games, Some(16639));
        assert_eq!((p.win_rate, p.pick_rate), (Some(52.36), Some(9.8)));
        let b = &p.benchmarks;
        // 204.82 minions + 1.25 jungle CS, 12142 gold, 25428 damage over a ~28.4 min average.
        let cs = b.cs_per_min.unwrap();
        assert!((6.8..7.6).contains(&cs), "cs/min {cs}");
        let gold = b.gold_per_min.unwrap();
        assert!((400.0..450.0).contains(&gold), "gold/min {gold}");
        assert!((800.0..950.0).contains(&b.damage_per_min.unwrap()));
        assert_eq!(b.vision_per_min, None);
    }

    #[test]
    fn build_page_aram() {
        let p = parse_build_page(&fixture("lolalytics-build-ahri-aram.html")).unwrap();
        assert_eq!((p.games, p.win_rate), (Some(503), Some(56.06)));
        assert!(p.benchmarks.gold_per_min.unwrap() > 500.0, "ARAM games are short");
    }

    #[test]
    fn build_page_runes() {
        let p = parse_build_page(&fixture("lolalytics-build-thresh-support.html")).unwrap();
        assert_eq!(p.games, Some(62757));
        let r = p.runes.unwrap();
        // Aftershock / Font of Life / Bone Plating / Unflinching + Cosmic Insight, Biscuits.
        assert_eq!(r.popular.primary, vec![8439, 8463, 8473, 8242]);
        assert_eq!(r.popular.secondary, vec![8347, 8345]);
        assert_eq!(r.popular.shards, vec![5008, 5008, 5001]);
        assert_eq!((r.popular.win_rate, r.popular.games), (52.88, 34569));
        assert_eq!(r.best.primary[0], 8465, "Guardian wins most");
        let unflinching = &r.stats[&8242];
        assert_eq!((unflinching[0].games, unflinching[1].games), (43786, 11828));
        assert_eq!(r.stats[&5013].len(), 1, "shards have one row");
    }

    #[test]
    fn build_page_skill_order() {
        let p = parse_build_page(&fixture("lolalytics-build-thresh-support.html")).unwrap();
        assert_eq!((p.skill_order.as_str(), p.skill_order_games, p.skill_priority.as_str()), ("QEWQQRQEQEREEWW", 7509, "QEW"));
        let row = |id: Value| skill_order(&serde_json::json!({ "skillorder": { "id": id, "n": 50 } }));
        assert_eq!(row(312114.into()), ("EQWQQR".into(), 50));
        assert_eq!(row("1321".into()), (String::new(), 0), "shorter than 6 levels");
        assert_eq!(row(1321514.into()), (String::new(), 0), "digit outside 1-4");
        assert_eq!(row(Value::Null), (String::new(), 0));
    }

    #[test]
    fn build_page_garbage_is_empty() {
        assert!(parse_build_page("<html>nope</html>").is_err());
        let bad = r#"<script type="qwik/json">{"objs":[{"stats":"1","count":"2"},"x",3]}</script>"#;
        assert_eq!(parse_build_page(bad).unwrap(), PageStats::default());
    }

    #[test]
    fn tier_list() {
        let t = parse_tier_list(&fixture("lolalytics-list-middle.json"), Lane::Middle).unwrap();
        assert_eq!(t.len(), 94, "unranked rows dropped");
        assert_eq!(t[0].tier.as_deref(), Some("S+"));
        let ahri = t.iter().find(|e| e.champ == 103).unwrap();
        assert_eq!((ahri.tier.as_deref(), ahri.games, ahri.win_rate), (Some("S"), 16202, 52.05));
        assert!(t.iter().all(|e| e.lane == Lane::Middle));
    }

    #[test]
    fn urls() {
        let q = Query {
            slug: "ahri",
            lane: Some(Lane::Middle),
            mode: Mode::Rift,
            tier: "emerald_plus",
            region: "all",
            patch: "16.21".into(),
        };
        assert_eq!(
            q.mega("counter"),
            "https://a1.lolalytics.com/mega/?ep=counter&v=1&patch=16.21&c=ahri&lane=middle&tier=emerald_plus&queue=ranked&region=all"
        );
        let a = Query { lane: None, mode: Mode::Mayhem, ..q };
        assert!(a.mega("build-itemset").contains("&c=ahri&tier=emerald_plus&queue=aram&"));
        assert_eq!(a.page(), "https://lolalytics.com/lol/ahri/aram/build/?tier=emerald_plus&patch=16.21&region=all");
    }

    fn ahri() -> Champion {
        Champion {
            key: 103,
            id: "Ahri".into(),
            name: "Ahri".into(),
            slug: "ahri".into(),
            classes: vec![],
            traits: Default::default(),
            base: Default::default(),
        }
    }

    fn live_http() -> crate::Http {
        crate::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap()
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_champ_stats() {
        let http = live_http();
        for mode in [Mode::Rift, Mode::Aram] {
            let s =
                champ_stats(&http, &ahri(), Some(Lane::Middle), mode, "emerald_plus", "all", "16.21").await.unwrap();
            assert!(s.games >= MIN_GAMES || s.patch == "30", "{mode:?}: {} games on {}", s.games, s.patch);
            assert!(!s.slots.is_empty() && !s.boots.is_empty() && !s.early.is_empty());
            assert!(s.benchmarks.cs_per_min.is_some(), "{mode:?} benchmarks");
            assert_eq!(s.counters.is_empty(), mode != Mode::Rift);
        }
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_tier_list() {
        let t = super::tier_list(&live_http(), Lane::Support, "emerald_plus", "all", "30").await.unwrap();
        assert!(t.len() > 30 && t[0].tier.is_some());
    }
}
