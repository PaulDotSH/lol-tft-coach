use flate2::Compression;
use flate2::write::GzEncoder;
use lol_coach_server::builds::{self, Builder, Damage, Dist, Items, MatchFile, Output, Purchase, Situation, TimelineFile, damage_situation, lead_situation, player_build, purchases, skip_reason};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

const BOOTS: u32 = 1001;
const GREAVES: u32 = 3006;
const LONG_SWORD: u32 = 1036;
const BF_SWORD: u32 = 1038;
const TEAR: u32 = 3070;
const MANAMUNE: u32 = 3004;
const MURAMANA: u32 = 3042;
const EDGE: u32 = 3031;
const KRAKEN: u32 = 6672;
const RAGEBLADE: u32 = 3124;
const STATIKK: u32 = 3087;
const POTION: u32 = 2003;
const DORAN: u32 = 1055;
const MIDWAY: u32 = 9001;
const TOPMOST: u32 = 9002;
const POSITIONS: [&str; 5] = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];

fn item(total: u32, from: &[u32], into: &[u32], tags: &[&str]) -> Value {
    json!({ "gold": { "base": 100, "total": total, "purchasable": true }, "maps": { "11": true, "12": true }, "from": from.iter().map(u32::to_string).collect::<Vec<_>>(), "into": into.iter().map(u32::to_string).collect::<Vec<_>>(), "tags": tags })
}

fn item_file() -> String {
    let mut data = serde_json::Map::new();
    let mut put = |id: u32, v: Value| data.insert(id.to_string(), v);
    put(BOOTS, item(300, &[], &[GREAVES], &["Boots"]));
    put(GREAVES, item(1100, &[BOOTS], &[], &["Boots", "AttackSpeed"]));
    put(LONG_SWORD, item(350, &[], &[EDGE, KRAKEN, MANAMUNE, MIDWAY], &["Damage"]));
    put(BF_SWORD, item(1300, &[], &[EDGE], &["Damage"]));
    put(TEAR, item(400, &[], &[MANAMUNE], &["Mana"]));
    put(MANAMUNE, item(2900, &[TEAR, LONG_SWORD], &[MURAMANA], &["Damage"]));
    put(MURAMANA, json!({ "gold": { "total": 2900, "purchasable": false }, "maps": { "11": true }, "from": [], "tags": ["Damage"] }));
    put(EDGE, item(3500, &[BF_SWORD, LONG_SWORD], &[], &["CriticalStrike"]));
    put(KRAKEN, item(3000, &[LONG_SWORD], &[], &["Damage"]));
    put(RAGEBLADE, item(3000, &[LONG_SWORD], &[], &["Damage"]));
    put(STATIKK, item(2700, &[LONG_SWORD], &[], &["Damage"]));
    put(MIDWAY, item(2200, &[LONG_SWORD], &[TOPMOST], &["Damage"]));
    put(TOPMOST, item(3200, &[MIDWAY], &[], &["Damage"]));
    put(POTION, item(50, &[], &[], &["Consumable"]));
    put(DORAN, item(450, &[], &[], &["Lane"]));
    put(3340, item(0, &[], &[], &["Trinket"]));
    put(4005, json!({ "gold": { "total": 2500, "purchasable": true }, "maps": { "11": false, "12": true }, "from": ["1036"], "tags": [] }));
    put(6693, json!({ "gold": { "total": 3000, "purchasable": true }, "maps": { "11": true }, "from": ["1036"], "tags": [], "requiredAlly": "Ornn" }));
    json!({ "type": "item", "data": data }).to_string()
}

fn items() -> Items {
    Items::parse(item_file().as_bytes()).unwrap()
}

struct P {
    champ: u32,
    magic: u64,
    physical: u64,
    gold: i64,
    buys: Vec<(u32, u64)>,
}

fn p(champ: u32, buys: &[(u32, u64)]) -> P {
    P { champ, magic: 100, physical: 100, gold: 400, buys: buys.to_vec() }
}

fn game_json(id: &str, secs: u64, players: &[P], undos: &[(u32, u32, u64)]) -> (Value, Value) {
    let parts: Vec<Value> = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({ "participantId": i + 1, "teamId": if i < 5 { 100 } else { 200 }, "championId": p.champ, "teamPosition": POSITIONS[i % 5], "gameEndedInEarlySurrender": false, "teamEarlySurrendered": false,
                    "magicDamageDealtToChampions": p.magic, "physicalDamageDealtToChampions": p.physical, "item0": 0 })
        })
        .collect();
    let game = json!({ "metadata": { "matchId": id }, "info": { "gameVersion": "16.22.123.4567", "mapId": 11, "queueId": 420, "gameDuration": secs, "participants": parts } });
    let minutes = secs / 60 + 1;
    let mut frames: Vec<Value> = (0..=minutes)
        .map(|m| {
            let pf: serde_json::Map<String, Value> = players.iter().enumerate().map(|(i, p)| ((i + 1).to_string(), json!({ "totalGold": p.gold * m as i64, "currentGold": 500 + m as i64, "level": 1 }))).collect();
            json!({ "timestamp": m * 60_000, "participantFrames": pf, "events": [] })
        })
        .collect();
    let mut push = |ms: u64, ev: Value| frames[(ms / 60_000 + 1).min(minutes) as usize]["events"].as_array_mut().unwrap().push(ev);
    for (i, p) in players.iter().enumerate() {
        for &(item, s) in &p.buys {
            push(s * 1000, json!({ "type": "ITEM_PURCHASED", "timestamp": s * 1000, "participantId": i + 1, "itemId": item }));
        }
    }
    for &(pid, item, s) in undos {
        push(s * 1000, json!({ "type": "ITEM_UNDO", "timestamp": s * 1000, "participantId": pid, "beforeId": item, "afterId": 0, "goldGain": 100 }));
    }
    push(1000, json!({ "type": "SKILL_LEVEL_UP", "timestamp": 1000, "participantId": 1, "skillSlot": 1 }));
    let timeline = json!({ "metadata": { "matchId": id }, "info": { "frameInterval": 60000, "frames": frames } });
    (game, timeline)
}

fn parsed(game: &Value, timeline: &Value) -> (MatchFile, TimelineFile) {
    (serde_json::from_value(game.clone()).unwrap(), serde_json::from_value(timeline.clone()).unwrap())
}

fn buys_of(timeline: &TimelineFile, pid: u32) -> Vec<(u32, u64)> {
    purchases(timeline, pid).iter().map(|p| (p.item, p.ms / 1000)).collect()
}

fn filler(champ: u32) -> P {
    p(champ, &[])
}

fn ten(first: P, enemy_champ: u32) -> Vec<P> {
    let mut v = vec![first];
    v.extend((2..=5).map(filler));
    v.extend((0..5).map(|_| filler(enemy_champ)));
    v
}

#[test]
fn finished_items_are_rift_items_of_2000_gold_built_from_components_that_build_into_nothing() {
    let items = items();
    for id in [MANAMUNE, EDGE, KRAKEN, RAGEBLADE, STATIKK, TOPMOST] {
        assert!(items.finished.contains(&id), "{id}");
    }
    for id in [MURAMANA, TEAR, LONG_SWORD, BF_SWORD, MIDWAY, BOOTS, GREAVES, POTION, DORAN, 3340, 4005, 6693] {
        assert!(!items.finished.contains(&id), "{id}");
    }
    assert_eq!(items.boots.iter().copied().collect::<Vec<_>>(), [GREAVES]);
    assert!(items.counted.contains(&LONG_SWORD) && !items.counted.contains(&POTION));
}

#[test]
fn an_undone_purchase_does_not_count_and_an_item_counts_at_its_first_purchase() {
    let mut players = ten(p(1, &[(DORAN, 0), (EDGE, 600), (KRAKEN, 620), (MANAMUNE, 900), (EDGE, 1000), (GREAVES, 700)]), 50);
    players[1].buys = vec![(EDGE, 650)];
    let (g, t) = game_json("EUW1_1", 1800, &players, &[(1, EDGE, 610), (2, KRAKEN, 655)]);
    let (_, t) = parsed(&g, &t);
    assert_eq!(buys_of(&t, 1), [(DORAN, 0), (KRAKEN, 620), (GREAVES, 700), (MANAMUNE, 900), (EDGE, 1000)]);
    assert_eq!(buys_of(&t, 2), [(EDGE, 650)], "an undo of an item never bought removes nothing");
    let build = player_build(&items(), &purchases(&t, 1));
    assert_eq!(build.finished, [Purchase { item: KRAKEN, ms: 620_000 }, Purchase { item: MANAMUNE, ms: 900_000 }, Purchase { item: EDGE, ms: 1_000_000 }], "Manamune counts as bought, Muramana never shows");
    assert_eq!(build.boots, Some(700_000));
}

#[test]
fn the_first_back_is_the_first_purchase_from_two_minutes_with_what_was_bought_within_30_seconds() {
    let buys = [(DORAN, 0), (POTION, 0), (LONG_SWORD, 119), (LONG_SWORD, 300), (POTION, 310), (BF_SWORD, 330), (TEAR, 331)];
    let (g, t) = game_json("EUW1_1", 1800, &ten(p(1, &buys), 50), &[]);
    let (_, t) = parsed(&g, &t);
    let build = player_build(&items(), &purchases(&t, 1));
    assert_eq!(build.back, Some((300_000, vec![LONG_SWORD, BF_SWORD])), "potions are not counted, and 31 s later is another visit");
    let none = player_build(&items(), &[Purchase { item: DORAN, ms: 0 }]);
    assert_eq!((none.back, none.finished.len(), none.boots), (None, 0, None));
}

#[test]
fn short_games_remakes_other_patches_and_other_maps_are_left_out() {
    let (g, _) = game_json("EUW1_1", 899, &ten(filler(1), 50), &[]);
    assert_eq!(skip_reason(&serde_json::from_value(g.clone()).unwrap(), "16.22"), Some("shorter than 15 min"));
    let mut long = g.clone();
    long["info"]["gameDuration"] = json!(900);
    assert_eq!(skip_reason(&serde_json::from_value(long.clone()).unwrap(), "16.22"), None);
    assert_eq!(skip_reason(&serde_json::from_value(long.clone()).unwrap(), "16.21"), Some("other patch"));
    let mut remake = long.clone();
    remake["info"]["participants"][7]["teamEarlySurrendered"] = json!(true);
    assert_eq!(skip_reason(&serde_json::from_value(remake).unwrap(), "16.22"), Some("remake or early surrender"));
    let mut aram = long;
    aram["info"]["mapId"] = json!(12);
    assert_eq!(skip_reason(&serde_json::from_value(aram).unwrap(), "16.22"), Some("not Summoner's Rift"));
}

#[test]
fn percentiles_interpolate_between_the_values_on_small_counts() {
    let dist = |values: &[u32]| {
        let mut d = Dist::default();
        values.iter().for_each(|v| d.add(*v));
        d.percentiles()
    };
    assert_eq!(dist(&[]), None);
    assert_eq!(dist(&[600]), Some([600, 600, 600]));
    assert_eq!(dist(&[10, 20, 30, 40]), Some([18, 25, 33]));
    assert_eq!(dist(&[40, 10, 10, 10, 10]), Some([10, 10, 10]));
    assert_eq!(dist(&[100, 200]), Some([125, 150, 175]));
}

#[test]
fn situations_and_leads_have_fixed_cuts() {
    assert_eq!([damage_situation(0.1, [0.3, 0.5]), damage_situation(0.3, [0.3, 0.5]), damage_situation(0.5, [0.3, 0.5])], [Situation::Physical, Situation::Mixed, Situation::Magic]);
    assert_eq!([lead_situation(900, 1000), lead_situation(901, 1000), lead_situation(1100, 1000), lead_situation(1099, 1000)], [Situation::Behind, Situation::Even, Situation::Ahead, Situation::Even]);
}

#[test]
fn magic_shares_are_over_all_games_and_the_cuts_are_thirds_of_team_values() {
    let mut damage = Damage::default();
    for (n, magic) in [(1u32, 0u64), (2, 50), (3, 100)].into_iter() {
        let mut players = ten(filler(10 + n), 20 + n);
        for pl in &mut players {
            (pl.magic, pl.physical) = (magic, 100 - magic);
        }
        let (g, _) = game_json(&format!("EUW1_{n}"), 1800, &players, &[]);
        damage.add(&serde_json::from_value(g).unwrap());
    }
    let (shares, cuts) = damage.finish();
    assert_eq!((shares[&11], shares[&2], shares[&22], shares[&23]), (0.0, 0.5, 0.5, 1.0));
    assert_eq!(cuts, [0.4667, 0.5333]);
}

fn run_builder(games: impl IntoIterator<Item = (Vec<P>, Option<&'static str>)>, shares: BTreeMap<u32, f64>) -> Output {
    let mut b = Builder::new(items(), shares, [0.33, 0.66]);
    for (n, (players, band)) in games.into_iter().enumerate() {
        let (g, t) = game_json(&format!("EUW1_{n}"), 1800, &players, &[]);
        let (m, t) = parsed(&g, &t);
        b.add(&m, &t, band).unwrap();
    }
    b.finish("16.22")
}

fn pair<'a>(out: &'a Output, champ: u32, role: &str) -> Option<&'a builds::Pair> {
    out.pairs.iter().find(|p| p.champion == champ && p.role == role)
}

#[test]
fn a_pair_needs_100_games_and_master_plus_its_own_100() {
    let game = |band| (ten(p(1, &[(KRAKEN, 600)]), 50), band);
    let out = run_builder((0..99).map(|_| game(None)), BTreeMap::new());
    assert!(out.pairs.is_empty());
    let out = run_builder((0..150).map(|n| game(if n < 99 { None } else { Some("EMERALD") })), BTreeMap::new());
    let jinx = pair(&out, 1, "TOP").unwrap();
    assert_eq!((jinx.all.games, jinx.top.is_none()), (150, true));
    assert_eq!(out.bands, BTreeMap::from([("EMERALD".to_owned(), 51), ("top".to_owned(), 99)]));
    let out = run_builder((0..200).map(|n| game(if n < 100 { None } else { Some("DIAMOND") })), BTreeMap::new());
    assert_eq!(pair(&out, 1, "TOP").unwrap().top.as_ref().unwrap().games, 100);
    assert!(pair(&out, 2, "JUNGLE").unwrap().all.first.is_empty(), "a pair whose players finish nothing is listed with empty lists");
}

#[test]
fn first_items_from_5_percent_and_next_items_from_10_percent() {
    let games = (0..200u64).map(|n| {
        let first = match n {
            0..189 => KRAKEN,
            189..199 => STATIKK,
            _ => RAGEBLADE,
        };
        let second = if n < 19 { RAGEBLADE } else if n < 37 { EDGE } else { MANAMUNE };
        (ten(p(1, &[(first, 600 + n), (second, 1000 + n), (GREAVES, 400)]), 50), None)
    });
    let out = run_builder(games, BTreeMap::new());
    let table = &pair(&out, 1, "TOP").unwrap().all;
    assert_eq!(table.first.iter().map(|p| (p.item, p.share, p.games)).collect::<Vec<_>>(), [(KRAKEN, 0.945, 189), (STATIKK, 0.05, 10)]);
    assert_eq!(table.first[0].time, Some([647, 694, 741]));
    assert_eq!(table.slots.first, Some([650, 700, 749]));
    assert_eq!(table.slots.boots, Some([400, 400, 400]));
    assert_eq!(table.slots.third, None);
    let after = &table.second[&KRAKEN];
    assert_eq!(after.iter().map(|p| (p.item, p.games)).collect::<Vec<_>>(), [(MANAMUNE, 152), (RAGEBLADE, 19)], "Edge has 18 of 189, under 10 %");
    assert_eq!(table.second[&STATIKK].iter().map(|p| p.item).collect::<Vec<_>>(), [MANAMUNE]);
    assert!(!table.second.contains_key(&RAGEBLADE), "only listed starts");
    let back = table.back.as_ref().unwrap();
    assert_eq!((back.games, back.time, back.gold), (200, [400, 400, 400], [506, 506, 506]));
    assert_eq!(back.items.iter().map(|p| (p.item, p.share)).collect::<Vec<_>>(), [(GREAVES, 1.0)]);
}

#[test]
fn a_situation_is_listed_when_its_most_taken_item_differs_and_it_has_150_games() {
    let shares = BTreeMap::from([(50, 1.0), (60, 0.0)]);
    let games = (0..200u64).map(|n| {
        let (second, enemy) = match n {
            0..96 => (RAGEBLADE, 50),
            96..160 => (EDGE, 50),
            _ => (EDGE, 60),
        };
        let mut players = ten(p(1, &[(KRAKEN, 600), (second, 1000)]), enemy);
        players[0].gold = if n % 2 == 0 { 300 } else { 500 };
        (players, None)
    });
    let out = run_builder(games, shares);
    let table = &pair(&out, 1, "TOP").unwrap().all;
    assert_eq!(table.second[&KRAKEN][0].item, EDGE);
    let listed: Vec<(u8, u32, &str, &str, u32, f64, u32)> = table.situations.iter().map(|s| (s.slot, s.start, s.kind.as_str(), s.value.as_str(), s.item, s.share, s.games)).collect();
    assert_eq!(listed, [(2, KRAKEN, "damage", "magic", RAGEBLADE, 0.6, 160)], "the physical games are too few and the leads follow the overall item");
}

#[test]
fn the_lead_is_against_the_enemy_in_the_same_position_in_the_frame_before_the_purchase() {
    let shares = BTreeMap::new();
    let games = (0..400u64).map(|n| {
        let mut players = ten(p(1, &[(KRAKEN, 600), (if n < 120 { RAGEBLADE } else { EDGE }, 1000)]), 50);
        players[0].gold = if n < 200 { 300 } else { 400 };
        players[1].gold = 400;
        players[5].gold = 400;
        (players, None)
    });
    let out = run_builder(games, shares);
    let table = &pair(&out, 1, "TOP").unwrap().all;
    assert_eq!(table.second[&KRAKEN][0].item, EDGE);
    let listed: Vec<(&str, &str, u32, f64, u32)> = table.situations.iter().map(|s| (s.kind.as_str(), s.value.as_str(), s.item, s.share, s.games)).collect();
    assert_eq!(listed, [("lead", "behind", RAGEBLADE, 0.6, 200)]);
}

fn gz(path: &Path, text: &str) {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(text.as_bytes()).unwrap();
    std::fs::write(path, e.finish().unwrap()).unwrap();
}

#[test]
fn the_subcommand_streams_the_raw_games_and_writes_the_table() {
    let dir = TempDir::new().unwrap();
    let (patch, raw) = (dir.path().join("16.22"), dir.path().join("16.22").join("raw"));
    std::fs::create_dir_all(&raw).unwrap();
    let mut bands = serde_json::Map::new();
    for n in 0..130u64 {
        let id = format!("EUW1_{n}");
        let mut players = ten(p(1, &[(DORAN, 0), (LONG_SWORD, 200), (KRAKEN, 600 + n)]), 50);
        players[5].magic = 300;
        let (g, t) = game_json(&id, if n == 0 { 600 } else { 1800 }, &players, &[]);
        gz(&raw.join(format!("{id}.match.json.gz")), &g.to_string());
        gz(&raw.join(format!("{id}.timeline.json.gz")), &t.to_string());
        if n >= 100 {
            bands.insert(id, json!("EMERALD"));
        }
    }
    std::fs::write(raw.join("EUW1_999.match.json.gz"), "no timeline").unwrap();
    std::fs::write(patch.join("euw1.json"), json!({ "stored": [], "bands": bands }).to_string()).unwrap();
    std::fs::write(patch.join("fit.json"), json!({ "bands": { "EUW1_5": "DIAMOND" } }).to_string()).unwrap();
    let items = dir.path().join("items-16.22.1.json");
    std::fs::write(&items, item_file()).unwrap();
    let out = patch.join("builds.json");
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let args: Vec<String> = ["--raw", &text(&raw), "--bands", &text(&patch), "--items", &text(&items), "--patch", "16.22", "--out", &text(&out)].iter().map(|s| (*s).to_owned()).collect();
    let summary = builds::run(&builds::parse_args(&args).unwrap()).unwrap();
    assert!(summary.contains("shorter than 15 min"), "{summary}");
    let written: Output = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!((written.patch.as_str(), written.games), ("16.22", 129));
    assert_eq!(written.bands, BTreeMap::from([("EMERALD".to_owned(), 30), ("top".to_owned(), 99)]));
    assert_eq!(written.lead_cut, 0.1);
    assert!(written.finished.contains(&KRAKEN) && written.boots == [GREAVES]);
    assert_eq!(written.magic_share[&50], 0.5833);
    let jinx = pair(&written, 1, "TOP").unwrap();
    assert_eq!((jinx.all.games, jinx.top.is_none()), (129, true));
    assert_eq!(jinx.all.back.as_ref().unwrap().items[0].item, LONG_SWORD);
    assert!(builds::parse_args(&["--raw".to_owned()]).is_err());
    assert!(builds::parse_args(&["--what".to_owned(), "x".to_owned()]).is_err());
}

#[test]
#[ignore = "reads the games crawled on this PC; run with --ignored"]
fn a_run_over_the_real_games() {
    let base = std::env::var_os("LOCALAPPDATA").map(|d| Path::new(&d).join("lol-coach").join("dev").join("winprob").join("16.22")).unwrap();
    let out = std::env::temp_dir().join("lol-coach-builds-16.22.json");
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let args: Vec<String> = ["--raw", &text(&base.join("raw")), "--items", &text(&base.join("item-16.22.1.json")), "--patch", "16.22", "--out", &text(&out)].iter().map(|s| (*s).to_owned()).collect();
    let begun = std::time::Instant::now();
    let summary = builds::run(&builds::parse_args(&args).unwrap()).unwrap();
    println!("{summary} in {:?}", begun.elapsed());
    let size = std::fs::metadata(&out).unwrap().len();
    assert!(size < 1_000_000, "{size} bytes");
    let written: Output = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert!(written.games > 15_000 && written.pairs.len() > 150, "{} {}", written.games, written.pairs.len());
}
