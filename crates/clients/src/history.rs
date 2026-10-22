//! My own match history from the League client, compacted into `PastGame`s and
//! cached on disk: a played game never changes, so details and timelines are
//! fetched once per game.

use crate::lcu::Lcu;
use crate::id;
use anyhow::{Context, Result};
use model::{Lane, Mode, PastGame, TeamObjectives};
use serde_json::Value;
use std::path::Path;

/// Games read from the history list, newest first.
pub const RECENT: u32 = 30;
/// Bump when parsing changes so cached games are parsed again.
const CACHE_VERSION: u32 = 1;

/// Recent games, newest first. Only games missing from `dir` cost requests; a
/// game whose details fail is used from the list entry and retried next sync.
pub async fn sync(lcu: &Lcu, dir: &Path) -> Result<Vec<PastGame>> {
    let path = format!("/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex={RECENT}");
    let list = lcu.get(&path).await?.context("no match history")?;
    tokio::fs::create_dir_all(dir).await?;
    let mut out = Vec::new();
    for entry in list_games(&list).filter(|g| wanted(g)) {
        let Some(gid) = entry["gameId"].as_u64() else { continue };
        let file = dir.join(format!("{gid}.v{CACHE_VERSION}.json"));
        if let Some(g) = tokio::fs::read(&file).await.ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            out.push(g);
            continue;
        }
        let puuid = my_puuid(entry);
        let detail = lcu.get(&format!("/lol-match-history/v1/games/{gid}")).await;
        let timeline = lcu.get(&format!("/lol-match-history/v1/game-timelines/{gid}")).await;
        let tl = timeline.as_ref().ok().and_then(Option::as_ref);
        let game = match &detail {
            Ok(Some(d)) => parse_game(d, puuid, tl),
            _ => None,
        }
        .or_else(|| parse_game(entry, puuid, None));
        let Some(game) = game else { continue };
        // 404s are final; other errors are retried next sync.
        if detail.is_ok() && timeline.is_ok()
            && let Ok(json) = serde_json::to_vec(&game)
        {
            let _ = tokio::fs::write(&file, json).await;
        }
        out.push(game);
    }
    Ok(out)
}

/// Game entries of a `/matches` response.
pub fn list_games(json: &Value) -> impl Iterator<Item = &Value> {
    json.pointer("/games/games").and_then(Value::as_array).into_iter().flatten()
}

/// Rift and Howling Abyss matchmade games; no customs, practice tool, bots, tutorials or Arena.
pub fn wanted(game: &Value) -> bool {
    let q = game["queueId"].as_i64().unwrap_or(0);
    matches!(game["mapId"].as_i64(), Some(11 | 12)) && q > 0 && !(800..900).contains(&q) && !(2000..=2020).contains(&q)
}

/// The list's own entry carries only my identity.
fn my_puuid(entry: &Value) -> Option<&str> {
    entry["participantIdentities"].as_array()?.first()?.pointer("/player/puuid")?.as_str().filter(|s| !s.is_empty())
}

fn arr(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

/// Lane from a participant's `timeline.lane` / `role`; None for "NONE".
fn lane_of(p: &Value) -> Option<Lane> {
    let t = &p["timeline"];
    match (t["lane"].as_str()?, t["role"].as_str().unwrap_or_default()) {
        ("BOTTOM" | "BOT", "DUO_SUPPORT" | "SUPPORT") => Some(Lane::Support),
        (l, _) => Lane::from_riot(l),
    }
}

fn objectives(t: &Value) -> TeamObjectives {
    let n = |k: &str| t[k].as_u64().unwrap_or(0) as u32;
    TeamObjectives {
        dragons: n("dragonKills"),
        barons: n("baronKills"),
        heralds: n("riftHeraldKills"),
        grubs: n("hordeKills"),
        towers: n("towerKills"),
    }
}

/// Pure parser for one game from `/lol-match-history/v1/games/{id}` (all ten
/// players) or a `/matches` list entry (me only). `puuid` picks me; a lone
/// participant is me. `timeline` is `/lol-match-history/v1/game-timelines/{id}`.
pub fn parse_game(game: &Value, puuid: Option<&str>, timeline: Option<&Value>) -> Option<PastGame> {
    if !wanted(game) {
        return None;
    }
    let parts: Vec<&Value> = arr(&game["participants"]).collect();
    let me_id = arr(&game["participantIdentities"])
        .find(|i| puuid.is_some() && i.pointer("/player/puuid").and_then(Value::as_str) == puuid)
        .and_then(|i| i["participantId"].as_i64())
        .or_else(|| (parts.len() == 1).then(|| parts[0]["participantId"].as_i64()).flatten())?;
    let me = *parts.iter().find(|p| p["participantId"].as_i64() == Some(me_id))?;
    let s = &me["stats"];
    let n = |k: &str| s[k].as_f64().map_or(0, |v| v.max(0.0) as u32);
    let queue_id = game["queueId"].as_i64()?;
    let mode = Mode::from_queue(queue_id);
    let mut duration_s = game["gameDuration"].as_u64()? as u32;
    if duration_s > 20_000 {
        duration_s /= 1000; // milliseconds in some client versions
    }

    let my_team = me["teamId"].as_i64();
    let lane = if mode == Mode::Rift { lane_of(me) } else { None };
    let opp = lane.and_then(|l| {
        let mut it = parts.iter().filter(|p| p["teamId"].as_i64() != my_team && lane_of(p) == Some(l));
        let o = it.next()?;
        it.next().is_none().then(|| o["participantId"].as_i64()).flatten()
    });

    let teams: Vec<&Value> = arr(&game["teams"]).collect();
    let ours = teams.iter().find(|t| t["teamId"].as_i64() == my_team);
    let theirs = teams.iter().find(|t| my_team.is_some() && t["teamId"].as_i64() != my_team);
    let first = |keys: &[&str]| {
        let got = |t: Option<&&Value>| t.is_some_and(|t| keys.iter().any(|k| t[*k] == true));
        if got(ours) { Some(true) } else if got(theirs) { Some(false) } else { None }
    };

    let frames = timeline.and_then(|t| t["frames"].as_array());
    let at = |min: u64, key: &str, pid: i64| -> Option<i64> {
        let f = frames?.iter().find(|f| f["timestamp"].as_u64().is_some_and(|t| t >= min * 60_000))?;
        f["participantFrames"][pid.to_string()][key].as_i64()
    };
    let diff = |min, key| Some((at(min, key, me_id)? - at(min, key, opp?)?) as i32);
    let death_times = frames.map(|fs| {
        fs.iter()
            .flat_map(|f| arr(&f["events"]))
            .filter(|e| e["type"] == "CHAMPION_KILL" && e["victimId"].as_i64() == Some(me_id))
            .filter_map(|e| Some((e["timestamp"].as_u64()? / 1000) as u32))
            .collect()
    });

    Some(PastGame {
        game_id: game["gameId"].as_u64()?,
        created: game["gameCreation"].as_u64().unwrap_or(0),
        queue_id,
        mode,
        champ: id(&me["championId"])?,
        lane,
        win: s["win"] == true,
        remake: duration_s < 300 || s["gameEndedInEarlySurrender"] == true,
        duration_s,
        kills: n("kills"),
        deaths: n("deaths"),
        assists: n("assists"),
        cs: n("totalMinionsKilled") + n("neutralMinionsKilled"),
        gold: n("goldEarned"),
        damage_to_champs: n("totalDamageDealtToChampions"),
        vision: n("visionScore"),
        death_times,
        gold_diff_10: diff(10, "totalGold"),
        gold_diff_15: diff(15, "totalGold"),
        xp_diff_15: diff(15, "xp"),
        ours: ours.map(|t| objectives(t)),
        theirs: theirs.map(|t| objectives(t)),
        first_dragon: first(&["firstDargon", "firstDragon"]),
        first_tower: first(&["firstTower"]),
        first_blood: first(&["firstBlood"]),
    })
}
