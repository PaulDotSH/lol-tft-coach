//! Live Client Data API (`https://127.0.0.1:2999/liveclientdata/allgamedata`).

use crate::id;
use model::{GameEvent, GameSnapshot, Lane, Mode, Player, StaticData, Team};
use serde_json::Value;
use std::time::Duration;

const URL: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

pub enum LiveState {
    /// No game process / connection refused.
    Unavailable,
    /// Game running but data not ready yet.
    Loading,
    InGame(GameSnapshot),
}

#[derive(Clone)]
pub struct Live {
    pub client: reqwest::Client,
}

impl Default for Live {
    fn default() -> Self {
        Live::new()
    }
}

impl Live {
    pub fn new() -> Live {
        Live { client: crate::local_client(Duration::from_millis(1500)) }
    }

    /// Also returns the raw JSON, for the fixture recorder.
    pub async fn poll(&self, sd: &StaticData) -> (LiveState, Option<Value>) {
        let resp = match self.client.get(URL).send().await {
            Ok(r) => r,
            Err(e) if e.is_connect() => return (LiveState::Unavailable, None),
            Err(_) => return (LiveState::Loading, None),
        };
        if !resp.status().is_success() {
            return (LiveState::Loading, None);
        }
        let Ok(json) = resp.json::<Value>().await else { return (LiveState::Loading, None) };
        match parse(&json, sd) {
            Some(s) => (LiveState::InGame(s), Some(json)),
            None => (LiveState::Loading, Some(json)),
        }
    }
}

fn arr(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn num(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn count(v: &Value) -> u32 {
    v.as_f64().map_or(0, |n| n.max(0.0) as u32)
}

/// Display name first ("Kai'Sa"), then the Data Dragon id in `rawChampionName`
/// ("game_character_displayname_MonkeyKing") for localized clients. 0 if unknown.
fn champ_key(p: &Value, sd: &StaticData) -> u32 {
    sd.champ_by_name(str_of(&p["championName"]))
        .or_else(|| sd.champ_by_name(str_of(&p["rawChampionName"]).strip_prefix("game_character_displayname_")?))
        .map_or(0, |c| c.key)
}

fn player(p: &Value, sd: &StaticData) -> Player {
    let s = &p["scores"];
    let riot_id = [&p["riotId"], &p["summonerName"]].into_iter().map(str_of).find(|s| !s.is_empty()).unwrap_or_default();
    Player {
        riot_id: riot_id.to_string(),
        champ: champ_key(p, sd),
        team: if p["team"] == "CHAOS" { Team::Chaos } else { Team::Order },
        position: Lane::from_riot(str_of(&p["position"])),
        level: count(&p["level"]),
        kills: count(&s["kills"]),
        deaths: count(&s["deaths"]),
        assists: count(&s["assists"]),
        cs: count(&s["creepScore"]),
        ward_score: num(&s["wardScore"]),
        items: arr(&p["items"]).filter_map(|i| id(&i["itemID"])).collect(),
        dead: p["isDead"] == true,
        respawn_in: num(&p["respawnTimer"]),
    }
}

/// Index of the player any of whose names equals `name` (events use game names).
fn resolve(all: &[Value], name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    all.iter().position(|p| ["riotIdGameName", "riotId", "summonerName"].iter().any(|k| p[*k] == name))
}

fn mode(game: &Value) -> Mode {
    match str_of(&game["gameMode"]) {
        "CLASSIC" => Mode::Rift,
        "ARAM" => Mode::Aram,
        _ if game["mapNumber"] == 12 => Mode::Mayhem,
        _ => Mode::Rift,
    }
}

/// Pure parser (tests use fixtures). Champion names resolve via `sd`.
/// None when there is no player list or no identifiable active player (spectating).
pub fn parse(json: &Value, sd: &StaticData) -> Option<GameSnapshot> {
    let all = json.get("allPlayers")?.as_array()?;
    let active = json.get("activePlayer")?;
    let me = ["riotId", "riotIdGameName", "summonerName"].iter().find_map(|k| {
        let name = str_of(&active[*k]);
        all.iter().position(|p| !name.is_empty() && p[*k] == name)
    })?;
    let players: Vec<Player> = all.iter().map(|p| player(p, sd)).collect();
    let who = |v: &Value| resolve(all, str_of(v));
    let team_of = |v: &Value| who(v).map(|i| players[i].team);

    let events = arr(&json["events"]["Events"])
        .filter_map(|e| {
            let time = num(&e["EventTime"]);
            let team = team_of(&e["KillerName"]);
            Some(match str_of(&e["EventName"]) {
                "ChampionKill" => GameEvent::Kill {
                    time,
                    killer: who(&e["KillerName"]),
                    victim: who(&e["VictimName"])?,
                    assisters: arr(&e["Assisters"]).filter_map(who).collect(),
                },
                "DragonKill" => GameEvent::Dragon { time, team, kind: str_of(&e["DragonType"]).to_string() },
                "BaronKill" => GameEvent::Baron { time, team },
                "HeraldKill" => GameEvent::Herald { time, team },
                "GameEnd" => GameEvent::GameEnd { time, win: e["Result"] == "Win" },
                // HordeKill (voidgrubs), AtakhanKill, … → name without the suffix.
                n if n.len() > 4 && n.ends_with("Kill") => GameEvent::Objective { time, team, name: n[..n.len() - 4].to_string() },
                _ => return None,
            })
        })
        .collect();

    let game = &json["gameData"];
    Some(GameSnapshot {
        time: num(&game["gameTime"]),
        mode: mode(game),
        map: count(&game["mapNumber"]),
        me,
        my_gold: num(&active["currentGold"]),
        players,
        events,
    })
}
