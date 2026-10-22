//! Parser tests over `fixtures/clients/`. All fixtures are hand-built from the
//! documented schemas except `live_sample_riot.json`, Riot's own sample from
//! https://static.developer.riotgames.com/docs/lol/liveclientdata_sample.json
//! (old single-player format: `summonerName` only, no `riotId`).

use clients::{lcu, live};
use model::{ChampSelect, Champion, GameEvent, Lane, Mode, StaticData, Team};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = format!("{}/../../fixtures/clients/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn sd() -> StaticData {
    let champs = [
        (1, "Annie", "Annie"),
        (62, "MonkeyKing", "Wukong"),
        (64, "LeeSin", "Lee Sin"),
        (86, "Garen", "Garen"),
        (103, "Ahri", "Ahri"),
        (111, "Nautilus", "Nautilus"),
        (145, "Kaisa", "Kai'Sa"),
        (222, "Jinx", "Jinx"),
        (238, "Zed", "Zed"),
        (254, "Vi", "Vi"),
        (412, "Thresh", "Thresh"),
    ];
    StaticData {
        version: "16.21.1".into(),
        items: Default::default(),
        champs: champs
            .into_iter()
            .map(|(key, id, name)| {
                let c = Champion {
                    key,
                    id: id.into(),
                    name: name.into(),
                    slug: id.to_lowercase(),
                    classes: vec![],
                    traits: Default::default(),
                };
                (key, c)
            })
            .collect(),
    }
}

#[test]
fn champ_select_draft_mid_pick() {
    let cs = lcu::parse_champ_select(&fixture("champ_select_draft.json"), 420).unwrap();
    assert_eq!(
        cs,
        ChampSelect {
            queue_id: 420,
            mode: Mode::Rift,
            my_lane: Some(Lane::Middle),
            my_hover: Some(103),
            my_pick: None,
            locked: false,
            allies: vec![86, 64],
            enemies: vec![238, 254],
            bans: vec![157, 777, 555, 875, 122, 266, 11, 84, 245],
            bench: vec![],
            timer_phase: "BAN_PICK".into(),
        }
    );
}

#[test]
fn champ_select_draft_locked_and_planning() {
    let mut j = fixture("champ_select_draft.json");
    j["actions"][4][1]["completed"] = true.into();
    let cs = lcu::parse_champ_select(&j, 420).unwrap();
    assert_eq!((cs.my_pick, cs.my_hover, cs.locked), (Some(103), None, true));

    // Before my turn: action has no champion yet, only the pick intent.
    let mut j = fixture("champ_select_draft.json");
    j["actions"][4][1]["championId"] = 0.into();
    j["myTeam"][2]["championId"] = 0.into();
    j["myTeam"][2]["championPickIntent"] = 7.into();
    let cs = lcu::parse_champ_select(&j, -1).unwrap();
    assert_eq!((cs.my_pick, cs.my_hover, cs.locked, cs.queue_id), (None, Some(7), false, 420));
}

#[test]
fn champ_select_bans_from_actions_only() {
    let mut j = fixture("champ_select_draft.json");
    j["bans"] = serde_json::json!({ "myTeamBans": [], "theirTeamBans": [], "numBans": 10 });
    j["actions"][0][6]["completed"] = false.into();
    let cs = lcu::parse_champ_select(&j, 420).unwrap();
    assert_eq!(cs.bans, vec![157, 777, 555, 875, 122, 11, 84, 245]);
}

#[test]
fn champ_select_aram_bench() {
    let cs = lcu::parse_champ_select(&fixture("champ_select_aram.json"), 450).unwrap();
    assert_eq!(cs.mode, Mode::Aram);
    assert_eq!((cs.my_pick, cs.my_hover, cs.locked, cs.my_lane), (Some(21), None, true, None));
    assert_eq!(cs.allies, vec![99, 89, 53, 1]);
    assert!(cs.enemies.is_empty() && cs.bans.is_empty());
    assert_eq!(cs.bench, vec![22, 115, 360]);
}

#[test]
fn champ_select_rejects_non_session() {
    let err = serde_json::json!({ "errorCode": "RPC_ERROR", "httpStatus": 404, "message": "No active delegate" });
    assert_eq!(lcu::parse_champ_select(&err, 420), None);
}

#[test]
fn eog_local_player() {
    let e = lcu::parse_eog(&fixture("eog_stats.json")).unwrap();
    assert_eq!((e.champ, e.mode, e.win, e.duration_s), (103, Mode::Rift, true, 1834));
    assert_eq!((e.kills, e.deaths, e.assists, e.cs), (7, 3, 9, 213));
    assert_eq!((e.gold, e.damage_to_champs, e.damage_taken, e.vision), (13450, 25432, 18765, 24));
    assert_eq!(e.items, vec![6655, 3020, 4645, 3089, 1058, 3340]);
}

#[test]
fn eog_fallbacks() {
    // No localPlayer: found in teams by summonerId; win from the team when stats lack WIN/LOSE.
    let mut j = fixture("eog_stats.json");
    j.as_object_mut().unwrap().remove("localPlayer");
    j["teams"][0]["players"][2]["isLocalPlayer"] = false.into();
    j["teams"][0]["players"][2]["stats"].as_object_mut().unwrap().remove("WIN");
    j["queueId"] = 450.into();
    let e = lcu::parse_eog(&j).unwrap();
    assert_eq!((e.champ, e.win, e.mode, e.kills), (103, true, Mode::Aram, 7));

    j["teams"][0]["isWinningTeam"] = false.into();
    assert!(!lcu::parse_eog(&j).unwrap().win);
    assert_eq!(lcu::parse_eog(&serde_json::json!({})), None);
}

#[test]
fn live_full_game() {
    let g = live::parse(&fixture("live_allgamedata.json"), &sd()).unwrap();
    assert_eq!((g.mode, g.map, g.me), (Mode::Rift, 11, 2));
    assert!((g.time - 1030.512).abs() < 1e-3 && (g.my_gold - 1234.56).abs() < 1e-3);
    assert_eq!(g.players.len(), 10);
    let champs: Vec<u32> = g.players.iter().map(|p| p.champ).collect();
    assert_eq!(champs, vec![86, 64, 103, 222, 412, 62, 254, 238, 145, 111]);

    let me = &g.players[2];
    assert_eq!((me.riot_id.as_str(), me.team, me.position, me.level), ("Mid Main#EUW", Team::Order, Some(Lane::Middle), 11));
    assert_eq!((me.kills, me.deaths, me.assists, me.cs), (1, 2, 1, 142));
    assert_eq!(me.items, vec![1056, 6655, 3020, 1058, 3340]);
    let nau = &g.players[9];
    assert_eq!((nau.team, nau.position), (Team::Chaos, Some(Lane::Support)));
    let jinx = &g.players[3];
    assert!(jinx.dead && (jinx.respawn_in - 18.6).abs() < 1e-3);

    use GameEvent::*;
    let ev = &g.events;
    assert_eq!(ev.len(), 11, "{ev:#?}");
    assert!(matches!(ev[0], Kill { killer: Some(7), victim: 2, ref assisters, .. } if *assisters == [6]));
    assert!(ev[1..4].iter().all(|e| matches!(e, Objective { team: Some(Team::Order), name, .. } if name == "Horde")));
    assert!(matches!(&ev[4], Dragon { team: Some(Team::Chaos), kind, .. } if kind == "Fire"));
    assert!(matches!(ev[5], Kill { killer: Some(2), victim: 7, .. }));
    assert!(matches!(ev[6], Kill { killer: None, victim: 0, ref assisters, .. } if *assisters == [5]));
    assert!(matches!(ev[7], Kill { killer: Some(7), victim: 2, ref assisters, .. } if assisters.is_empty()));
    assert!(matches!(ev[8], Herald { team: Some(Team::Chaos), .. }));
    assert!(matches!(&ev[9], Dragon { team: Some(Team::Order), kind, .. } if kind == "Earth"));
    assert!(matches!(ev[10], Kill { killer: Some(8), victim: 3, .. }));
}

#[test]
fn live_modes_end_and_unknowns() {
    let sd = sd();
    let mut j = fixture("live_allgamedata.json");
    let events = j["events"]["Events"].as_array_mut().unwrap();
    events.push(serde_json::json!({ "EventID": 99, "EventName": "BaronKill", "EventTime": 1500.0, "KillerName": "Minion_T100L2S30N0123", "Assisters": [] }));
    events.push(serde_json::json!({ "EventID": 100, "EventName": "GameEnd", "EventTime": 1600.0, "Result": "Lose" }));
    j["allPlayers"][0]["championName"] = "Garen (localized)".into();
    j["allPlayers"][1]["championName"] = "Brand New Champ".into();
    j["allPlayers"][1]["rawChampionName"] = "game_character_displayname_Unknown".into();
    j["gameData"]["gameMode"] = "ARAM".into();
    j["gameData"]["mapNumber"] = 12.into();
    let g = live::parse(&j, &sd).unwrap();
    assert_eq!(g.mode, Mode::Aram);
    assert_eq!((g.players[0].champ, g.players[1].champ), (86, 0));
    let n = g.events.len();
    assert!(matches!(g.events[n - 2], GameEvent::Baron { team: None, .. }));
    assert!(matches!(g.events[n - 1], GameEvent::GameEnd { win: false, .. }));

    j["gameData"]["gameMode"] = "KIWI".into();
    assert_eq!(live::parse(&j, &sd).unwrap().mode, Mode::Mayhem);
    j["gameData"]["mapNumber"] = 11.into();
    assert_eq!(live::parse(&j, &sd).unwrap().mode, Mode::Rift);

    // Spectator mode: activePlayer is an error object.
    j["activePlayer"] = serde_json::json!({ "error": "Spectator mode doesn't currently support this feature" });
    assert!(live::parse(&j, &sd).is_none());
    assert!(live::parse(&serde_json::json!({ "errorCode": "RESOURCE_NOT_FOUND" }), &sd).is_none());
}

#[test]
fn live_riot_sample_legacy_names() {
    let g = live::parse(&fixture("live_sample_riot.json"), &sd()).unwrap();
    assert_eq!((g.me, g.mode, g.map, g.time), (0, Mode::Rift, 11, 0.0));
    let p = &g.players[0];
    assert_eq!((p.riot_id.as_str(), p.champ, p.position, p.team), ("Riot Tuxedo", 1, None, Team::Order));
    assert!(g.events.is_empty());
}

// ---- real APIs (run with `cargo test -p clients -- --ignored` while playing) ----

#[tokio::test]
#[ignore = "needs a running League client"]
async fn real_lcu() {
    let lcu = tokio::task::spawn_blocking(lcu::discover).await.unwrap().expect("client running");
    let phase = lcu.phase().await.unwrap();
    println!("phase {phase}, queue {:?}", lcu.queue_id().await.unwrap());
    println!("champ select {:#?}", lcu.champ_select().await.unwrap());
    println!("eog {:#?}", lcu.eog_stats().await.unwrap());
}

#[tokio::test]
#[ignore = "needs a game in progress"]
async fn real_live_game() {
    let (state, raw) = live::Live::new().poll(&sd()).await;
    match state {
        live::LiveState::InGame(g) => println!("{} players at {:.0}s, me {}", g.players.len(), g.time, g.me),
        live::LiveState::Loading => println!("loading, raw: {}", raw.is_some()),
        live::LiveState::Unavailable => panic!("no game"),
    }
}

#[tokio::test]
#[ignore = "expects no game running: checks refused → Unavailable quickly"]
async fn real_live_unavailable() {
    let t = std::time::Instant::now();
    let (state, raw) = live::Live::new().poll(&sd()).await;
    assert!(matches!(state, live::LiveState::Unavailable) && raw.is_none());
    println!("refused in {:?}", t.elapsed());
}
