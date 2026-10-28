//! Coachless (coachless.gg, by xPetu): win probability added (WPA) per item, rune and
//! summoner spell for one champion and role, optionally against the lane opponent and after
//! given first items. Needs a Coachless account: the app catches the token pair from the
//! site's own login reply (`LOGIN_PATH`) and renews it with the refresh token.
//!
//! Findings (2026-10-28, from github.com/controlado/lol-autobuild and probing):
//! - `GET /api/ChampionWinprob/GetPatches` is public, oldest first. A free account gets the
//!   newest patch but one; Premium (`isSubscribed: "1"` in the access JWT) the newest.
//! - Everything else is a POST with `commonFilters` (patch, championIds,
//!   matchupChampionIds, leagueTiers 3 gold … 7 master, 8 grandmaster, 9 challenger (nothing
//!   below Gold), regions, role 0 top … 4 support)
//!   and answers `[]` without a valid token. Items need `itemSlots`, boots too (`[]` gets
//!   nothing); a support's legendary slot 1 is the support item.
//! - Rows are `{itemId | rune | summonerSpell | tree, wpaOverall, occurrence}`. WPA is in
//!   win-rate points (winrateObserved - winrateExpected), added to the champion's win rate.

use model_static::{ChampKey, ChampStats, ItemId, ItemStat, Lane, RuneData, RuneId, RunePage, RuneStat};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str = "https://api.coachless.gg";
pub const LOGIN_URL: &str = "https://coachless.gg/login-area/login";
/// The site's login request; its reply holds the token pair.
pub const LOGIN_PATH: &str = "/api/Auth/login";
const TTL: Duration = Duration::from_secs(12 * 3600);
const PATCH_TTL: Duration = Duration::from_secs(3600);
/// A row needs this share of its list's games to be chosen on WPA (fewer is noise).
const MIN_SHARE: f64 = 0.05;
/// Summoner spells: the share to be considered, and Flash is kept above FLASH_KEEP.
const SPELL_SHARE: f64 = 0.2;
const FLASH: u32 = 4;
const FLASH_KEEP: f64 = 0.5;
/// Fewer first-item games than this against the lane opponent: drop the matchup filter.
const MIN_MATCHUP_GAMES: f64 = 1000.0;
/// Fewer games than this in a slot: keep lolalytics' slot.
const MIN_SLOT_GAMES: f64 = 200.0;

// `itemType`.
const LEGENDARY: u8 = 1;
const BOOTS: u8 = 2;
const SUPPORT: u8 = 3;

// ---- sign-in ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// The token pair in a login or refresh reply.
pub fn parse_tokens(body: &str) -> Option<Tokens> {
    let t: Tokens = serde_json::from_str(body).ok()?;
    (!t.access_token.is_empty() && !t.refresh_token.is_empty()).then_some(t)
}

/// Expiry (unix seconds) and Premium flag from the access JWT's payload.
fn claims(jwt: &str) -> Option<(u64, bool)> {
    let v: Value = serde_json::from_slice(&b64url(jwt.split('.').nth(1)?)?).ok()?;
    let sub = &v["isSubscribed"];
    Some((v["exp"].as_u64()?, sub.as_str() == Some("1") || sub.as_bool() == Some(true) || sub.as_u64() == Some(1)))
}

fn b64url(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0);
    for c in s.bytes().take_while(|&c| c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        (acc, bits) = ((acc << 6) | v as u32, bits + 6);
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The signed-in account, kept in `path` (the token pair as JSON) across runs.
pub struct Auth {
    path: PathBuf,
    tokens: Mutex<Option<Tokens>>,
    /// One refresh at a time: a refresh token works once.
    renew: tokio::sync::Mutex<()>,
}

impl Auth {
    pub fn load(path: PathBuf) -> Auth {
        let tokens = std::fs::read_to_string(&path).ok().and_then(|s| parse_tokens(&s));
        Auth { path, tokens: Mutex::new(tokens), renew: Default::default() }
    }

    pub fn signed_in(&self) -> bool {
        self.tokens.lock().unwrap().is_some()
    }

    pub fn premium(&self) -> bool {
        self.tokens.lock().unwrap().as_ref().and_then(|t| claims(&t.access_token)).is_some_and(|c| c.1)
    }

    /// Sign in (Some) or out (None).
    pub fn set(&self, t: Option<Tokens>) {
        let saved = match &t {
            Some(t) => serde_json::to_string(t).map_err(std::io::Error::other).and_then(|s| std::fs::write(&self.path, s)),
            None => std::fs::remove_file(&self.path).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }),
        };
        if let Err(e) = saved {
            tracing::warn!("coachless tokens {:?}: {e}", self.path);
        }
        *self.tokens.lock().unwrap() = t;
    }

    /// A valid access token, renewed first when it expires within a minute. A refused
    /// refresh token signs out; a network error keeps it for the next try.
    pub async fn access(&self, http: &crate::Http) -> anyhow::Result<String> {
        let _one = self.renew.lock().await;
        let Some(t) = self.tokens.lock().unwrap().clone() else { anyhow::bail!("not signed in to Coachless") };
        if claims(&t.access_token).is_some_and(|(exp, _)| exp > unix_now() + 60) {
            return Ok(t.access_token);
        }
        let resp = http.client.post(format!("{API}/api/Auth/refresh")).json(&json!({ "refreshToken": t.refresh_token })).send().await?;
        let status = resp.status();
        let fresh = if status.is_success() { parse_tokens(&resp.text().await?) } else { None };
        match fresh {
            Some(n) => {
                self.set(Some(n.clone()));
                Ok(n.access_token)
            }
            None if status.is_client_error() || status.is_success() => {
                self.set(None);
                anyhow::bail!("Coachless refused the refresh token ({status}); sign in again")
            }
            None => anyhow::bail!("Coachless refresh: {status}"),
        }
    }
}

// ---- requests ----

/// Who and what to ask about. `first` / `second`: my first two completed items, in order.
#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
    pub champ: ChampKey,
    pub lane: Lane,
    pub vs: Option<ChampKey>,
    /// lolalytics tier setting ("emerald_plus"); ranks below Gold ask for Gold.
    pub tier: &'a str,
    /// lolalytics region setting ("all", "euw").
    pub region: &'a str,
    pub first: Option<ItemId>,
    pub second: Option<ItemId>,
}

#[derive(Debug, Deserialize)]
struct Row {
    #[serde(alias = "itemId", alias = "rune", alias = "summonerSpell", alias = "tree")]
    id: u32,
    #[serde(rename = "wpaOverall", default)]
    wpa: f32,
    occurrence: f64,
}

/// An empty list is refused, so it never sits in the cache (it is also what a lapsed token gets).
fn parse_rows(json: &str) -> anyhow::Result<Vec<Row>> {
    let rows: Vec<Row> = serde_json::from_str(json)?;
    anyhow::ensure!(!rows.is_empty(), "no rows");
    Ok(rows)
}

fn parse_obj(json: &str, keys: &[&str]) -> anyhow::Result<Vec<Vec<Row>>> {
    let mut v: HashMap<String, Value> = serde_json::from_str(json)?;
    let out: Vec<Vec<Row>> =
        keys.iter().map(|k| v.remove(*k).map_or(Ok(vec![]), serde_json::from_value)).collect::<Result<_, _>>()?;
    anyhow::ensure!(out.iter().any(|r| !r.is_empty()), "no rows");
    Ok(out)
}

fn games(rows: &[Row]) -> f64 {
    rows.iter().map(|r| r.occurrence).sum()
}

/// Best WPA among rows with at least `share` of the games; the most played when none has.
fn pick(rows: &[Row], share: f64) -> Option<&Row> {
    let min = share * games(rows);
    rows.iter().filter(|r| r.occurrence >= min).max_by(|a, b| a.wpa.total_cmp(&b.wpa)).or_else(|| most_played(rows))
}

fn most_played(rows: &[Row]) -> Option<&Row> {
    rows.iter().max_by(|a, b| a.occurrence.total_cmp(&b.occurrence))
}

fn role(l: Lane) -> u8 {
    match l {
        Lane::Top => 0,
        Lane::Jungle => 1,
        Lane::Middle => 2,
        Lane::Bottom => 3,
        Lane::Support => 4,
    }
}

fn tiers(t: &str) -> Vec<u8> {
    let rank = |s: &str| match s {
        "platinum" => 4,
        "emerald" => 5,
        "diamond" => 6,
        "master" => 7,
        "grandmaster" => 8,
        "challenger" => 9,
        _ => 3,
    };
    match t.strip_suffix("_plus") {
        Some(base) => (rank(base)..=9).collect(),
        None if t == "all" => (3..=9).collect(),
        None => vec![rank(t)],
    }
}

fn region(r: &str) -> Option<u8> {
    let i = ["br", "eune", "euw", "jp", "kr", "lan", "las", "me", "na", "oce", "", "ru", "sg", "", "tr", "tw", "vn"]
        .iter()
        .position(|&x| x == r && !x.is_empty())?;
    Some(i as u8)
}

/// The patch to ask for: the newest for Premium, the one before for a free account.
async fn patch(http: &crate::Http, premium: bool) -> anyhow::Result<(String, Value)> {
    #[derive(Deserialize)]
    struct P {
        label: String,
        major: u32,
        patch: u32,
    }
    let url = format!("{API}/api/ChampionWinprob/GetPatches");
    let all: Vec<P> = http.get_parsed(&url, PATCH_TTL, |s| Ok(serde_json::from_str(s)?)).await?;
    let i = if premium { all.len().checked_sub(1) } else { all.len().checked_sub(2).or(all.len().checked_sub(1)) };
    let p = &all[i.ok_or_else(|| anyhow::anyhow!("Coachless has no patches"))?];
    Ok((p.label.clone(), json!({ "major": p.major, "patch": p.patch, "patchAdditions": 0 })))
}

fn filters(q: &Query, patch: &Value) -> Value {
    let mut f = json!({
        "patch": patch,
        "championIds": [q.champ],
        "leagueTiers": tiers(q.tier),
        "role": role(q.lane),
    });
    if let Some(v) = q.vs {
        f["matchupChampionIds"] = json!([v]);
    }
    if let Some(r) = region(q.region) {
        f["regions"] = json!([r]);
    }
    f
}

struct Api<'a> {
    http: &'a crate::Http,
    token: &'a str,
}

impl Api<'_> {
    async fn post<T>(&self, path: &str, body: Value, parse: impl Fn(&str) -> anyhow::Result<T>) -> anyhow::Result<T> {
        self.http.post_parsed(&format!("{API}/api/{path}"), &body, self.token, TTL, parse).await
    }

    async fn items(&self, f: &Value, kind: u8, slots: &[u8], first: Option<ItemId>, second: Option<ItemId>) -> anyhow::Result<Vec<Row>> {
        let body = json!({
            "commonFilters": f,
            "itemSlots": slots,
            "itemType": kind,
            "keystone": null,
            "starterId": null,
            "firstPurchaseId": null,
            "firstLegendaryId": first,
            "secondLegendaryId": first.and(second),
            "loadFirstEpicPurchase": false,
            "includeSupportItems": kind == SUPPORT,
        });
        self.post("ChampionWinprob/GetGlobalItemStatistics", body, parse_rows).await
    }

    async fn runes(&self, f: &Value, base_wr: f32) -> anyhow::Result<RuneData> {
        let ks = self.post("Rune/GetKeystoneData", json!({ "commonFilters": f }), parse_rows).await?;
        let key = pick(&ks, MIN_SHARE).ok_or_else(|| anyhow::anyhow!("no keystone"))?;
        let tree = tree_of(key.id);
        let body = json!({ "commonFilters": f, "tree": tree, "keystone": key.id });
        let trees = self.post("Rune/GetSecondaryTreePlaycount", body, parse_rows).await?;
        let sec = trees.iter().filter(|r| r.id != tree).max_by(|a, b| a.occurrence.total_cmp(&b.occurrence));
        let sec = sec.ok_or_else(|| anyhow::anyhow!("no secondary tree"))?.id;
        let rows = ["rowOnes", "rowTwos", "rowThrees"];
        let tree_rows = |load: RuneId| {
            let body = json!({ "commonFilters": f, "keystone": key.id, "mainTree": tree, "treeToLoad": load });
            self.post("Rune/GetRunesForKeystoneAndTree", body, move |s| parse_obj(s, &rows))
        };
        let shards = self.post("Rune/GetShardsForKeystoneAndTree", json!({ "commonFilters": f, "keystone": key.id }), |s| {
            parse_obj(s, &["offense", "flex", "defense"])
        });
        let (primary, secondary, shards) = tokio::try_join!(tree_rows(tree), tree_rows(sec), shards)?;
        Ok(rune_data(key, &primary, &secondary, &shards, base_wr))
    }

    async fn spells(&self, f: &Value) -> anyhow::Result<Vec<u32>> {
        let rows = self.post("ChampionWinprob/GetGlobalSummonerSpellStatistics", json!({ "commonFilters": f, "pairedSpell": null }), parse_rows).await?;
        Ok(spell_pair(&rows))
    }
}

/// The rune tree a keystone sits in (Hail of Blades has a 99xx id).
fn tree_of(keystone: RuneId) -> RuneId {
    if keystone == 9923 { 8100 } else { keystone / 100 * 100 }
}

fn rune_stat(r: &Row, rows: &[Row], base_wr: f32) -> RuneStat {
    RuneStat { pick_rate: (100.0 * r.occurrence / games(rows).max(1.0)) as f32, win_rate: base_wr + r.wpa, games: r.occurrence as u32 }
}

/// One page (keystone, the best rune of each primary row, the two best secondary rows,
/// the best shards) as both `popular` and `best`, so the coach starts from it.
fn rune_data(key: &Row, primary: &[Vec<Row>], secondary: &[Vec<Row>], shards: &[Vec<Row>], base_wr: f32) -> RuneData {
    let mut page = RunePage { primary: vec![key.id], secondary: vec![], shards: vec![], win_rate: base_wr + key.wpa, games: key.occurrence as u32 };
    page.primary.extend(primary.iter().filter_map(|r| pick(r, MIN_SHARE)).map(|r| r.id));
    let mut sec: Vec<(usize, &Row)> = secondary.iter().enumerate().filter_map(|(i, r)| Some((i, pick(r, MIN_SHARE)?))).collect();
    sec.sort_by(|a, b| b.1.wpa.total_cmp(&a.1.wpa));
    sec.truncate(2);
    sec.sort_by_key(|s| s.0);
    page.secondary = sec.iter().map(|s| s.1.id).collect();
    page.shards = shards.iter().filter_map(|r| pick(r, MIN_SHARE)).map(|r| r.id).collect();

    // Per rune: as a primary pick, then as a secondary; shards one entry (the defense row last,
    // as the coach only swaps that row).
    let (mut prim, mut second) = (HashMap::new(), HashMap::new());
    for rows in primary {
        for r in rows {
            prim.insert(r.id, rune_stat(r, rows, base_wr));
        }
    }
    for rows in secondary {
        for r in rows {
            second.insert(r.id, rune_stat(r, rows, base_wr));
        }
    }
    let mut stats: HashMap<RuneId, Vec<RuneStat>> = HashMap::new();
    for id in prim.keys().chain(second.keys()) {
        let (p, s) = (prim.get(id), second.get(id));
        if let (Some(a), Some(b)) = (p.or(s), s.or(p)) {
            stats.insert(*id, vec![a.clone(), b.clone()]);
        }
    }
    for rows in shards {
        for r in rows {
            stats.insert(r.id, vec![rune_stat(r, rows, base_wr)]);
        }
    }
    RuneData { popular: page.clone(), best: page, stats }
}

/// Two summoner spells: Flash when most games take it, then the best WPA among common ones.
fn spell_pair(rows: &[Row]) -> Vec<u32> {
    // Every game counts twice, once per spell.
    let total = (games(rows) / 2.0).max(1.0);
    let common = |r: &&Row| r.occurrence >= SPELL_SHARE * total;
    let best = |skip: Option<u32>| {
        rows.iter().filter(common).filter(|r| Some(r.id) != skip).max_by(|a, b| a.wpa.total_cmp(&b.wpa)).map(|r| r.id)
    };
    let flash = rows.iter().find(|r| r.id == FLASH).is_some_and(|r| r.occurrence >= FLASH_KEEP * total);
    let first = if flash { Some(FLASH) } else { best(None) };
    first.into_iter().chain(best(first)).collect()
}

fn item_stats(rows: &[Row], base_wr: f32) -> Vec<ItemStat> {
    let mut v: Vec<ItemStat> = rows
        .iter()
        .map(|r| {
            let wr = ((base_wr + r.wpa) / 100.0).clamp(0.0, 1.0) as f64;
            ItemStat { id: r.id, games: r.occurrence as u32, wins: (r.occurrence * wr).round() as u32 }
        })
        .filter(|s| s.games > 0)
        .collect();
    v.sort_unstable_by(|a, b| b.games.cmp(&a.games).then(a.id.cmp(&b.id)));
    v
}

/// `base` (lolalytics) with its item slots, boots, runes and summoners from Coachless.
/// WPA becomes a win rate on top of `base.win_rate`, so the coach scores it as before.
/// A slot Coachless has too few games for keeps lolalytics' rows.
pub async fn champ_stats(http: &crate::Http, auth: &Auth, q: Query<'_>, base: &ChampStats) -> anyhow::Result<ChampStats> {
    let token = auth.access(http).await?;
    let api = Api { http, token: &token };
    let (label, patch) = patch(http, auth.premium()).await?;
    let support = q.lane == Lane::Support;
    // The quest item holds a support's first slot: no first-item conditions.
    let (first, second) = if support { (None, None) } else { (q.first, q.second) };
    let first_kind = if support { SUPPORT } else { LEGENDARY };

    let mut f = filters(&q, &patch);
    let mut slot1 = api.items(&f, first_kind, &[1], None, None).await;
    if q.vs.is_some() && slot1.as_ref().map_or(true, |r| games(r) < MIN_MATCHUP_GAMES) {
        f = filters(&Query { vs: None, ..q }, &patch);
        slot1 = api.items(&f, first_kind, &[1], None, None).await;
    }
    let slot1 = slot1?;
    let (s2, s3, s4, boots, runes, spells) = tokio::join!(
        api.items(&f, LEGENDARY, &[2], first, None),
        api.items(&f, LEGENDARY, &[3], first, second),
        api.items(&f, LEGENDARY, &[4, 5, 6], first, second),
        api.items(&f, BOOTS, &[1], None, None),
        api.runes(&f, base.win_rate),
        api.spells(&f),
    );

    let mut out = base.clone();
    let wr = base.win_rate;
    let slots = [Ok(slot1), s2, s3, s4];
    for (i, rows) in slots.iter().enumerate() {
        match rows {
            Ok(rows) if games(rows) >= MIN_SLOT_GAMES => {
                let stats = item_stats(rows, wr);
                // Items 4–6 are one list: it fills slots 4 and 5.
                for j in if i == 3 { 3..5 } else { i..i + 1 } {
                    if j < out.slots.len() {
                        out.slots[j] = stats.clone();
                    } else if j == out.slots.len() {
                        out.slots.push(stats.clone());
                    }
                }
            }
            Ok(_) => tracing::debug!("coachless slot {}: too few games", i + 1),
            Err(e) => tracing::warn!("coachless slot {}: {e:#}", i + 1),
        }
    }
    match boots {
        Ok(b) if games(&b) >= MIN_SLOT_GAMES => out.boots = item_stats(&b, wr),
        Ok(_) => {}
        Err(e) => tracing::warn!("coachless boots: {e:#}"),
    }
    match runes {
        Ok(r) => out.runes = Some(r),
        Err(e) => tracing::warn!("coachless runes: {e:#}"),
    }
    match spells {
        Ok(s) => out.summoners = s,
        Err(e) => tracing::warn!("coachless spells: {e:#}"),
    }
    out.coachless = Some(label);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(json: &str) -> Vec<Row> {
        parse_rows(json).unwrap()
    }

    #[test]
    fn tokens_come_from_the_login_reply() {
        let t = parse_tokens(r#"{"actionResult":0,"accessToken":"a.b.c","refreshToken":"r"}"#).unwrap();
        assert_eq!((t.access_token.as_str(), t.refresh_token.as_str()), ("a.b.c", "r"));
        assert!(parse_tokens(r#"{"actionResult":1,"accessToken":"","refreshToken":""}"#).is_none());
        // The saved file round-trips.
        assert_eq!(parse_tokens(&serde_json::to_string(&t).unwrap()), Some(t));
    }

    #[test]
    fn jwt_claims() {
        // {"exp":1800000000,"isSubscribed":"1"}
        let jwt = "x.eyJleHAiOjE4MDAwMDAwMDAsImlzU3Vic2NyaWJlZCI6IjEifQ.sig";
        assert_eq!(claims(jwt), Some((1_800_000_000, true)));
        // {"exp":1,"isSubscribed":"0"}, unpadded
        assert_eq!(claims("x.eyJleHAiOjEsImlzU3Vic2NyaWJlZCI6IjAifQ.s"), Some((1, false)));
    }

    #[test]
    fn filters_map_settings() {
        assert_eq!(tiers("emerald_plus"), vec![5, 6, 7, 8, 9]);
        assert_eq!(tiers("all"), vec![3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(tiers("master_plus"), vec![7, 8, 9]);
        assert_eq!(tiers("diamond"), vec![6]);
        assert_eq!(tiers("silver"), vec![3]);
        assert_eq!((region("euw"), region("vn"), region("all")), (Some(2), Some(16), None));
        let q = Query { champ: 111, lane: Lane::Support, vs: Some(412), tier: "emerald_plus", region: "all", first: None, second: None };
        let f = filters(&q, &json!({"major": 16, "patch": 20, "patchAdditions": 0}));
        assert_eq!(f["role"], 4);
        assert_eq!(f["matchupChampionIds"], json!([412]));
        assert!(f.get("regions").is_none());
    }

    #[test]
    fn empty_lists_are_refused() {
        assert!(parse_rows("[]").is_err());
        assert!(parse_obj(r#"{"rowOnes":[],"rowTwos":[]}"#, &["rowOnes", "rowTwos"]).is_err());
    }

    #[test]
    fn picks_best_wpa_among_common_rows() {
        let r = rows(r#"[{"itemId":1,"wpaOverall":0.5,"occurrence":900},{"itemId":2,"wpaOverall":3.0,"occurrence":20},{"itemId":3,"wpaOverall":1.0,"occurrence":300}]"#);
        // Item 2 has under 5% of the games.
        assert_eq!(pick(&r, MIN_SHARE).unwrap().id, 3);
    }

    #[test]
    fn wpa_becomes_wins_over_the_champion_win_rate() {
        let r = rows(r#"[{"itemId":3078,"wpaOverall":2.0,"occurrence":1000},{"itemId":3071,"wpaOverall":-1.0,"occurrence":2000}]"#);
        let s = item_stats(&r, 50.0);
        assert_eq!((s[0].id, s[0].games, s[0].wins), (3071, 2000, 980));
        assert_eq!((s[1].id, s[1].wins), (3078, 520));
    }

    #[test]
    fn spells_keep_flash_then_best_common() {
        let r = rows(r#"[{"summonerSpell":4,"wpaOverall":-0.2,"occurrence":950},{"summonerSpell":14,"wpaOverall":0.8,"occurrence":600},
            {"summonerSpell":3,"wpaOverall":1.5,"occurrence":350},{"summonerSpell":21,"wpaOverall":4.0,"occurrence":20}]"#);
        assert_eq!(spell_pair(&r), vec![4, 3]);
    }

    #[test]
    fn rune_page_from_rows() {
        let key = Row { id: 8439, wpa: 1.0, occurrence: 5000.0 };
        assert_eq!(tree_of(8439), 8400);
        assert_eq!(tree_of(9923), 8100);
        let row = |a: u32, b: u32| rows(&format!(r#"[{{"rune":{a},"wpaOverall":1.0,"occurrence":800}},{{"rune":{b},"wpaOverall":0.2,"occurrence":200}}]"#));
        let primary = vec![row(8446, 8463), row(8444, 8473), row(8242, 8451)];
        let secondary = vec![row(8304, 8313), rows(r#"[{"rune":8345,"wpaOverall":-1.0,"occurrence":900}]"#), row(8347, 8410)];
        let shards = vec![row(5005, 5008), row(5008, 5007), row(5001, 5011)];
        let d = rune_data(&key, &primary, &secondary, &shards, 50.0);
        assert_eq!(d.popular.primary, vec![8439, 8446, 8444, 8242]);
        assert_eq!(d.popular.secondary, vec![8304, 8347], "the two best rows, in row order");
        assert_eq!(d.popular.shards, vec![5005, 5008, 5001]);
        assert_eq!(d.popular.win_rate, 51.0);
        assert_eq!(d.stats[&8473].len(), 2);
        assert_eq!(d.stats[&5011].len(), 1);
        assert_eq!(d.stats[&8444][0].win_rate, 51.0);
    }
}
