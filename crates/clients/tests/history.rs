//! Match-history parser tests over `fixtures/clients/match_*.json`, hand-built
//! from the documented LCU schema (not recorded responses; see `_fixture`).

use clients::history;
use model::{Lane, Mode, PastGame, TeamObjectives};
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
