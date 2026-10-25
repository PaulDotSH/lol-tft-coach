//! LCU TFT match history (`/lol-match-history/v1/products/tft/{puuid}/matches`) → my games.
//! The response has no augments (Riot removed them from match data after Set 13).

use serde_json::Value;
use tft_model::{TftMode, TftPastGame, TftPastUnit};

/// My games, newest first as the LCU sends them. I am the participant whose `puuid` is the
/// response's `active_puuid`, or `me` when the client leaves that empty (it sometimes does);
/// games without me are skipped. Only active traits are kept, as
/// (trait, units, tier reached). `comp` is left for the coach to fill.
pub fn parse_history(json: &Value, me: Option<&str>) -> Vec<TftPastGame> {
    let Some(me) = json["active_puuid"].as_str().filter(|s| !s.is_empty()).or(me) else { return Vec::new() };
    let u32_of = |v: &Value| v.as_u64().unwrap_or(0) as u32;
    let games = json["games"].as_array().into_iter().flatten();
    games
        .filter_map(|g| {
            let g = &g["json"];
            let p = g["participants"].as_array()?.iter().find(|p| p["puuid"].as_str() == Some(me))?;
            let queue = g["queueId"].as_i64().or_else(|| g["queue_id"].as_i64()).unwrap_or(0);
            let units = p["units"].as_array().into_iter().flatten().filter_map(|u| {
                let items = u["itemNames"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
                let api = u["character_id"].as_str()?.to_owned();
                Some(TftPastUnit { api, star: u32_of(&u["tier"]), items: items.collect() })
            });
            let traits = p["traits"].as_array().into_iter().flatten().filter_map(|t| {
                let tier = u32_of(&t["tier_current"]);
                if tier == 0 {
                    return None;
                }
                Some((t["name"].as_str()?.to_owned(), u32_of(&t["num_units"]), tier))
            });
            Some(TftPastGame {
                game_id: g["gameId"].as_u64().or_else(|| g["game_id"].as_u64())?,
                created: g["gameCreation"].as_u64().or_else(|| g["game_datetime"].as_u64()).unwrap_or(0),
                queue,
                mode: TftMode::of_queue(queue),
                placement: u32_of(&p["placement"]),
                level: u32_of(&p["level"]),
                last_round: u32_of(&p["last_round"]),
                duration_s: g["game_length"].as_f64().unwrap_or(0.0) as u32,
                units: units.collect(),
                traits: traits.collect(),
                comp: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_games() {
        let json: Value = serde_json::from_str(&crate::fixture("lcu_tft_history.json")).unwrap();
        let games = parse_history(&json, None);
        assert_eq!(games.len(), 20);
        // An empty `active_puuid`: my puuid from the caller.
        let mut blank = json.clone();
        blank["active_puuid"] = "".into();
        assert!(parse_history(&blank, None).is_empty());
        assert_eq!(parse_history(&blank, Some("me-puuid")).len(), 20);
        let g = &games[0];
        assert_eq!((g.game_id, g.created, g.queue, g.mode), (4019475174, 1792877669575, 1230, TftMode::Choncc));
        assert_eq!((g.placement, g.level, g.last_round, g.duration_s), (2, 10, 40, 2290));
        assert_eq!(g.units.len(), 16);
        let dragon = &g.units[0];
        assert_eq!((dragon.api.as_str(), dragon.star), ("DA_18_ElderDragon", 3));
        assert_eq!(dragon.items, ["DA_InfinityEdgeRadiant", "DA_EdgeOfNight", "DA_LastWhisper"]);
        assert_eq!(g.traits.len(), 9, "only tier_current > 0");
        assert!(g.traits.contains(&("DA_Riftbeast18".into(), 7, 3)));
        assert!(g.comp.is_none());

        let modes: Vec<_> = games.iter().map(|g| g.mode).collect();
        assert_eq!(modes.iter().filter(|&&m| m == TftMode::Choncc).count(), 5);
        assert_eq!(games.iter().filter(|g| g.queue == 1100).count(), 14);
        assert_eq!(games.iter().find(|g| g.queue == 1220).unwrap().mode, TftMode::Standard);
        assert_eq!(games.iter().filter(|g| g.placement <= 4).count(), 12);
    }

    #[test]
    fn without_me_or_garbage() {
        assert!(parse_history(&Value::Null, None).is_empty());
        let game = serde_json::json!({"json": {"gameId": 1, "participants": [{"puuid": "x"}]}});
        let other = serde_json::json!({"active_puuid": "nobody", "games": [game]});
        assert!(parse_history(&other, None).is_empty());
    }
}
