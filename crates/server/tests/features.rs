use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::GzEncoder;
use lol_coach_server::features::{self, Game, KEYS, Parsed, Tables, Values};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::OnceLock;
use tempfile::TempDir;

fn values() -> &'static Values {
    static VALUES: OnceLock<Values> = OnceLock::new();
    VALUES.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/ddragon-item.json");
        features::item_values(&serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()).unwrap()
    })
}

fn ev(ts: i64, kind: &str, fields: Value) -> Value {
    let mut e = json!({ "type": kind, "timestamp": ts });
    e.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    e
}

fn buy(ts: i64, pid: i64, item: i64) -> Value {
    ev(ts, "ITEM_PURCHASED", json!({ "participantId": pid, "itemId": item }))
}

fn gone(ts: i64, pid: i64, item: i64) -> Value {
    ev(ts, "ITEM_DESTROYED", json!({ "participantId": pid, "itemId": item }))
}

fn participant(pid: i64, champ: i64) -> Value {
    let mut p = json!({ "participantId": pid, "teamId": if pid <= 5 { 100 } else { 200 }, "championId": champ, "gameEndedInEarlySurrender": false, "teamEarlySurrendered": false });
    for i in 0..7 {
        p[format!("item{i}")] = json!(0);
    }
    p
}

fn hand_game(extra: Vec<Value>) -> (Value, Value) {
    let mut events = vec![
        ev(0, "PAUSE_END", json!({})),
        buy(1000, 1, 1055),
        buy(1000, 1, 2003),
        buy(1500, 6, 1036),
        buy(1600, 6, 1036),
        ev(2000, "SKILL_LEVEL_UP", json!({ "participantId": 1, "skillSlot": 1 })),
        gone(30000, 1, 2003),
        buy(100000, 6, 3133),
        gone(100000, 6, 1036),
        gone(100000, 6, 1036),
        ev(105000, "ITEM_UNDO", json!({ "participantId": 6, "beforeId": 3133, "afterId": 0, "goldGain": 1050 })),
        gone(110000, 6, 1036),
        gone(110000, 6, 1036),
        buy(110000, 6, 3133),
        ev(130000, "ITEM_SOLD", json!({ "participantId": 1, "itemId": 1055 })),
        ev(131000, "ITEM_UNDO", json!({ "participantId": 1, "beforeId": 0, "afterId": 1055, "goldGain": -180 })),
        buy(140000, 1, 1001),
        ev(235000, "CHAMPION_KILL", json!({ "killerId": 1, "victimId": 6, "assistingParticipantIds": [2] })),
        ev(250000, "WARD_PLACED", json!({ "creatorId": 3, "wardType": "YELLOW_TRINKET" })),
        ev(300000, "BUILDING_KILL", json!({ "buildingType": "TOWER_BUILDING", "teamId": 200, "killerId": 2, "laneType": "MID_LANE", "towerType": "OUTER_TURRET" })),
        ev(330000, "ELITE_MONSTER_KILL", json!({ "monsterType": "DRAGON", "monsterSubType": "FIRE_DRAGON", "killerTeamId": 200, "killerId": 7 })),
        ev(360000, "BUILDING_KILL", json!({ "buildingType": "INHIBITOR_BUILDING", "teamId": 100, "killerId": 8, "laneType": "BOT_LANE" })),
        ev(400000, "ELITE_MONSTER_KILL", json!({ "monsterType": "HORDE", "killerTeamId": 100, "killerId": 2 })),
        ev(401000, "ELITE_MONSTER_KILL", json!({ "monsterType": "HORDE", "killerTeamId": 100, "killerId": 2 })),
        ev(420000, "ELITE_MONSTER_KILL", json!({ "monsterType": "RIFTHERALD", "killerTeamId": 200, "killerId": 7 })),
        ev(700000, "CHAMPION_KILL", json!({ "killerId": 0, "victimId": 2, "assistingParticipantIds": [] })),
        ev(750000, "GAME_END", json!({ "gameId": 1, "winningTeam": 100, "realTimestamp": 0 })),
    ];
    events.extend(extra);
    let stamps: Vec<i64> = (0..13).map(|m| m * 60000).chain([750000]).collect();
    let mut frames: Vec<Value> = stamps
        .iter()
        .map(|ts| {
            let m = ts / 60000;
            let pf: serde_json::Map<String, Value> = (1..=10)
                .map(|p: i64| {
                    let level = 1 + m / 2 + i64::from(p == 1);
                    let minions = if p <= 5 { 6 * m } else { 5 * m };
                    (p.to_string(), json!({ "participantId": p, "level": level, "minionsKilled": minions, "jungleMinionsKilled": if p == 2 { 4 * m } else { 0 }, "totalGold": 500, "currentGold": 500, "xp": 0 }))
                })
                .collect();
            json!({ "timestamp": ts, "participantFrames": pf, "events": [] })
        })
        .collect();
    for e in events {
        let ts = e["timestamp"].as_i64().unwrap_or(5000);
        let fi = stamps.iter().rposition(|s| *s <= ts).unwrap();
        frames[(fi + 1).min(stamps.len() - 1)]["events"].as_array_mut().unwrap().push(e);
    }
    let mut parts: Vec<Value> = (1..=10).map(|p| participant(p, 100 + p)).collect();
    parts[0]["item0"] = json!(1055);
    parts[0]["item1"] = json!(1001);
    parts[5]["item0"] = json!(3133);
    let game = json!({ "metadata": { "matchId": "EUW1_1" }, "info": { "gameVersion": "16.21.712.1234", "queueId": 420, "mapId": 11, "gameDuration": 750,
        "participants": parts, "teams": [{ "teamId": 100, "win": true }, { "teamId": 200, "win": false }] } });
    let timeline = json!({ "metadata": { "matchId": "EUW1_1" }, "info": { "frameInterval": 60000, "frames": frames,
        "participants": (1..=10).map(|p| json!({ "participantId": p, "puuid": format!("p{p}") })).collect::<Vec<_>>() } });
    (game, timeline)
}

fn parse(game: &Value, timeline: &Value) -> Result<Parsed<Game>, String> {
    features::parse_game(&serde_json::to_vec(game).unwrap(), &serde_json::to_vec(timeline).unwrap(), values())
}

fn kept(game: &Value, timeline: &Value) -> Game {
    match parse(game, timeline) {
        Ok(Parsed::Kept(g)) => g,
        Ok(Parsed::Skipped(why)) => panic!("left out: {why}"),
        Err(err) => panic!("{err}"),
    }
}

fn x(g: &Game, minute: usize, key: &str) -> f64 {
    g.states[minute].features()[KEYS.iter().position(|k| *k == key).unwrap()]
}

fn items(pairs: &[(i64, i64)]) -> BTreeMap<i64, i64> {
    pairs.iter().copied().collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn features_by_hand() {
    let (game, timeline) = hand_game(Vec::new());
    let g = kept(&game, &timeline);
    assert_eq!((g.head.mode, g.head.patch.as_str(), g.head.y, g.head.champs), ("rift", "16.21", true, [[101, 102, 103, 104, 105], [106, 107, 108, 109, 110]]));
    assert_eq!(g.minutes, (0..13).map(f64::from).collect::<Vec<_>>(), "one row per minute frame; the game-end frame is left out");
    assert_eq!(g.states[1].item_gold, [450.0, 700.0], "potion counts 0; two Long Swords");
    assert_eq!(g.states[2].item_gold, [450.0, 1050.0], "the undone Caulfield's gave its swords back; bought again");
    assert_eq!(g.states[4].item_gold, [750.0, 1050.0], "Doran's sold, sale undone, Boots bought");
    assert!(close(x(&g, 4, "gold"), (3750.0f64 / 4050.0).ln()));
    assert!(close(x(&g, 4, "level"), 0.2));
    assert!(close(x(&g, 4, "cs"), f64::from(5 * 24 + 16 - 5 * 20) / 100.0));
    assert!(close(x(&g, 4, "kills"), 1.0 / 5f64.sqrt()));
    assert!(close(x(&g, 4, "dead"), 5.0 / 45.0), "level 2 at the 3:00 frame: 10 s, 5 left at 4:00");
    assert_eq!((x(&g, 3, "dead"), x(&g, 5, "dead")), (0.0, 0.0));
    assert_eq!(["towers", "drakes", "inhibs", "grubs", "side"].map(|k| x(&g, 4, k)), [0.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(["towers", "drakes", "inhibs", "grubs"].map(|k| x(&g, 6, k)), [1.0, -1.0, -1.0, 0.0], "an inhibitor falling on the frame counts");
    assert!(close(x(&g, 7, "grubs"), 2.0 / 3.0));
    assert_eq!(x(&g, 10, "inhibs"), -1.0);
    assert_eq!(x(&g, 11, "inhibs"), 0.0, "back 300 s after it fell");
    assert_eq!((x(&g, 12, "kills"), x(&g, 12, "dead")), (1.0 / 5f64.sqrt(), 0.0), "an execution is a death, not a kill");
    assert_eq!(g.final_items[1], items(&[(1055, 1), (1001, 1)]));
    assert_eq!(g.final_items[6], items(&[(3133, 1)]));
    assert!(g.states.iter().all(|s| s.outlook == 0.0 && s.soul == 0 && s.elder == 0 && s.baron == 0));
    assert!(g.odd.is_empty());
}

#[test]
fn buffs_and_soul() {
    let monster = |ts: i64, fields: Value| {
        let mut e = ev(ts, "ELITE_MONSTER_KILL", fields);
        e["killerId"] = json!(2);
        e
    };
    let soul = ev(244000, "DRAGON_SOUL_GIVEN", json!({ "teamId": 100, "name": "Cloud" }));
    let mut more: Vec<Value> = (0..4).map(|i| monster(61000 * (i + 1), json!({ "monsterType": "DRAGON", "monsterSubType": "AIR_DRAGON", "killerTeamId": 100 }))).collect();
    more.push(soul.clone());
    more.push(monster(490000, json!({ "monsterType": "DRAGON", "monsterSubType": "ELDER_DRAGON", "killerTeamId": 200 })));
    more.push(monster(500000, json!({ "monsterType": "BARON_NASHOR", "killerTeamId": 100 })));
    let (game, timeline) = hand_game(more);
    let g = kept(&game, &timeline);
    assert_eq!((x(&g, 3, "drakes"), x(&g, 3, "soul")), (2.0, 0.0));
    assert_eq!((x(&g, 5, "drakes"), x(&g, 5, "soul")), (3.0, 1.0), "four dragons: capped at three, and the soul");
    assert_eq!((x(&g, 9, "elder"), x(&g, 9, "baron"), x(&g, 9, "drakes")), (-1.0, 1.0, 2.0), "an Elder is no dragon stack");
    assert_eq!((x(&g, 11, "elder"), x(&g, 11, "baron")), (0.0, 1.0), "Elder buff 150 s");
    assert_eq!(x(&g, 12, "baron"), 0.0, "Baron buff 180 s");
    let mut red_soul = soul;
    red_soul["teamId"] = json!(200);
    let (game, timeline) = hand_game(vec![red_soul]);
    assert!(parse(&game, &timeline).is_err(), "a soul for a side with fewer than four dragons");
}

#[test]
fn what_real_timelines_do() {
    let atlas = || ev(0, "ITEM_PURCHASED", json!({ "participantId": 0, "itemId": 3865 }));
    let more = vec![
        atlas(),
        atlas(),
        gone(70000, 5, 3865),
        gone(190000, 5, 3866),
        gone(200000, 5, 3867),
        gone(250000, 10, 3865),
        ev(130000, "DRAGON_SOUL_GIVEN", json!({ "teamId": 0, "name": "Cloud" })),
        ev(150000, "ITEM_UNDO", json!({ "participantId": 2, "beforeId": 0, "afterId": 0, "goldGain": 75 })),
        ev(160000, "ITEM_SOLD", json!({ "participantId": 3, "itemId": 3031 })),
        ev(170000, "ITEM_UNDO", json!({ "participantId": 3, "beforeId": 3031, "afterId": 0, "goldGain": 3400 })),
        buy(300000, 4, 3070),
        gone(500000, 4, 3070),
        buy(500000, 4, 3004),
        gone(500000, 4, 3004),
        ev(410000, "ELITE_MONSTER_KILL", json!({ "monsterType": "HORDE", "killerTeamId": 300, "killerId": 0 })),
        ev(410000, "ELITE_MONSTER_KILL", json!({ "monsterType": "RIFTHERALD", "killerTeamId": 300, "killerId": 0 })),
    ];
    let (game, timeline) = hand_game(more);
    let g = kept(&game, &timeline);
    assert_eq!(g.states[1].item_gold, [450.0 + 400.0, 700.0 + 400.0], "each support's World Atlas, bought by participant 0 at 0:00");
    assert_eq!(g.states[2].item_gold, [450.0 + 400.0, 1050.0 + 400.0], "Runic Compass follows it with no event and counts the same");
    assert_eq!(g.final_items[5], items(&[(3869, 1)]), "the upgrade is not logged: the lowest of the five, same cost");
    assert_eq!(g.final_items[10], items(&[(3866, 1)]));
    assert_eq!(g.final_items[4], items(&[(3042, 1)]), "a Manamune bought with a full Tear is a Muramana at once");
    assert_eq!((g.states[12].item_gold[0], g.states[12].soul), (750.0 + 400.0 + 2900.0, 0), "Bounty of Worlds counts 0 in the app, the upgrade 400 again");
    assert_eq!(g.states[12].grubs, [2, 0], "a grub that left the map (team 300) is nobody's");
    let odd: BTreeMap<&str, u32> = [("undo with no item", 1), ("lost an item of value never given", 1), ("undid a purchase never seen", 1)].into_iter().collect();
    assert_eq!(g.odd, odd);
}

#[test]
fn a_role_bound_item_takes_a_purchase_by_nobody() {
    let (mut game, timeline) = hand_game(vec![ev(0, "ITEM_PURCHASED", json!({ "participantId": 0, "itemId": 3865 }))]);
    assert!(parse(&game, &timeline).is_err(), "nobody lost or ended with it");
    game["info"]["participants"][3]["roleBoundItem"] = json!(3865);
    let g = kept(&game, &timeline);
    assert_eq!(g.final_items[4], items(&[(3865, 1)]));
    assert_eq!(g.states[1].item_gold, [450.0 + 400.0, 700.0]);
    game["info"]["participants"][3]["roleBoundItem"] = Value::Null;
    assert!(parse(&game, &timeline).is_err(), "an empty role slot holds nothing");
}

#[test]
fn viego_keeps_his_items_through_a_possession() {
    let (mut game, timeline) = hand_game(vec![
        buy(200000, 3, 3031),
        buy(205000, 3, 1036),
        gone(260000, 3, 3031),
        gone(260000, 3, 1036),
        gone(270000, 3, 3865),
        gone(270000, 3, 1055),
        buy(290000, 3, 3133),
        gone(290000, 3, 1036),
    ]);
    game["info"]["participants"][2]["championId"] = json!(234);
    let g = kept(&game, &timeline);
    assert_eq!(g.final_items[3], items(&[(3031, 1), (3133, 1)]), "only what a purchase of his used up is gone");
    assert!(g.odd.is_empty());
}

#[test]
fn left_out() {
    let (mut game, timeline) = hand_game(Vec::new());
    game["info"]["participants"][4]["gameEndedInEarlySurrender"] = json!(true);
    assert!(matches!(parse(&game, &timeline), Ok(Parsed::Skipped(why)) if why == "remake or early surrender"));
    let (mut game, timeline) = hand_game(Vec::new());
    game["info"]["gameDuration"] = json!(299);
    assert!(matches!(parse(&game, &timeline), Ok(Parsed::Skipped(why)) if why == "shorter than 5 min"));
}

#[test]
fn unknown_shapes_fail_loudly() {
    let bad = [
        json!({ "type": "NEW_EVENT", "timestamp": 5000 }),
        json!({ "timestamp": 5000 }),
        json!({ "type": "CHAMPION_KILL", "timestamp": 5000, "victimId": 3 }),
        json!({ "type": "BUILDING_KILL", "timestamp": 5000, "buildingType": "NEW_BUILDING", "teamId": 100 }),
        json!({ "type": "ELITE_MONSTER_KILL", "timestamp": 5000, "monsterType": "NEW_MONSTER", "killerTeamId": 100 }),
        json!({ "type": "ELITE_MONSTER_KILL", "timestamp": 5000, "monsterType": "BARON_NASHOR", "killerTeamId": 300 }),
        json!({ "type": "ELITE_MONSTER_KILL", "timestamp": 5000, "monsterType": "DRAGON", "monsterSubType": "NEW_DRAGON", "killerTeamId": 100 }),
        json!({ "type": "ITEM_SOLD", "timestamp": 5000, "participantId": 11, "itemId": 3031 }),
        json!({ "type": "ITEM_PURCHASED", "timestamp": 5000, "participantId": 0, "itemId": 3031 }),
        json!({ "type": "ITEM_UNDO", "timestamp": 5000, "participantId": 3, "beforeId": 3031, "afterId": 1001 }),
        json!({ "type": "GAME_END", "timestamp": 5000, "winningTeam": 200 }),
        json!({ "type": "CHAMPION_KILL", "timestamp": 5000.5, "killerId": 1, "victimId": 3 }),
    ];
    for e in bad {
        let (game, timeline) = hand_game(vec![e.clone()]);
        assert!(parse(&game, &timeline).is_err(), "{e}");
    }
    let (game, mut timeline) = hand_game(Vec::new());
    timeline["info"]["frames"][3]["participantFrames"]["4"].as_object_mut().unwrap().remove("minionsKilled");
    assert!(parse(&game, &timeline).is_err());
    let (game, mut timeline) = hand_game(Vec::new());
    timeline["info"]["frames"][3]["participantFrames"].as_object_mut().unwrap().remove("10");
    assert!(parse(&game, &timeline).is_err());
    let (game, mut timeline) = hand_game(Vec::new());
    timeline["info"]["frameInterval"] = json!(30000);
    assert!(parse(&game, &timeline).is_err());
    let (mut game, timeline) = hand_game(Vec::new());
    game["info"]["mapId"] = json!(30);
    assert!(parse(&game, &timeline).is_err());
    let (game, mut timeline) = hand_game(Vec::new());
    timeline["metadata"]["matchId"] = json!("EUW1_2");
    assert!(parse(&game, &timeline).is_err());
}

#[test]
fn respawn_shrunk_win_rates_and_splits() {
    assert_eq!(features::respawn(1, 0.0), 10.0);
    assert!(close(features::respawn(10, 1200.0), 32.5 * (1.0 + 0.00425 * 10.0)));
    assert!(close(features::respawn(18, 3300.0), 52.5 * 1.5));
    assert_eq!(features::respawn(0, 0.0), 10.0);
    let shrink = |wr: f64, games: f64| 50.0 + (wr - 50.0) * games / (games + 400.0);
    let mut mine = [[0; 2]; 7];
    for (i, w) in [(0, 46), (2, 50), (4, 54), (6, 58)] {
        mine[i] = [1000, w * 10];
    }
    assert!(close(features::wr_from(&mine, 14.0).unwrap(), 50.0 + 2.0 * 4000.0 / 4400.0));
    assert!(close(features::wr_from(&mine, 15.0).unwrap(), shrink(100.0 * 1620.0 / 3000.0, 3000.0)), "at 15:00 the 0-15 bucket is behind");
    assert!(close(features::wr_from(&mine, 31.0).unwrap(), shrink(100.0 * 1120.0 / 2000.0, 2000.0)));
    assert!(close(features::wr_from(&mine, 99.0).unwrap(), shrink(58.0, 1000.0)));
    assert_eq!(features::wr_from(&[[0; 2]; 7], 5.0), None);
    let splits: Vec<(bool, u32)> = ["EUW1_0", "EUW1_1", "EUW1_8035444904", "KR_7000000001"].iter().map(|m| (features::is_test(m), features::fold(m))).collect();
    assert_eq!(splits, [(true, 2), (false, 4), (false, 4), (false, 0)], "the same as features.is_test and features.fold");
}

#[test]
fn outlook_from_training_games_without_their_own_result() {
    let a = ([[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]], true, features::bucket(22.0));
    let b = ([[1, 12, 13, 14, 15], [16, 17, 18, 19, 20]], false, features::bucket(33.0));
    let mut t = Tables::new();
    for (champs, y, bucket) in [a, b] {
        features::add_to_tables(&mut t, &champs, y, bucket);
    }
    assert_eq!(t[&1], [[0, 0], [0, 0], [1, 1], [0, 0], [1, 0], [0, 0], [0, 0]], "champion 1: won at 22 min, lost at 33");
    let wr = |games: f64, wins: f64| 50.0 + (100.0 * wins / games - 50.0) * games / (games + 400.0);
    let c = [[1, 30, 31, 32, 33], [6, 34, 35, 36, 37]];
    let bc = features::bucket(30.0);
    assert!(close(features::outlook(&c, true, bc, 10.0, &t, false), (wr(2.0, 1.0) - 50.0) - (wr(1.0, 0.0) - 50.0)));
    assert!(close(features::outlook(&c, true, bc, 31.0, &t, false), wr(1.0, 0.0) - 50.0), "after 30:00 only the 30-35 bucket and later count");
    assert!(close(features::outlook(&a.0, true, a.2, 10.0, &t, true), wr(1.0, 0.0) - 50.0), "A's own win is taken out; champion 1 keeps B's loss");
    assert_eq!(features::outlook(&[[40; 5], [41; 5]], true, bc, 10.0, &t, false), 0.0, "no games, no outlook");
}

#[test]
fn item_values_follow_the_app() {
    let v = values();
    assert_eq!([1055, 1036, 3133, 1001].map(|i| v.gold[&i]), [450, 350, 1050, 300]);
    assert_eq!((v.gold[&2003], v.gold[&2055]), (0, 0), "consumables");
    assert!(!v.gold.contains_key(&3340), "trinkets are not shop items");
    assert_eq!(v.gold[&3042], 2900, "a transform of a shop item counts");
    assert_eq!(v.gold[&3866], 400);
    assert!(!v.gold.contains_key(&3867), "a transform of a transform is not loaded by ddragon::parse_items");
}

fn write_gz(path: &Path, value: &Value) {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::write(path, gz.finish().unwrap()).unwrap();
}

struct Npy {
    header: String,
    data: Vec<u8>,
}

fn read_npz(path: &Path) -> BTreeMap<String, Npy> {
    let bytes = std::fs::read(path).unwrap();
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let (mut out, mut at) = (BTreeMap::new(), 0);
    while u32_at(at) == 0x0403_4b50 {
        assert_eq!(u16_at(at + 8), 8, "deflated");
        let (crc, packed, size, name_len, extra) = (u32_at(at + 14), u32_at(at + 18) as usize, u32_at(at + 22) as usize, u16_at(at + 26), u16_at(at + 28));
        let name = String::from_utf8(bytes[at + 30..at + 30 + name_len].to_vec()).unwrap();
        let start = at + 30 + name_len + extra;
        let mut raw = Vec::new();
        DeflateDecoder::new(&bytes[start..start + packed]).read_to_end(&mut raw).unwrap();
        assert_eq!(raw.len(), size, "{name}");
        let mut sum = flate2::Crc::new();
        sum.update(&raw);
        assert_eq!(sum.sum(), crc, "{name}");
        assert_eq!(&raw[..8], b"\x93NUMPY\x01\x00");
        let header_len = u16::from_le_bytes([raw[8], raw[9]]) as usize;
        assert_eq!((10 + header_len) % 64, 0, "{name} header aligned");
        let header = String::from_utf8(raw[10..10 + header_len].to_vec()).unwrap();
        out.insert(name.trim_end_matches(".npy").to_owned(), Npy { header, data: raw[10 + header_len..].to_vec() });
        at = start + packed;
    }
    assert_eq!(u32_at(at), 0x0201_4b50, "the central directory follows the members");
    out
}

fn texts(npy: &Npy) -> Vec<String> {
    let width: usize = npy.header.split("'<U").nth(1).unwrap().split('\'').next().unwrap().parse().unwrap();
    npy.data.chunks(4 * width).map(|c| c.chunks(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).filter(|&u| u != 0).map(|u| char::from_u32(u).unwrap()).collect()).collect()
}

fn floats(npy: &Npy) -> Vec<f64> {
    npy.data.chunks(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect()
}

#[test]
fn rows_file_from_a_raw_dir() {
    let dir = TempDir::new().unwrap();
    let raw = dir.path().join("raw");
    std::fs::create_dir(&raw).unwrap();
    for (i, early) in [false, false, true].into_iter().enumerate() {
        let (mut game, mut timeline) = hand_game(Vec::new());
        game["metadata"]["matchId"] = json!(format!("EUW1_{i}"));
        timeline["metadata"]["matchId"] = json!(format!("EUW1_{i}"));
        game["info"]["participants"][0]["gameEndedInEarlySurrender"] = json!(early);
        write_gz(&raw.join(format!("EUW1_{i}.match.json.gz")), &game);
        write_gz(&raw.join(format!("EUW1_{i}.timeline.json.gz")), &timeline);
    }
    let (mut other, _) = hand_game(Vec::new());
    other["info"]["gameVersion"] = json!("16.20.1.1");
    write_gz(&raw.join("EUW1_9.match.json.gz"), &other);
    write_gz(&raw.join("EUW1_8.match.json.gz"), &other);
    write_gz(&raw.join("EUW1_8.timeline.json.gz"), &json!({}));
    std::fs::write(dir.path().join("euw1.json"), r#"{"stored": ["EUW1_1"], "bands": {"EUW1_1": "DIAMOND", "EUW1_0": 3}}"#).unwrap();
    std::fs::write(dir.path().join("kr.json"), r#"{"bands": {"EUW1_1": "EMERALD"}}"#).unwrap();
    std::fs::write(dir.path().join("fit.json"), r#"{"fit_games": 5}"#).unwrap();
    std::fs::write(dir.path().join("broken.json"), "{").unwrap();
    let out = dir.path().join("rows.npz");
    let items = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/ddragon-item.json");
    let args = ["--raw", raw.to_str().unwrap(), "--out", out.to_str().unwrap(), "--patch", "16.21", "--items", items.to_str().unwrap(), "--bands", dir.path().to_str().unwrap()];
    assert_eq!(features::run(args.map(str::to_owned)), ExitCode::SUCCESS);
    let r = read_npz(&out);
    assert_eq!(r.keys().map(String::as_str).collect::<Vec<_>>(), ["X", "band", "fold", "game", "keys", "match_ids", "minute", "mode", "patch", "source", "test", "train_games", "y"]);
    assert_eq!(["mode", "patch", "source"].map(|k| texts(&r[k]).concat()), ["rift", "16.21", "riot"]);
    assert!(r["mode"].header.contains("'shape': ()"));
    assert_eq!(texts(&r["keys"]), KEYS);
    assert_eq!(texts(&r["match_ids"]), ["EUW1_0", "EUW1_1"], "the early surrender and the other patch are left out");
    assert_eq!(texts(&r["band"]), ["APEX", "DIAMOND"], "the first state file by name wins; a band that is not text counts as none");
    assert!(r["X"].header.contains("'descr': '<f8'") && r["X"].header.contains("'shape': (26, 14)"), "{}", r["X"].header);
    let x = floats(&r["X"]);
    assert!(x.chunks(14).all(|row| row[13] == 1.0));
    let outlook: Vec<f64> = x.chunks(14).map(|row| row[12]).collect();
    assert!(outlook[..13].iter().all(|o| close(*o, 10.0 * 50.0 / 401.0)), "the held-out game sees the training game's result: {outlook:?}");
    assert!(outlook[13..].iter().all(|o| *o == 0.0), "the training game does not see its own");
    assert_eq!(floats(&r["minute"]), (0..13).chain(0..13).map(f64::from).collect::<Vec<_>>());
    assert_eq!(floats(&r["y"]), vec![1.0; 26]);
    let game: Vec<i64> = r["game"].data.chunks(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect();
    assert_eq!(game, [vec![0; 13], vec![1; 13]].concat());
    assert!(r["test"].header.contains("'|b1'") && r["fold"].header.contains("'|i1'"));
    assert_eq!(r["test"].data, [vec![1u8; 13], vec![0; 13]].concat(), "EUW1_0 is held out");
    assert_eq!(r["fold"].data, [vec![2u8; 13], vec![4; 13]].concat());
    assert_eq!(i64::from_le_bytes(r["train_games"].data[..].try_into().unwrap()), 1);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|n| n.contains(".tmp") || n.contains(".part")).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_bad_game_fails_the_run_and_leaves_no_rows_file() {
    let dir = TempDir::new().unwrap();
    let raw = dir.path().join("raw");
    std::fs::create_dir(&raw).unwrap();
    let (game, timeline) = hand_game(vec![json!({ "type": "NEW_EVENT", "timestamp": 5000 })]);
    write_gz(&raw.join("EUW1_1.match.json.gz"), &game);
    write_gz(&raw.join("EUW1_1.timeline.json.gz"), &timeline);
    let out = dir.path().join("rows.npz");
    let items = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/ddragon-item.json");
    let args = ["--raw", raw.to_str().unwrap(), "--out", out.to_str().unwrap(), "--patch", "16.21", "--items", items.to_str().unwrap()];
    assert_eq!(features::run(args.map(str::to_owned)), ExitCode::FAILURE);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "only raw/ is left");
    assert_eq!(features::run(["--raw".to_owned()]), ExitCode::FAILURE);
    assert!(features::parse_args(["--raw", "a", "--out", "b", "--patch", "16.21"].map(str::to_owned)).is_err(), "--items is needed");
}
