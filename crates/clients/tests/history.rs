//! Match-history parser tests over `fixtures/clients/match_*.json`: hand-built
//! from the documented LCU schema, except the recorded `*_flex.json` game (see `_fixture`).

use clients::history;
use model::{DeathSpot, Lane, MAP_END, Mode, PastGame, TeamObjectives};
use serde_json::Value;

const ME: &str = "8f2c1d4e-me00-4a6b-9c1d-000000000000";

fn fixture(name: &str) -> Value {
    let path = format!("{}/../../fixtures/clients/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
fn list_keeps_rift_and_abyss_matchmade_games() {
    let list = fixture("match_history.json");
    let games: Vec<PastGame> = history::list_games(&list).filter_map(|g| history::parse_game(g, Some(ME), None)).collect();
    // Practice tool (queue 0), Arena (map 30) and co-op vs AI (870) are dropped.
    let ids: Vec<(i64, Mode, bool)> = games.iter().map(|g| (g.queue_id, g.mode, g.remake)).collect();
    assert_eq!(ids, [(420, Mode::Rift, false), (450, Mode::Aram, false), (2400, Mode::Mayhem, false), (420, Mode::Rift, true)]);

    let aram = &games[1];
    assert_eq!((aram.champ, aram.win, aram.kills, aram.deaths, aram.assists), (99, true, 14, 6, 31));
    assert_eq!(aram.lane, None, "no lanes outside the Rift");
    assert_eq!((aram.death_times.as_ref(), aram.ours.as_ref(), aram.first_dragon), (None, None, None));

    assert_eq!(games[2].augments, [1030, 2132, 1389]);
    assert_eq!(games[2].cs_10, None, "no timeline");

    // List entries have no teams and no other players.
    let rift = &games[0];
    assert_eq!((rift.lane, rift.gold_diff_15, rift.first_tower), (Some(Lane::Middle), None, None));
}

#[test]
fn detail_and_timeline() {
    let g = history::parse_game(&fixture("match_game.json"), Some(ME), Some(&fixture("match_timeline.json"))).unwrap();
    assert_eq!(
        g,
        PastGame {
            game_id: 3712004411,
            created: 1792610000000,
            queue_id: 420,
            mode: Mode::Rift,
            champ: 103,
            lane: Some(Lane::Middle),
            win: false,
            remake: false,
            duration_s: 1874,
            kills: 4,
            deaths: 7,
            assists: 6,
            cs: 202,
            gold: 11840,
            damage_to_champs: 21400,
            vision: 18,
            death_times: Some(vec![372, 700, 1500]),
            gold_diff_10: Some(-600),
            gold_diff_15: Some(-900),
            xp_diff_15: Some(-600),
            ours: Some(TeamObjectives { dragons: 1, barons: 0, heralds: 0, grubs: 3, towers: 3 }),
            theirs: Some(TeamObjectives { dragons: 3, barons: 1, heralds: 1, grubs: 3, towers: 9 }),
            first_dragon: Some(false),
            first_tower: Some(false),
            first_blood: Some(true),
            cs_10: Some(70),
            augments: vec![],
            death_spots: vec![
                DeathSpot { time: 372, x: 6800, y: 7100 },
                DeathSpot { time: 700, x: 7400, y: 7300 },
                DeathSpot { time: 1500, x: 9000, y: 9000 },
            ],
            enemies: vec![122, 254, 145, 238, 111],
            opponent: Some(238),
            killed_by: vec![Some(238), Some(238), Some(145)],
        }
    );
}

#[test]
fn detail_edge_cases() {
    let game = fixture("match_game.json");
    // Without my puuid, ten participants are ambiguous.
    assert_eq!(history::parse_game(&game, None, None), None);
    assert_eq!(history::parse_game(&game, Some("someone-else"), None), None);

    // Support: lane BOTTOM + role DUO_SUPPORT; opponent is the enemy support.
    let mut j = game.clone();
    j["participantIdentities"][3]["player"]["puuid"] = "x".into();
    j["participantIdentities"][4]["player"]["puuid"] = ME.into();
    let g = history::parse_game(&j, Some(ME), Some(&fixture("match_timeline.json"))).unwrap();
    assert_eq!((g.champ, g.lane), (412, Some(Lane::Support)));
    assert_eq!(g.gold_diff_15, Some((250 - 270) * 15));
    assert_eq!(g.death_times, Some(vec![]));

    // Two enemies in my lane: no single opponent, no diffs.
    let mut j = game.clone();
    j["participants"][5]["timeline"]["lane"] = "MIDDLE".into();
    let g = history::parse_game(&j, Some(ME), Some(&fixture("match_timeline.json"))).unwrap();
    assert_eq!((g.gold_diff_10, g.xp_diff_15), (None, None));

    // Short timeline (game over before 15:00): only the 10-minute diff.
    let mut tl = fixture("match_timeline.json");
    tl["frames"].as_array_mut().unwrap().truncate(13);
    let g = history::parse_game(&game, Some(ME), Some(&tl)).unwrap();
    assert_eq!((g.gold_diff_10, g.gold_diff_15), (Some(-600), None));

    // Duration in milliseconds; nobody took a dragon.
    let mut j = game.clone();
    j["gameDuration"] = 1874000.into();
    for t in j["teams"].as_array_mut().unwrap() {
        t["firstDargon"] = false.into();
    }
    let g = history::parse_game(&j, Some(ME), None).unwrap();
    assert_eq!((g.duration_s, g.first_dragon), (1874, None));
}

/// Needs a running League client: `cargo test -p clients --test history -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn live_history() {
    let lcu = tokio::task::spawn_blocking(clients::lcu::discover).await.unwrap().expect("League client running");
    let dir = std::env::temp_dir().join("lol-coach-history-test");
    let t = std::time::Instant::now();
    let games = history::sync(&lcu, &dir).await.unwrap();
    println!("{} games in {:?} (cache {})", games.len(), t.elapsed(), dir.display());
    for g in &games {
        println!(
            "{} q{} {:?} champ {} {:?} {} {}/{}/{} deaths@{:?} gold15 {:?} first dragon {:?}",
            g.game_id, g.queue_id, g.mode, g.champ, g.lane, if g.win { "W" } else { "L" }, g.kills, g.deaths, g.assists,
            g.death_times, g.gold_diff_15, g.first_dragon
        );
    }
    let again = history::sync(&lcu, &dir).await.unwrap();
    assert_eq!(again.len(), games.len());
}

#[test]
fn recorded_game_lanes_from_positions_and_full_match() {
    // Recorded flex game: Riot labels two supports on my team and two junglers on
    // theirs; where each player stood sorts them out.
    let (game, tl) = (fixture("match_game_flex.json"), fixture("match_timeline_flex.json"));
    let d = history::parse_match(&game, Some("me-puuid"), Some(&tl)).unwrap();
    let lanes: Vec<(u32, Option<Lane>)> = d.players.iter().map(|p| (p.champ, p.lane)).collect();
    use Lane::*;
    assert_eq!(
        lanes,
        [
            (799, Some(Top)),
            (131, Some(Jungle)),
            (27, Some(Middle)),
            (22, Some(Bottom)),
            (111, Some(Support)),
            (79, Some(Top)),
            (234, Some(Jungle)),
            (246, Some(Middle)),
            (523, Some(Bottom)),
            (201, Some(Support)),
        ]
    );
    assert_eq!((d.me, d.game.lane, d.game.gold_diff_15), (4, Some(Support), Some(345)));
    let me = &d.players[4];
    assert!(me.me && me.ally && me.name == "Me" && !d.players[5].ally);
    assert_eq!((d.frames.len(), d.kills.len()), (31, 60));
    // Grubs fold into one mark; a herald with no killer is left out; buildings count for the side that took them.
    let m = &d.marks[0];
    assert_eq!((m.kind.as_str(), m.ours, m.detail.as_deref()), ("Grubs", true, Some("×3")));
    assert!(!d.marks.iter().any(|m| m.kind == "Herald"));
    let first_tower = d.marks.iter().find(|m| m.kind == "Tower").unwrap();
    assert_eq!((first_tower.ours, first_tower.detail.as_deref()), (false, Some("top outer")));
    assert_eq!(d.marks.iter().filter(|m| m.kind == "Dragon" && m.ours).count(), 3);
}

#[test]
fn death_spots_are_seen_from_blue_side() {
    let (game, tl) = (fixture("match_game_flex.json"), fixture("match_timeline_flex.json"));
    let g = history::parse_game(&game, Some("me-puuid"), Some(&tl)).unwrap();
    let times: Vec<u32> = g.death_spots.iter().map(|d| d.time).collect();
    assert_eq!(Some(times), g.death_times);
    // I was blue here: positions as recorded.
    let first = tl["frames"].as_array().unwrap().iter().flat_map(|f| f["events"].as_array().unwrap()).find(|e| e["victimId"] == 5).unwrap();
    assert_eq!((g.death_spots[0].x, g.death_spots[0].y), (first["position"]["x"].as_i64().unwrap() as i32, first["position"]["y"].as_i64().unwrap() as i32));
    // The same game as a red-side player: turned around.
    let mut j = game.clone();
    j["participantIdentities"][4]["player"]["puuid"] = "x".into();
    j["participantIdentities"][9]["player"]["puuid"] = "me-puuid".into();
    let r = history::parse_game(&j, Some("me-puuid"), Some(&tl)).unwrap();
    let e = tl["frames"].as_array().unwrap().iter().flat_map(|f| f["events"].as_array().unwrap()).find(|e| e["victimId"] == 10).unwrap();
    assert_eq!((r.death_spots[0].x, r.death_spots[0].y), (MAP_END - e["position"]["x"].as_i64().unwrap() as i32, MAP_END - e["position"]["y"].as_i64().unwrap() as i32));
}
