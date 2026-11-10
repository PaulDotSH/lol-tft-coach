//! lolalytics data for the draft coach: teammate synergy (`build-team`) and a
//! light per-lane matchup fetch (`counter` only, no item sets or build page).
//!
//! Findings (verified 2026-10-22, Ahri mid / Jinx bot, emerald_plus 16.21):
//! - `build-team` answers `team_h` = ["id","wr","d1","d2","pr","n"] and `team`
//!   = rows per teammate lane (the champion's own lane is absent), sorted by
//!   pick rate. `wr` is the win rate together over `n` games; `d1` is `wr`
//!   minus the expectation from both champions' own win rates
//!   (`wr_a + wr_b − 50`), i.e. the synergy itself. Unknown champions answer
//!   `{"status":404}`.
//! - Each lane's `n` sums to the champion's games in its lane (~20k for Ahri).

use crate::lolalytics::{MIN_GAMES, parse_counters};
use crate::num;
use model_static::{ChampStats, Champion, Lane, Mode, TeamSynergy, Teammate};
use serde_json::Value;
use std::time::Duration;

const MEGA: &str = "https://a1.lolalytics.com/mega/";
const TTL: Duration = Duration::from_secs(12 * 3600);
const FALLBACK_PATCH: &str = "30";

fn url(ep: &str, slug: &str, lane: Option<Lane>, tier: &str, region: &str, patch: &str) -> String {
    let lane = lane.map(|l| format!("&lane={}", l.as_str())).unwrap_or_default();
    format!("{MEGA}?ep={ep}&v=1&patch={patch}&c={slug}{lane}&tier={tier}&queue=ranked&region={region}")
}

/// Teammate synergy for `champ` in `lane` (Rift ranked), with the 30-day
/// fallback when the current patch sample is small.
pub async fn team(
    http: &crate::Http,
    champ: &Champion,
    lane: Lane,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<TeamSynergy> {
    let get = |p: &str| {
        let u = url("build-team", &champ.slug, Some(lane), tier, region, p);
        let p = p.to_owned();
        async move { http.get_parsed(&u, TTL, |s| parse_team(s, champ.key, lane, &p)).await }
    };
    let t = get(patch).await?;
    if (t.games as u64) < MIN_GAMES && patch != FALLBACK_PATCH {
        match get(FALLBACK_PATCH).await {
            Ok(alt) if alt.games > t.games => return Ok(alt),
            Ok(_) => {}
            Err(e) => tracing::warn!("lolalytics team 30-day fallback for {}: {e:#}", champ.slug),
        }
    }
    Ok(t)
}

/// Win rate, lane shares and matchups for `champ` in `lane` from `ep=counter`
/// alone. Item sets, early sets and benchmarks are left empty. The URL equals
/// the one [`crate::lolalytics::champ_stats`] uses, so they share the disk cache.
pub async fn lane_stats(
    http: &crate::Http,
    champ: &Champion,
    lane: Option<Lane>,
    tier: &str,
    region: &str,
    patch: &str,
) -> anyhow::Result<ChampStats> {
    let get = |p: &str| {
        let u = url("counter", &champ.slug, lane, tier, region, p);
        async move { http.get_parsed(&u, TTL, parse_counters).await }
    };
    let games = |c: &crate::lolalytics::Counters| c.counters.iter().map(|m| m.games as u64).sum::<u64>();
    let mut used = patch.to_owned();
    let mut c = get(patch).await?;
    if games(&c) < MIN_GAMES && patch != FALLBACK_PATCH {
        match get(FALLBACK_PATCH).await {
            Ok(alt) if games(&alt) > games(&c) => (c, used) = (alt, FALLBACK_PATCH.to_owned()),
            Ok(_) => {}
            Err(e) => tracing::warn!("lolalytics counter 30-day fallback for {}: {e:#}", champ.slug),
        }
    }
    Ok(ChampStats {
        champ: champ.key,
        lane: lane.or(c.lane),
        mode: Mode::Rift,
        patch: used,
        win_rate: c.win_rate,
        pick_rate: c.pick_rate,
        games: games(&c),
        lanes: c.lanes,
        slots: vec![],
        boots: vec![],
        early: vec![],
        counters: c.counters,
        benchmarks: Default::default(),
        runes: None,
        coachless_runes: None,
        coachless: None,
        summoners: vec![],
        skill_order: String::new(),
        skill_order_games: 0,
        skill_priority: String::new(),
        by_length: vec![],
    })
}

/// Pure parser for `ep=build-team`; columns are located by `team_h`.
pub fn parse_team(json: &str, champ: model_static::ChampKey, lane: Lane, patch: &str) -> anyhow::Result<TeamSynergy> {
    let root: Value = serde_json::from_str(json)?;
    let head: Vec<&str> = root
        .get("team_h")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("no team_h"))?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let col = |name: &str| head.iter().position(|h| *h == name).ok_or_else(|| anyhow::anyhow!("team_h lacks {name}"));
    let (id, wr, d1, n) = (col("id")?, col("wr")?, col("d1")?, col("n")?);
    let lanes = root.get("team").and_then(Value::as_object).ok_or_else(|| anyhow::anyhow!("no team"))?;
    let mut mates = Vec::new();
    let mut games = 0;
    for (l, rows) in lanes {
        let Some(mate_lane) = Lane::from_riot(l) else { continue };
        let rows = rows.as_array().map_or(&[][..], Vec::as_slice);
        let lane_games: u32 = rows.iter().filter_map(|r| num(&r[n])).map(|v| v as u32).sum();
        games = games.max(lane_games);
        mates.extend(rows.iter().filter_map(|r| {
            Some(Teammate {
                champ: r[id].as_u64()? as u32,
                lane: mate_lane,
                win_rate: num(&r[wr])? as f32,
                delta: num(&r[d1])? as f32,
                games: num(&r[n])? as u32,
            })
        }));
    }
    Ok(TeamSynergy { champ, lane, patch: patch.to_owned(), games, mates })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../fixtures/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn team_ahri_middle() {
        let t = parse_team(&fixture("lolalytics-team-ahri-middle.json"), 103, Lane::Middle, "16.21").unwrap();
        assert_eq!((t.champ, t.lane, t.games), (103, Lane::Middle, 20284));
        assert!(t.mates.iter().all(|m| m.lane != Lane::Middle), "own lane absent");
        let shen = t.mates.iter().find(|m| m.champ == 98 && m.lane == Lane::Top).unwrap();
        assert_eq!((shen.win_rate, shen.delta, shen.games), (58.15, 5.45, 399));
        let thresh = t.mates.iter().find(|m| m.champ == 412 && m.lane == Lane::Support).unwrap();
        assert_eq!((thresh.win_rate, thresh.delta, thresh.games), (53.3, 0.6, 1653));
        for l in [Lane::Top, Lane::Jungle, Lane::Bottom, Lane::Support] {
            let n: u32 = t.mates.iter().filter(|m| m.lane == l).map(|m| m.games).sum();
            assert_eq!(n, 20284, "{l:?}");
        }
    }

    #[test]
    fn team_jinx_bottom_supports() {
        let t = parse_team(&fixture("lolalytics-team-jinx-bottom.json"), 222, Lane::Bottom, "16.21").unwrap();
        let sup: Vec<_> = t.mates.iter().filter(|m| m.lane == Lane::Support).collect();
        assert!(sup.len() > 100);
        assert_eq!((sup[0].champ, sup[0].delta, sup[0].games), (117, 1.77, 4248), "Lulu most played");
        assert!(t.mates.iter().any(|m| m.lane == Lane::Middle));
    }

    #[test]
    fn team_garbage() {
        assert!(parse_team(r#"{"status":404}"#, 1, Lane::Top, "16.21").is_err());
        assert!(parse_team("nope", 1, Lane::Top, "16.21").is_err());
        let odd = r#"{"team_h":["n","id","d1","wr"],"team":{"support":[[100,412,1.5,53.0]],"utility2":[[1,2,3,4]]}}"#;
        let t = parse_team(odd, 222, Lane::Bottom, "30").unwrap();
        assert_eq!(t.mates, vec![Teammate { champ: 412, lane: Lane::Support, win_rate: 53.0, delta: 1.5, games: 100 }]);
        assert_eq!((t.games, t.patch.as_str()), (100, "30"));
    }

    #[test]
    fn urls_match_champ_stats() {
        assert_eq!(
            url("counter", "ahri", Some(Lane::Middle), "emerald_plus", "all", "16.21"),
            "https://a1.lolalytics.com/mega/?ep=counter&v=1&patch=16.21&c=ahri&lane=middle&tier=emerald_plus&queue=ranked&region=all"
        );
    }

    #[tokio::test]
    #[ignore = "live network"]
    async fn live_team() {
        let http = crate::Http::new(std::env::temp_dir().join("lol-coach-live-cache")).unwrap();
        let jinx = Champion {
            key: 222,
            id: "Jinx".into(),
            name: "Jinx".into(),
            slug: "jinx".into(),
            classes: vec![],
            traits: Default::default(),
            base: Default::default(),
        };
        let t = team(&http, &jinx, Lane::Bottom, "emerald_plus", "all", "16.21").await.unwrap();
        assert!(t.games as u64 >= MIN_GAMES && t.mates.iter().any(|m| m.lane == Lane::Support));
        let s = lane_stats(&http, &jinx, Some(Lane::Bottom), "emerald_plus", "all", "16.21").await.unwrap();
        assert!(s.counters.len() > 20 && s.lanes.contains_key(&Lane::Bottom));
    }
}
