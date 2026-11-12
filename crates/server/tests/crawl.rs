mod common;

use common::{FakeDdragon, FakeRiot, ITEM_JSON, TestApp, fake_ddragon, fake_riot};
use flate2::read::GzDecoder;
use lol_coach_server::config::CrawlConfig;
use lol_coach_server::crawl::{self, Command, Crawl, Env, Runner, Status, due, ladder_order, patch_of_version};
use serde_json::{Value, json};
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

const OTHER_KEY_BODY: &str = r#"{"status":{"message":"Bad Request - Exception decrypting abc","status_code":400}}"#;

struct Rig {
    fake: FakeRiot,
    dd: FakeDdragon,
    dir: TempDir,
    commands: Arc<Mutex<Vec<Command>>>,
    fit_code: Arc<AtomicI32>,
}

fn id(n: u32) -> String {
    format!("EUW1_{n:03}")
}

fn match_body(id: &str, version: &str) -> String {
    format!("{{\n  \"metadata\" : {{\"matchId\":\"{id}\"}},\"info\":{{\"gameVersion\":\"{version}\",\"pad\":[1,  2.50,1e3, \"\u{e9}\"]}}\n}}\n")
}

fn timeline_body(id: &str) -> String {
    format!("{{\"metadata\":{{\"matchId\":\"{id}\"}},\"info\":{{\"frames\":[ ]}}}}")
}

fn read_gz(path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_end(&mut out).unwrap();
    out
}

async fn wait(status: &Status, what: &str, ok: impl Fn(&Value) -> bool) {
    for _ in 0..6000 {
        if ok(&status.snapshot()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("never {what}: {}", status.snapshot());
}

fn state_is(i: usize, prefix: &'static str) -> impl Fn(&Value) -> bool {
    move |v| v["servers"][i]["state"].as_str().is_some_and(|s| s.starts_with(prefix))
}

async fn settle() {
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

impl Rig {
    async fn new(versions: &[&str]) -> Self {
        let fake = fake_riot().await;
        fake.set(|m| m.rate_limit = Some("100000:1,1000000:120".to_owned()));
        Self { fake, dd: fake_ddragon(versions).await, dir: TempDir::new().unwrap(), commands: Arc::default(), fit_code: Arc::default() }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn runner(&self) -> Runner {
        let (commands, code) = (self.commands.clone(), self.fit_code.clone());
        Arc::new(move |command: Command| {
            commands.lock().unwrap().push(command.clone());
            let code = if command.iter().any(|a| a.ends_with("fit.py")) { code.load(Ordering::SeqCst) } else { 0 };
            Box::pin(async move { code }) as Pin<Box<dyn Future<Output = i32> + Send>>
        })
    }

    fn env(&self, servers: &[&str], max_games: u64, fit_at: u64, refit_every: u64) -> Env {
        let config = CrawlConfig { servers: servers.iter().map(|s| (*s).to_owned()).collect(), max_games, fit_at, refit_every, python: "py".to_owned(), tools_dir: PathBuf::from("/tools") };
        Env { config, data_dir: self.root().to_owned(), riot: self.fake.hosted_riot(), ddragon: self.dd.base.clone(), runner: self.runner() }
    }

    fn start(&self, servers: &[&str], max_games: u64) -> Crawl {
        crawl::start(self.env(servers, max_games, 1_000_000, 1_000_000))
    }

    fn apex(&self, host: &str, tier: &str, puuids: &[&str]) {
        let entries: Vec<Value> = puuids.iter().map(|p| json!({ "puuid": p, "leaguePoints": 100 })).collect();
        self.fake.route(&format!("/{host}/lol/league/v4/{}leagues/by-queue/RANKED_SOLO_5x5", tier.to_lowercase()), 200, json!({ "tier": tier, "entries": entries }).to_string());
    }

    fn page(&self, host: &str, division: &str, page: u32, puuids: &[&str]) {
        let (tier, rank) = division.split_once(' ').unwrap();
        let entries: Vec<Value> = puuids.iter().map(|p| json!({ "puuid": p })).collect();
        self.fake.route(&format!("/{host}/lol/league/v4/entries/RANKED_SOLO_5x5/{tier}/{rank}?page={page}"), 200, Value::Array(entries).to_string());
    }

    fn player(&self, region: &str, puuid: &str, ids: &[String]) {
        self.fake.route(&format!("/{region}/lol/match/v5/matches/by-puuid/{puuid}/ids*"), 200, json!(ids).to_string());
    }

    fn game(&self, region: &str, id: &str, version: &str) -> (String, String) {
        let (game, timeline) = (match_body(id, version), timeline_body(id));
        self.fake.route(&format!("/{region}/lol/match/v5/matches/{id}"), 200, game.clone());
        self.fake.route(&format!("/{region}/lol/match/v5/matches/{id}/timeline"), 200, timeline.clone());
        (game, timeline)
    }

    fn match_path(region: &str, id: &str) -> String {
        format!("/{region}/lol/match/v5/matches/{id}")
    }

    fn patch_dir(&self, patch: &str) -> PathBuf {
        self.root().join("winprob").join(patch)
    }

    fn state(&self, patch: &str, platform: &str) -> Value {
        serde_json::from_slice(&std::fs::read(self.patch_dir(patch).join(format!("{platform}.json"))).unwrap()).unwrap()
    }

    fn raw_names(&self, patch: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.patch_dir(patch).join("raw")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }

    fn one_player_world(&self, ids: &[(u32, &str)]) {
        self.apex("euw1", "CHALLENGER", &["a"]);
        self.player("europe", "a", &ids.iter().map(|(n, _)| id(*n)).collect::<Vec<_>>());
        for (n, version) in ids {
            self.game("europe", &id(*n), version);
        }
    }
}

#[test]
fn a_fit_is_due_at_fit_at_and_then_every_refit_every() {
    let is_due = |games, last| due(games, last, 10_000, 2_500);
    assert_eq!([is_due(9_999, None), is_due(10_000, None), is_due(12_499, Some(10_000)), is_due(12_500, Some(10_000))], [false, true, false, true]);
}

#[test]
fn patches_come_from_versions_and_the_ladder_has_a_fixed_order() {
    assert_eq!(patch_of_version("16.22.1").as_deref(), Some("16.22"));
    assert_eq!(patch_of_version("16.22.123.4567").as_deref(), Some("16.22"));
    assert_eq!([patch_of_version("lolpatch_3.7"), patch_of_version("16"), patch_of_version("16.x.1")], [None, None, None]);
    let order = ladder_order();
    assert_eq!(order.len(), 8);
    assert_eq!(&order[..4], ["DIAMOND I", "EMERALD I", "DIAMOND II", "EMERALD II"]);
}

#[tokio::test]
async fn stored_files_hold_the_exact_bytes_riot_sent() {
    let rig = Rig::new(&["16.22.1", "16.21.1"]).await;
    rig.apex("euw1", "CHALLENGER", &["a"]);
    rig.player("europe", "a", &[id(1)]);
    let (game, timeline) = rig.game("europe", &id(1), "16.22.1.9");
    let mut crawl = rig.start(&["euw1/europe"], 1);
    wait(&crawl.status, "done", state_is(0, "done")).await;
    crawl.stop().await;
    let raw = rig.patch_dir("16.22").join("raw");
    assert_eq!(read_gz(&raw.join("EUW1_001.match.json.gz")), game.as_bytes());
    assert_eq!(read_gz(&raw.join("EUW1_001.timeline.json.gz")), timeline.as_bytes());
    assert_eq!(rig.raw_names("16.22"), ["EUW1_001.match.json.gz", "EUW1_001.timeline.json.gz"], "no temporary file stays");
    let state = rig.state("16.22", "euw1");
    assert_eq!(state["stored"], json!(["EUW1_001"]));
    assert_eq!(state["seen"], json!(["EUW1_001"]));
    assert_eq!(state["ladder"]["puuids"], json!(["a"]));
    assert_eq!(rig.fake.state.tokens.lock().unwrap().iter().filter(|t| t.as_str() != common::RIOT_KEY).count(), 0);
    assert_eq!(rig.dd.hits_with("/api/versions.json"), 1);
}

#[tokio::test]
async fn it_resumes_without_fetching_a_game_twice() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.apex("euw1", "CHALLENGER", &["a"]);
    rig.apex("euw1", "MASTER", &["b", "a"]);
    rig.page("euw1", "DIAMOND I", 1, &["c"]);
    rig.player("europe", "a", &[id(1), id(2)]);
    rig.player("europe", "b", &[id(2), id(3)]);
    rig.player("europe", "c", &[id(4), id(5)]);
    for n in 1..=5 {
        rig.game("europe", &id(n), if n == 2 { "16.20.1.2" } else { "16.21.1.2" });
    }
    let mut crawl = rig.start(&["euw1/europe"], 2);
    wait(&crawl.status, "done", state_is(0, "done")).await;
    crawl.stop().await;
    assert_eq!(rig.state("16.21", "euw1")["stored"], json!([id(1), id(3)]));
    assert_eq!(rig.raw_names("16.21").len(), 4);
    assert_eq!(rig.fake.hits_exact(&format!("{}/timeline", Rig::match_path("europe", &id(2)))), 0, "a match of another patch costs no timeline");

    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    let state = rig.state("16.21", "euw1");
    assert_eq!(state["stored"], json!([id(1), id(3), id(4), id(5)]));
    assert_eq!(state["ladder"]["puuids"], json!(["a", "b", "c"]));
    for n in 1..=5 {
        let path = Rig::match_path("europe", &id(n));
        assert_eq!(rig.fake.hits_exact(&path), 1, "{path}");
        assert_eq!(rig.fake.hits_exact(&format!("{path}/timeline")), usize::from(n != 2), "{path}");
    }
    assert_eq!(read_gz(&rig.patch_dir("16.21").join("raw").join("EUW1_004.timeline.json.gz")), timeline_body("EUW1_004").as_bytes());
}

#[tokio::test]
async fn an_older_patch_ends_a_players_list_and_a_newer_one_is_passed_over() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.22.1.2"), (2, "16.21.1.2"), (3, "16.20.1.2"), (4, "16.20.1.2"), (5, "16.21.1.2")]);
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    let state = rig.state("16.21", "euw1");
    assert_eq!(state["stored"], json!([id(2)]), "ids come newest first: nothing follows an older patch");
    assert_eq!(state["seen"], json!([id(1), id(2), id(3)]));
    let hits = |n| rig.fake.hits_exact(&Rig::match_path("europe", &id(n)));
    assert_eq!([hits(1), hits(2), hits(3), hits(4), hits(5)], [1, 1, 1, 0, 0]);
    assert_eq!(rig.fake.hits_exact(&format!("{}/timeline", Rig::match_path("europe", &id(1)))), 0);
    assert_eq!(rig.fake.hits_exact(&format!("{}/timeline", Rig::match_path("europe", &id(3)))), 0);
    assert_eq!(state["ladder"]["next_puuid"], 1);
}

#[tokio::test]
async fn a_missing_match_or_timeline_is_remembered_and_skipped() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.21.1.2"), (2, "16.21.1.2"), (3, "16.21.1.2")]);
    rig.fake.route(&Rig::match_path("europe", &id(1)), 404, "{}");
    rig.fake.route(&format!("{}/timeline", Rig::match_path("europe", &id(2))), 404, "{}");
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    let state = rig.state("16.21", "euw1");
    assert_eq!(state["stored"], json!([id(3)]));
    assert_eq!(state["seen"], json!([id(1), id(2), id(3)]));
    assert_eq!(rig.raw_names("16.21"), ["EUW1_003.match.json.gz", "EUW1_003.timeline.json.gz"]);
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    assert_eq!(rig.fake.hits_exact(&Rig::match_path("europe", &id(1))), 1);
    assert_eq!(rig.fake.hits_exact(&format!("{}/timeline", Rig::match_path("europe", &id(2)))), 1);
}

#[tokio::test]
async fn the_ladder_is_read_top_first_then_divisions_in_turn_until_each_is_used_up() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.page("euw1", "DIAMOND I", 1, &["p1"]);
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    let league: Vec<String> = rig.fake.hit_list().into_iter().filter(|h| h.contains("/league/")).collect();
    let entries = |division: &str, page: u32| {
        let (tier, rank) = division.split_once(' ').unwrap();
        format!("/euw1/lol/league/v4/entries/RANKED_SOLO_5x5/{tier}/{rank}?page={page}")
    };
    let mut want: Vec<String> = ["challenger", "grandmaster", "master"].iter().map(|t| format!("/euw1/lol/league/v4/{t}leagues/by-queue/RANKED_SOLO_5x5")).collect();
    want.extend(ladder_order().iter().map(|d| entries(d, 1)));
    want.push(entries("DIAMOND I", 2));
    assert_eq!(league, want, "a division with an empty page is not asked again");
    assert_eq!(rig.state("16.21", "euw1")["ladder"]["exhausted"].as_array().unwrap().len(), 8);
}

fn seed_other_key_state(rig: &Rig) {
    let dir = rig.patch_dir("16.21");
    std::fs::create_dir_all(&dir).unwrap();
    let state = json!({
        "ladder": { "apex_done": ["CHALLENGER", "GRANDMASTER", "MASTER"], "pages": { "DIAMOND I": 9 }, "exhausted": ["DIAMOND I"], "turn": 3, "puuids": ["gone", "old"], "next_puuid": 1 },
        "seen": [id(1)],
        "stored": [id(1)],
    });
    std::fs::write(dir.join("euw1.json"), state.to_string()).unwrap();
    rig.fake.route("/europe/lol/match/v5/matches/by-puuid/old/ids*", 400, OTHER_KEY_BODY);
    rig.page("euw1", "DIAMOND I", 1, &["new"]);
}

#[tokio::test]
async fn players_of_another_key_are_dropped_once_and_the_ladder_is_read_again() {
    let rig = Rig::new(&["16.21.1"]).await;
    seed_other_key_state(&rig);
    rig.player("europe", "new", &[id(1), id(2)]);
    rig.game("europe", &id(2), "16.21.1.2");
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    crawl.stop().await;
    let state = rig.state("16.21", "euw1");
    assert_eq!(state["ladder"]["puuids"], json!(["new"]));
    assert_eq!(state["stored"], json!([id(1), id(2)]), "the games on disk are kept");
    assert_eq!(rig.fake.hits_exact(&Rig::match_path("europe", &id(1))), 0, "a game already seen is not fetched again");
    assert_eq!(rig.fake.hits_with("/euw1/lol/league/v4/challengerleagues"), 1, "the ladder is read again, from the top");
    assert_eq!(rig.fake.hits_with("/europe/lol/match/v5/matches/by-puuid/old/"), 1);
}

#[tokio::test]
async fn a_second_rejection_right_after_stops_that_server() {
    let rig = Rig::new(&["16.21.1"]).await;
    seed_other_key_state(&rig);
    rig.fake.route("/europe/lol/match/v5/matches/by-puuid/new/ids*", 400, OTHER_KEY_BODY);
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "stopped", state_is(0, "stopped")).await;
    crawl.stop().await;
    assert_eq!(rig.fake.hits_with("/europe/lol/match/v5/matches/by-puuid/new/"), 1);
    assert_eq!(rig.fake.hits_with("/euw1/lol/league/v4/challengerleagues"), 1, "the ladder is read again only once");
    assert_eq!(rig.state("16.21", "euw1")["stored"], json!([id(1)]));
}

#[tokio::test(start_paused = true)]
async fn a_rejected_key_waits_ten_minutes_and_goes_on() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.21.1.2")]);
    rig.fake.set(|m| m.fail = Some(403));
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "key rejected", state_is(0, "waiting: key rejected")).await;
    let hits = rig.fake.total();
    tokio::time::advance(Duration::from_secs(590)).await;
    settle().await;
    assert_eq!(rig.fake.total(), hits, "nothing is sent for ten minutes");
    rig.fake.set(|m| m.fail = None);
    tokio::time::advance(Duration::from_secs(20)).await;
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    assert_eq!(crawl.status.snapshot()["servers"][0]["stored"], 1);
    crawl.stop().await;
}

#[tokio::test(start_paused = true)]
async fn riot_being_unavailable_is_waited_out_without_spinning() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.21.1.2")]);
    rig.fake.set(|m| m.fail = Some(500));
    let mut crawl = rig.start(&["euw1/europe"], 10);
    wait(&crawl.status, "unavailable", state_is(0, "waiting: riot unavailable")).await;
    let hits = rig.fake.total();
    tokio::time::advance(Duration::from_secs(50)).await;
    settle().await;
    assert_eq!(rig.fake.total(), hits);
    rig.fake.set(|m| m.fail = None);
    tokio::time::advance(Duration::from_secs(20)).await;
    wait(&crawl.status, "ladder used up", state_is(0, "done: ladder used up")).await;
    assert_eq!(crawl.status.snapshot()["servers"][0]["stored"], 1);
    crawl.stop().await;
}

fn two_regions(rig: &Rig) {
    rig.apex("euw1", "CHALLENGER", &["e"]);
    rig.apex("na1", "CHALLENGER", &["n"]);
    let (euw, na): (Vec<String>, Vec<String>) = ((1..=5).map(id).collect(), (1..=5).map(|n| format!("NA1_{n:03}")).collect());
    rig.player("europe", "e", &euw);
    rig.player("americas", "n", &na);
    for (region, ids) in [("europe", &euw), ("americas", &na)] {
        for id in ids {
            rig.game(region, id, "16.21.1.2");
        }
    }
}

#[tokio::test]
async fn each_server_crawls_to_its_share() {
    let rig = Rig::new(&["16.21.1"]).await;
    two_regions(&rig);
    let mut crawl = rig.start(&["euw1/europe", "na1/americas"], 6);
    wait(&crawl.status, "both done", |v| state_is(0, "done")(v) && state_is(1, "done")(v)).await;
    crawl.stop().await;
    assert_eq!(rig.state("16.21", "euw1")["stored"].as_array().unwrap().len(), 3);
    assert_eq!(rig.state("16.21", "na1")["stored"].as_array().unwrap().len(), 3);
    assert_eq!(rig.raw_names("16.21").len(), 12, "one raw directory for all servers");
    let status = crawl.status.snapshot();
    assert_eq!((status["games"].as_u64(), status["max_games"].as_u64()), (Some(6), Some(6)));
}

#[tokio::test]
async fn the_servers_together_never_pass_max_games() {
    let rig = Rig::new(&["16.21.1"]).await;
    two_regions(&rig);
    let mut crawl = rig.start(&["euw1/europe", "na1/americas"], 5);
    wait(&crawl.status, "both done", |v| state_is(0, "done")(v) && state_is(1, "done")(v)).await;
    crawl.stop().await;
    let (euw, na) = (rig.state("16.21", "euw1")["stored"].as_array().unwrap().len(), rig.state("16.21", "na1")["stored"].as_array().unwrap().len());
    assert_eq!(euw + na, 5);
    assert!(euw <= 3 && na <= 3, "{euw} {na}");
    assert_eq!(rig.raw_names("16.21").len(), 10);
}

fn old_games(rig: &Rig, patch: &str, ids: &[&str]) {
    let dir = rig.patch_dir(patch);
    std::fs::create_dir_all(dir.join("raw")).unwrap();
    std::fs::create_dir_all(dir.join("fit")).unwrap();
    for id in ids {
        std::fs::write(dir.join("raw").join(format!("{id}.match.json.gz")), "x").unwrap();
        std::fs::write(dir.join("raw").join(format!("{id}.timeline.json.gz")), "x").unwrap();
    }
    std::fs::write(dir.join("rows.npz"), "x").unwrap();
    std::fs::write(dir.join("fit").join("winprob.toml"), "x").unwrap();
    std::fs::write(dir.join("euw1.json"), json!({ "seen": ids, "stored": ids }).to_string()).unwrap();
}

#[tokio::test]
async fn two_patches_share_max_games_and_older_patches_lose_their_raw_games() {
    let rig = Rig::new(&["16.22.1", "16.21.1", "16.20.1", "16.19.1"]).await;
    old_games(&rig, "16.19", &["X1"]);
    old_games(&rig, "16.20", &["Y1", "Y2"]);
    old_games(&rig, "16.21", &["O1", "O2", "O3", "O4"]);
    rig.one_player_world(&(1..=5).map(|n| (n, "16.22.1.2")).collect::<Vec<_>>());
    let mut crawl = rig.start(&["euw1/europe"], 6);
    wait(&crawl.status, "done", state_is(0, "done")).await;
    crawl.stop().await;
    assert_eq!(rig.state("16.22", "euw1")["stored"].as_array().unwrap().len(), 5);
    assert_eq!(rig.state("16.21", "euw1")["stored"], json!(["O4"]), "the oldest games of the older patch went first");
    assert_eq!(rig.raw_names("16.21"), ["O4.match.json.gz", "O4.timeline.json.gz"]);
    for old in ["16.19", "16.20"] {
        let dir = rig.patch_dir(old);
        assert!(!dir.join("raw").exists() && !dir.join("rows.npz").exists(), "{old}");
        assert!(dir.join("fit").join("winprob.toml").exists() && dir.join("euw1.json").exists(), "{old} keeps its fit and state");
    }
    assert!(rig.patch_dir("16.21").join("rows.npz").exists(), "the older of the two newest patches keeps its rows");
    let status = crawl.status.snapshot();
    assert_eq!((status["games"].as_u64(), status["patch"].as_str()), (Some(6), Some("16.22")));
}

fn commands_of(rig: &Rig) -> Vec<Command> {
    rig.commands.lock().unwrap().clone()
}

fn fit_world(rig: &Rig, games: u32) {
    rig.one_player_world(&(1..=games).map(|n| (n, "16.21.1.2")).collect::<Vec<_>>());
}

fn memo_of(rig: &Rig) -> Value {
    serde_json::from_slice(&std::fs::read(rig.patch_dir("16.21").join("fit.json")).unwrap()).unwrap()
}

#[tokio::test]
async fn the_fit_runs_as_two_commands_when_due_and_not_again_for_the_same_games() {
    let rig = Rig::new(&["16.21.2", "16.21.1", "16.20.1"]).await;
    fit_world(&rig, 5);
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 5, 2));
    wait(&crawl.status, "fit", |v| v["fit"]["fit_games"] == 5).await;
    crawl.stop().await;
    let dir = rig.patch_dir("16.21");
    let (tools, text) = (PathBuf::from("/tools"), |p: PathBuf| p.to_string_lossy().into_owned());
    let items = text(rig.root().join("winprob").join("items-16.21.1.json"));
    let want = vec![
        vec!["py".to_owned(), "-I".to_owned(), text(tools.join("features.py")), "--raw".to_owned(), text(dir.join("raw")), "--out".to_owned(), text(dir.join("rows.npz")), "--patch".to_owned(), "16.21".to_owned(), "--items".to_owned(), items.clone()],
        vec!["py".to_owned(), "-I".to_owned(), text(tools.join("fit.py")), "--rows".to_owned(), text(dir.join("rows.npz")), "--out".to_owned(), text(dir.join("fit").join("winprob.toml")), "--cases".to_owned(), text(dir.join("fit").join("winprob_cases.json"))],
    ];
    assert_eq!(commands_of(&rig), want);
    assert_eq!(std::fs::read_to_string(&items).unwrap(), ITEM_JSON, "the items of the first version of the patch");
    assert_eq!(rig.dd.hits_with("/cdn/16.21.1/data/en_US/item.json"), 1);
    let memo = memo_of(&rig);
    assert_eq!((memo["fit_games"].as_u64(), memo["passed"].as_bool()), (Some(5), Some(true)));
    assert!(memo["at"].as_u64().unwrap() > 0);
    assert_eq!(crawl.status.snapshot()["fit"], memo);

    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 5, 2));
    wait(&crawl.status, "done", state_is(0, "done")).await;
    settle().await;
    crawl.stop().await;
    assert_eq!(commands_of(&rig).len(), 2, "the same games are not fitted twice, also after a restart");
}

#[tokio::test]
async fn it_fits_again_after_refit_every_more_games_and_downloads_the_items_once() {
    let rig = Rig::new(&["16.21.1"]).await;
    fit_world(&rig, 5);
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 5, 5, 2));
    wait(&crawl.status, "first fit", |v| v["fit"]["fit_games"] == 5).await;
    crawl.stop().await;
    rig.apex("euw1", "MASTER", &["b"]);
    rig.player("europe", "b", &[id(6), id(7)]);
    rig.game("europe", &id(6), "16.21.1.2");
    rig.game("europe", &id(7), "16.21.1.2");
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 5, 2));
    wait(&crawl.status, "second fit", |v| v["fit"]["fit_games"] == 7).await;
    crawl.stop().await;
    assert_eq!(commands_of(&rig).len(), 4);
    assert_eq!(rig.dd.hits_with("item.json"), 1);
    assert_eq!(memo_of(&rig)["fit_games"], 7);
}

#[tokio::test]
async fn a_fit_whose_gates_fail_is_recorded_all_the_same() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.fit_code.store(1, Ordering::SeqCst);
    fit_world(&rig, 3);
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 3, 2));
    wait(&crawl.status, "fit", |v| v["fit"]["fit_games"] == 3).await;
    crawl.stop().await;
    assert_eq!(memo_of(&rig)["passed"], false);
    assert_eq!(commands_of(&rig).len(), 2);
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 3, 2));
    wait(&crawl.status, "done", state_is(0, "done")).await;
    settle().await;
    crawl.stop().await;
    assert_eq!(commands_of(&rig).len(), 2);
}

#[tokio::test]
async fn a_fit_waits_when_the_items_cannot_be_fetched() {
    let rig = Rig::new(&["16.21.1"]).await;
    *rig.dd.fail.lock().unwrap() = true;
    fit_world(&rig, 3);
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 3, 2));
    wait(&crawl.status, "Data Dragon", state_is(0, "waiting: Data Dragon")).await;
    crawl.stop().await;
    *rig.dd.fail.lock().unwrap() = false;
    let mut crawl = crawl::start(rig.env(&["euw1/europe"], 20, 3, 2));
    wait(&crawl.status, "fit", |v| v["fit"]["fit_games"] == 3).await;
    crawl.stop().await;
}

#[tokio::test]
async fn the_status_has_the_documented_shape_and_is_served_by_the_route() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.21.1.2")]);
    let mut crawl = rig.start(&["euw1/europe"], 1);
    let app = TestApp::with_crawl(crawl.status.clone());
    let before = app.get("/v1/status").await.json();
    assert_eq!(before["crawl"]["servers"][0]["platform"], "euw1");
    wait(&crawl.status, "done", state_is(0, "done")).await;
    let after = app.get("/v1/status").await.json();
    assert_eq!(
        after["crawl"],
        json!({ "patch": "16.21", "games": 1, "max_games": 1, "servers": [{ "platform": "euw1", "stored": 1, "players": 1, "next_player": 1, "state": "done" }], "fit": null })
    );
    crawl.stop().await;
    let off = TestApp::new().get("/v1/status").await.json();
    assert!(off["crawl"].is_null());
}

#[tokio::test(start_paused = true)]
async fn a_newer_patch_is_looked_for_every_hour() {
    let rig = Rig::new(&["16.21.1"]).await;
    rig.one_player_world(&[(1, "16.21.1.2")]);
    let mut crawl = rig.start(&["euw1/europe"], 5);
    wait(&crawl.status, "done", state_is(0, "done")).await;
    assert_eq!(rig.dd.hits_with("/api/versions.json"), 1);
    rig.dd.set_versions(&["16.22.1", "16.21.1"]);
    rig.player("europe", "a", &[id(10), id(1)]);
    rig.game("europe", &id(10), "16.22.1.2");
    tokio::time::advance(Duration::from_secs(1800)).await;
    settle().await;
    assert_eq!(rig.dd.hits_with("/api/versions.json"), 1, "not asked again within the hour");
    tokio::time::advance(Duration::from_secs(1900)).await;
    wait(&crawl.status, "the new patch", |v| v["patch"] == "16.22" && state_is(0, "done")(v)).await;
    let status = crawl.status.snapshot();
    assert_eq!((status["servers"][0]["stored"].as_u64(), status["games"].as_u64()), (Some(1), Some(2)));
    assert_eq!(rig.state("16.22", "euw1")["stored"], json!([id(10)]));
    assert_eq!(rig.state("16.21", "euw1")["stored"], json!([id(1)]));
    crawl.stop().await;
}

#[tokio::test(start_paused = true)]
async fn stopping_waits_30_seconds_for_all_tasks_together() {
    let hung = || tokio::spawn(async { tokio::time::sleep(Duration::from_secs(100_000)).await });
    let tasks = vec![hung(), hung(), hung()];
    let begun = tokio::time::Instant::now();
    crawl::wait_all(tasks, Duration::from_secs(30)).await;
    assert_eq!(begun.elapsed(), Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn stopping_does_not_wait_for_tasks_that_are_done() {
    let quick = |secs| tokio::spawn(async move { tokio::time::sleep(Duration::from_secs(secs)).await });
    let hung = tokio::spawn(async { tokio::time::sleep(Duration::from_secs(100_000)).await });
    let begun = tokio::time::Instant::now();
    crawl::wait_all(vec![quick(2), hung, quick(1)], Duration::from_secs(30)).await;
    assert_eq!(begun.elapsed(), Duration::from_secs(30));
    let begun = tokio::time::Instant::now();
    crawl::wait_all(vec![quick(2), quick(1)], Duration::from_secs(30)).await;
    assert_eq!(begun.elapsed(), Duration::from_secs(2));
}
