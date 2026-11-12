use super::*;
use axum::Router;
use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use lol_coach_server::config::Config;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;

const TOKEN: &str = "test-token-0123456789abcdef0123456789";
const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_KEY: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const RIOT_ID: &str = "Some Player#EUW";
const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;

fn it(kind: &str, name: &str, sha: &str, modified: u64) -> Item {
    Item { kind: kind.into(), name: name.into(), sha256: sha.into(), modified }
}

fn pair(kind: &str, name: &str) -> (String, String) {
    (kind.into(), name.into())
}

#[test]
fn same_hash_is_left_alone_whatever_the_times() {
    let p = plan(&[it("history", "1.v7.json", "a", 5)], &[it("history", "1.v7.json", "a", 900)]);
    assert_eq!(p, Plan::default());
}

#[test]
fn a_file_only_here_goes_up_and_one_only_there_comes_down() {
    let p = plan(&[it("recaps", "100.json", "a", 5)], &[it("recaps", "200.json", "b", 5)]);
    assert_eq!(p, Plan { up: vec![pair("recaps", "100.json")], down: vec![pair("recaps", "200.json")] });
}

#[test]
fn of_two_versions_the_newer_wins_and_a_tie_is_left_alone() {
    let local = [it("ranked", "snapshots.json", "a", 100), it("recaps", "1.json", "a", 50), it("recaps", "2.json", "a", 70)];
    let remote = [it("ranked", "snapshots.json", "b", 90), it("recaps", "1.json", "b", 60), it("recaps", "2.json", "b", 70)];
    let p = plan(&local, &remote);
    assert_eq!(p, Plan { up: vec![pair("ranked", "snapshots.json")], down: vec![pair("recaps", "1.json")] });
}

#[test]
fn settings_go_up_when_they_differ_whatever_the_times() {
    let p = plan(&[it("settings", "settings.toml", "a", 1)], &[it("settings", "settings.toml", "b", 999)]);
    assert_eq!(p, Plan { up: vec![pair("settings", "settings.toml")], down: vec![] });
}

#[test]
fn settings_come_down_only_where_there_are_none() {
    let p = plan(&[], &[it("settings", "settings.toml", "b", 999)]);
    assert_eq!(p, Plan { up: vec![], down: vec![pair("settings", "settings.toml")] });
    let p = plan(&[it("settings", "settings.toml", "b", 1)], &[it("settings", "settings.toml", "b", 999)]);
    assert_eq!(p, Plan::default());
}

#[test]
fn names_and_kinds_that_are_not_ours_are_never_planned() {
    let remote = [
        it("history", "../escape.json", "x", 1),
        it("history", "a/b.json", "x", 1),
        it("history", ".hidden", "x", 1),
        it("lookup", "euw1.json", "x", 1),
        it("tft", "looks.bin", "x", 1),
        it("settings", "other.toml", "x", 1),
        it("history", &"n".repeat(129), "x", 1),
    ];
    assert_eq!(plan(&[], &remote), Plan::default());
    let local = [it("calib", "calib.json", "x", 1)];
    assert_eq!(plan(&local, &[]), Plan::default());
}

#[test]
fn the_plan_has_no_deletes_and_is_ordered() {
    let local = [it("recaps", "b.json", "1", 1), it("history", "a.json", "1", 1)];
    let p = plan(&local, &[]);
    assert_eq!(p.up, vec![pair("history", "a.json"), pair("recaps", "b.json")]);
    assert!(p.down.is_empty());
}

#[test]
fn the_settings_copy_leaves_out_the_address_and_the_token() {
    let text = "tier = \"all\"\nserver_url = \"https://lol.example\"\nserver_token = \"abc-secret\"\naudio = false\n[[goals]]\nname = \"x\"\n";
    let copy = settings_copy(text).unwrap();
    assert!(!copy.contains("server_url") && !copy.contains("server_token") && !copy.contains("abc-secret") && !copy.contains("lol.example"));
    let back: toml::Table = copy.parse().unwrap();
    assert_eq!(back["tier"].as_str(), Some("all"));
    assert_eq!(back["audio"].as_bool(), Some(false));
    assert_eq!(back["goals"].as_array().map(Vec::len), Some(1));
    assert_eq!(settings_copy(&copy).unwrap(), copy);
    assert!(settings_copy("not = [toml").is_none());
}

fn put(root: &Path, rel: &str, body: &str, age_s: i64) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, body).unwrap();
    let at = if age_s >= 0 { SystemTime::now() - Duration::from_secs(age_s as u64) } else { SystemTime::now() + Duration::from_secs(-age_s as u64) };
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

#[test]
fn the_local_listing_takes_regular_files_with_good_names_and_caches_hashes() {
    let t = TempDir::new().unwrap();
    let pd = t.path().join("player");
    put(&pd, "history/1.v7.json", "h", 10);
    put(&pd, "recaps/100.json", "r", 10);
    put(&pd, "ranked/snapshots.json", "s", 10);
    put(&pd, "history/.1.v7.json.part", "p", 10);
    put(&pd, "history/has space.json", "p", 10);
    put(&pd, "history/sub/deep.json", "p", 10);
    put(&pd, "tft/looks.bin", "t", 10);
    let settings = t.path().join("settings.toml");
    put(t.path(), "settings.toml", "tier = \"all\"\nserver_token = \"zzz\"\n", 10);
    let cache = HashCache::default();
    let mut items = local_items(&pd, &settings, &cache);
    items.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    let names: Vec<_> = items.iter().map(|i| format!("{}/{}", i.kind, i.name)).collect();
    assert_eq!(names, ["history/1.v7.json", "ranked/snapshots.json", "recaps/100.json", "settings/settings.toml"]);
    assert_eq!(items[0].sha256, sha256_hex(b"h"));
    assert_eq!(items[3].sha256, sha256_hex(settings_copy("tier = \"all\"\n").unwrap().as_bytes()));
    assert_eq!(cache.hashed(), 3);
    let mut again = local_items(&pd, &settings, &cache);
    again.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    assert_eq!(again, items);
    assert_eq!(cache.hashed(), 3);
    put(&pd, "recaps/100.json", "changed", 5);
    local_items(&pd, &settings, &cache);
    assert_eq!(cache.hashed(), 4);
}

#[test]
fn the_newest_game_comes_from_the_history_list() {
    let list: Value = serde_json::from_str(include_str!("../../fixtures/clients/match_history.json")).unwrap();
    let game = newest_game(&list).unwrap();
    assert_eq!(game, ClaimGame { platform: "eun1".into(), game_id: 3712004411, champion_id: 103, win: false, duration_s: 1874, ended_ms: 1792610000000 + 1874 * 1000 });
    assert!(game.recent(game.ended_ms + 23 * 3_600_000));
    assert!(!game.recent(game.ended_ms + 24 * 3_600_000));
    assert!(newest_game(&json!({ "games": { "games": [] } })).is_none());
}

#[test]
fn the_claim_body_has_the_fields_the_service_reads() {
    let body = ClaimBody { platform: "euw1", game_id: 7, champion_id: 14, win: true, duration_s: 1358, account_key: KEY };
    assert_eq!(serde_json::to_value(&body).unwrap(), json!({ "platform": "euw1", "game_id": 7, "champion_id": 14, "win": true, "duration_s": 1358, "account_key": KEY }));
}

#[derive(Clone, Default)]
struct Stub {
    seen: Arc<Mutex<Vec<(String, String, HeaderMap, Vec<u8>)>>>,
    reply: Arc<Mutex<(u16, String)>>,
    echo: bool,
}

async fn stub_handler(State(stub): State<Stub>, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let bytes = to_bytes(body, 1 << 20).await.unwrap().to_vec();
    let echoed = format!("{:?} {:?}", parts.headers.get("authorization"), parts.headers.get("x-account-key"));
    stub.seen.lock().unwrap().push((parts.method.to_string(), parts.uri.path().to_owned(), parts.headers, bytes));
    let (code, body) = stub.reply.lock().unwrap().clone();
    if code == 302 {
        return (StatusCode::FOUND, [("location", body)]).into_response();
    }
    let body = if stub.echo { json!({ "error": format!("unexpected {echoed} {body}") }).to_string() } else { body };
    (StatusCode::from_u16(code).unwrap(), [("content-type", "application/json")], body).into_response()
}

async fn stub(code: u16, body: &str, echo: bool) -> (String, Stub) {
    let stub = Stub { reply: Arc::new(Mutex::new((code, body.into()))), echo, ..Stub::default() };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(stub_handler).with_state(stub.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, stub)
}

fn game() -> ClaimGame {
    ClaimGame { platform: "euw1".into(), game_id: 7000000001, champion_id: 14, win: false, duration_s: 1358, ended_ms: 1 }
}

#[tokio::test]
async fn the_claim_is_one_post_with_the_token_and_the_key_in_the_body() {
    let (base, stub) = stub(200, r#"{"claimed":true}"#, false).await;
    let remote = Remote::new(&base, TOKEN, RIOT_ID, KEY).unwrap();
    assert!(matches!(remote.claim(&game()).await, Ok(Claimed::Accepted)));
    let seen = stub.seen.lock().unwrap();
    let (method, path, headers, body) = &seen[0];
    assert_eq!((method.as_str(), path.as_str()), ("POST", "/v1/players/Some%20Player%23EUW/claim"));
    assert_eq!(headers["authorization"], format!("Bearer {TOKEN}"));
    assert!(headers.get("x-account-key").is_none());
    assert_eq!(serde_json::from_slice::<Value>(body).unwrap(), json!({ "platform": "euw1", "game_id": 7000000001u64, "champion_id": 14, "win": false, "duration_s": 1358, "account_key": KEY }));
}

#[tokio::test]
async fn a_202_means_ask_again_and_403_409_404_are_refusals() {
    let (base, stub) = stub(202, r#"{"claimed":false,"retry":true}"#, false).await;
    let remote = Remote::new(&base, TOKEN, RIOT_ID, KEY).unwrap();
    assert!(matches!(remote.claim(&game()).await, Ok(Claimed::Later)));
    for (code, text) in [(403, "the match does not prove this account"), (409, "this match was used for a claim before"), (404, "not found"), (401, "unauthorized")] {
        *stub.reply.lock().unwrap() = (code, json!({ "error": text }).to_string());
        match remote.claim(&game()).await {
            Err(Fail::Status(got, msg)) => assert_eq!((got, msg.as_str()), (code, text)),
            other => panic!("{code}: {:?}", other.map(|_| ())),
        }
    }
}

#[tokio::test]
async fn a_failing_request_names_neither_the_token_nor_the_key() {
    let (base, _) = stub(500, "{}", true).await;
    let remote = Remote::new(&base, TOKEN, RIOT_ID, KEY).unwrap();
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let dead = Remote::new(&closed, TOKEN, RIOT_ID, KEY).unwrap();
    let errors = vec![
        remote.list().await.unwrap_err(),
        remote.fetch("recaps", "1.json").await.unwrap_err(),
        remote.store("recaps", "1.json", vec![1]).await.unwrap_err(),
        remote.claim(&game()).await.err().unwrap(),
        dead.list().await.unwrap_err(),
        dead.claim(&game()).await.err().unwrap(),
    ];
    assert!(matches!(&errors[0], Fail::Status(500, msg) if msg.contains("***")), "{}", errors[0]);
    assert!(matches!(errors[4], Fail::Net(_)));
    for e in errors {
        for text in [e.to_string(), format!("{e:?}")] {
            assert!(!text.contains(TOKEN) && !text.contains(KEY), "{text}");
        }
    }
}

#[tokio::test]
async fn a_redirect_is_not_followed_so_the_key_stays_with_the_service() {
    let (other, elsewhere) = stub(200, "[]", false).await;
    let (base, _) = stub(302, &format!("{other}/v1/players/x/files"), false).await;
    let remote = Remote::new(&base, TOKEN, RIOT_ID, KEY).unwrap();
    assert!(matches!(remote.list().await, Err(Fail::Status(302, _))));
    assert!(elsewhere.seen.lock().unwrap().is_empty());
}

struct Service {
    base: String,
    data: TempDir,
}

impl Service {
    async fn start() -> Service {
        let data = TempDir::new().unwrap();
        let config = Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            data_dir: data.path().to_owned(),
            tokens: vec![TOKEN.to_owned()],
            riot_key_file: None,
            lookup_ttl_hours: 12,
            max_file_mb: 32,
            claim_max_age_hours: 24,
            crawl: None,
        };
        let router = lol_coach_server::app(lol_coach_server::AppState::new(config, None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let service = Service { base, data };
        service.register(KEY);
        service
    }

    fn player_dir(&self) -> PathBuf {
        self.data.path().join("players").join(account::player_key(RIOT_ID).unwrap())
    }

    fn register(&self, key: &str) {
        let dir = self.player_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("account.json"), json!({ "keys": [sha256_hex(key.as_bytes())], "matches": [] }).to_string()).unwrap();
    }

    fn remote(&self, key: &str) -> Remote {
        Remote::new(&self.base, TOKEN, RIOT_ID, key).unwrap()
    }

    fn stored(&self) -> Vec<String> {
        let mut out = vec![];
        for kind in std::fs::read_dir(self.player_dir()).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
            for f in std::fs::read_dir(kind.path()).unwrap().filter_map(|e| e.ok()) {
                out.push(format!("{}/{}", kind.file_name().to_string_lossy(), f.file_name().to_string_lossy()));
            }
        }
        out.sort();
        out
    }
}

struct Pc {
    root: TempDir,
}

impl Pc {
    fn new() -> Pc {
        Pc { root: TempDir::new().unwrap() }
    }

    fn player(&self) -> PathBuf {
        self.root.path().join("players").join(account::player_key(RIOT_ID).unwrap())
    }

    fn settings(&self) -> PathBuf {
        self.root.path().join("settings.toml")
    }

    async fn sync(&self, remote: &Remote, cache: &Arc<HashCache>) -> Result<Report, Fail> {
        round(remote, &self.player(), &self.settings(), cache, NEVER).await
    }
}

const SETTINGS_TEXT: &str = "tier = \"all\"\nserver_url = \"http://127.0.0.1:1\"\nserver_token = \"test-token-0123456789abcdef0123456789\"\naudio = false\n";

#[tokio::test]
async fn files_go_up_from_one_pc_come_down_to_another_and_the_newer_wins() {
    let service = Service::start().await;
    let (a, b) = (Pc::new(), Pc::new());
    let cache = Arc::new(HashCache::default());
    put(&a.player(), "history/1.v7.json", "game one", 3000);
    put(&a.player(), "recaps/100.json", "recap a", 3000);
    put(&a.player(), "ranked/snapshots.json", "ranked from a", 3000);
    put(a.root.path(), "settings.toml", SETTINGS_TEXT, 3000);

    let first = a.sync(&service.remote(KEY), &cache).await.unwrap();
    assert_eq!(first, Report { sent: 4, received: 0, failed: 0, paused: false });
    assert_eq!(service.stored(), ["history/1.v7.json", "ranked/snapshots.json", "recaps/100.json", "settings/settings.toml"]);
    let on_service = std::fs::read_to_string(service.player_dir().join("settings").join("settings.toml")).unwrap();
    assert!(on_service.contains("tier") && !on_service.contains("server_url") && !on_service.contains("server_token") && !on_service.contains(TOKEN));
    assert_eq!(a.sync(&service.remote(KEY), &cache).await.unwrap(), Report::default());

    let second = b.sync(&service.remote(KEY), &Arc::new(HashCache::default())).await.unwrap();
    assert_eq!(second, Report { sent: 0, received: 4, failed: 0, paused: false });
    assert_eq!(read(&b.player(), "history/1.v7.json").as_deref(), Some("game one"));
    assert_eq!(read(&b.player(), "ranked/snapshots.json").as_deref(), Some("ranked from a"));
    assert_eq!(read(b.root.path(), "settings.toml").unwrap(), on_service);
    assert!(std::fs::read_dir(b.player().join("recaps")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));

    put(&b.player(), "ranked/snapshots.json", "ranked from b, newer", -2000);
    put(&b.player(), "recaps/100.json", "recap b, older", 9000);
    put(&b.player(), "recaps/200.json", "only on b", 100);
    let cache_b = Arc::new(HashCache::default());
    let third = b.sync(&service.remote(KEY), &cache_b).await.unwrap();
    assert_eq!(third, Report { sent: 2, received: 1, failed: 0, paused: false });
    assert_eq!(read(&b.player(), "recaps/100.json").as_deref(), Some("recap a"));
    assert_eq!(std::fs::read_to_string(service.player_dir().join("ranked").join("snapshots.json")).unwrap(), "ranked from b, newer");

    let back = a.sync(&service.remote(KEY), &cache).await.unwrap();
    assert_eq!(back, Report { sent: 0, received: 2, failed: 0, paused: false });
    assert_eq!(read(&a.player(), "ranked/snapshots.json").as_deref(), Some("ranked from b, newer"));
    assert_eq!(read(&a.player(), "recaps/200.json").as_deref(), Some("only on b"));
}

fn snap(time: u64, queue: &str, lp: u32) -> Value {
    json!({ "time": time, "queue": queue, "tier": "EMERALD", "division": "II", "lp": lp, "wins": 10, "losses": 9 })
}

#[test]
fn two_rank_logs_become_one_with_every_snapshot_once() {
    let a = json!([snap(100, "solo", 10), snap(300, "solo", 30)]).to_string();
    let b = json!([snap(100, "solo", 10), snap(200, "solo", 20), snap(200, "flex", 5)]).to_string();
    let merged: Vec<Value> = serde_json::from_slice(&merge_snapshots(a.as_bytes(), b.as_bytes()).unwrap()).unwrap();
    assert_eq!(merged, [snap(100, "solo", 10), snap(200, "flex", 5), snap(200, "solo", 20), snap(300, "solo", 30)]);
    assert_eq!(merge_snapshots(a.as_bytes(), b.as_bytes()), merge_snapshots(b.as_bytes(), a.as_bytes()), "the same whichever PC does it");
    assert_eq!(merge_snapshots(b"not a list", a.as_bytes()), None);
    assert_eq!(merge_snapshots(a.as_bytes(), b"{}"), None);
    assert_eq!(merge_snapshots(a.as_bytes(), b"[{\"queue\":\"solo\"}]"), None, "a snapshot without a time is not guessed at");
}

#[tokio::test]
async fn the_rank_logs_of_two_pcs_are_joined_and_neither_loses_a_snapshot() {
    let service = Service::start().await;
    let (a, b) = (Pc::new(), Pc::new());
    let (cache_a, cache_b) = (Arc::new(HashCache::default()), Arc::new(HashCache::default()));
    let logs = |pc: &Pc| -> Vec<Value> { serde_json::from_str(&read(&pc.player(), "ranked/snapshots.json").unwrap()).unwrap() };
    put(&a.player(), "ranked/snapshots.json", &json!([snap(100, "solo", 10), snap(300, "solo", 30)]).to_string(), 3000);
    assert_eq!(a.sync(&service.remote(KEY), &cache_a).await.unwrap(), Report { sent: 1, received: 0, failed: 0, paused: false });

    put(&b.player(), "ranked/snapshots.json", &json!([snap(200, "solo", 20)]).to_string(), 9000);
    let joined = b.sync(&service.remote(KEY), &cache_b).await.unwrap();
    assert_eq!(joined, Report { sent: 1, received: 1, failed: 0, paused: false }, "b is older, and still nothing of b is lost");
    let all = vec![snap(100, "solo", 10), snap(200, "solo", 20), snap(300, "solo", 30)];
    assert_eq!(logs(&b), all);
    assert_eq!(serde_json::from_slice::<Vec<Value>>(&std::fs::read(service.player_dir().join("ranked").join("snapshots.json")).unwrap()).unwrap(), all);

    assert_eq!(a.sync(&service.remote(KEY), &cache_a).await.unwrap(), Report { sent: 0, received: 1, failed: 0, paused: false });
    assert_eq!(logs(&a), all);
    assert_eq!(a.sync(&service.remote(KEY), &cache_a).await.unwrap(), Report::default());
    assert_eq!(b.sync(&service.remote(KEY), &cache_b).await.unwrap(), Report::default());
}

#[tokio::test]
async fn settings_are_only_uploaded_and_never_replace_this_pcs_own() {
    let service = Service::start().await;
    let (a, b) = (Pc::new(), Pc::new());
    let cache = Arc::new(HashCache::default());
    put(a.root.path(), "settings.toml", SETTINGS_TEXT, 3000);
    a.sync(&service.remote(KEY), &cache).await.unwrap();
    let own = "tier = \"gold\"\nserver_url = \"http://x\"\nserver_token = \"mine\"\n";
    put(b.root.path(), "settings.toml", own, 10);
    let report = b.sync(&service.remote(KEY), &Arc::new(HashCache::default())).await.unwrap();
    assert_eq!(report, Report { sent: 1, received: 0, failed: 0, paused: false });
    assert_eq!(read(b.root.path(), "settings.toml").as_deref(), Some(own));
    let on_service = std::fs::read_to_string(service.player_dir().join("settings").join("settings.toml")).unwrap();
    assert!(on_service.contains("gold") && !on_service.contains("mine") && !on_service.contains("http://x"));
}

#[tokio::test]
async fn a_wrong_account_key_gets_nothing_and_says_so_without_naming_the_key() {
    let service = Service::start().await;
    let a = Pc::new();
    let cache = Arc::new(HashCache::default());
    put(&a.player(), "recaps/100.json", "mine", 100);
    a.sync(&service.remote(KEY), &cache).await.unwrap();
    let before = service.stored();

    let b = Pc::new();
    put(&b.player(), "recaps/300.json", "intruder", 100);
    let wrong = service.remote(OTHER_KEY);
    let err = b.sync(&wrong, &Arc::new(HashCache::default())).await.unwrap_err();
    assert!(matches!(err, Fail::Status(403, _)), "{err}");
    for text in [err.to_string(), format!("{err:?}")] {
        assert!(!text.contains(OTHER_KEY) && !text.contains(KEY) && !text.contains(TOKEN), "{text}");
    }
    assert_eq!(service.stored(), before);
    assert!(read(&b.player(), "recaps/100.json").is_none());
    assert!(wrong.fetch("recaps", "100.json").await.is_err());
}

#[tokio::test]
async fn nothing_is_deleted_on_either_side() {
    let service = Service::start().await;
    let (a, b) = (Pc::new(), Pc::new());
    let cache = Arc::new(HashCache::default());
    put(&a.player(), "history/1.v7.json", "one", 3000);
    put(&a.player(), "history/2.v7.json", "two", 3000);
    a.sync(&service.remote(KEY), &cache).await.unwrap();
    std::fs::remove_file(a.player().join("history").join("1.v7.json")).unwrap();
    put(&b.player(), "history/3.v7.json", "three", 3000);
    b.sync(&service.remote(KEY), &Arc::new(HashCache::default())).await.unwrap();
    assert_eq!(service.stored(), ["history/1.v7.json", "history/2.v7.json", "history/3.v7.json"]);
    assert_eq!(read(&b.player(), "history/1.v7.json").as_deref(), Some("one"));
    let again = a.sync(&service.remote(KEY), &cache).await.unwrap();
    assert_eq!(again.received, 2);
    assert_eq!(read(&a.player(), "history/1.v7.json").as_deref(), Some("one"));
    assert_eq!(service.stored().len(), 3);
}

#[tokio::test]
async fn a_round_ends_at_the_next_file_when_a_game_starts() {
    let service = Service::start().await;
    let a = Pc::new();
    put(&a.player(), "recaps/100.json", "r", 100);
    put(&a.player(), "recaps/200.json", "r2", 100);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let stop = || calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 1;
    let report = round(&service.remote(KEY), &a.player(), &a.settings(), &Arc::new(HashCache::default()), &stop).await.unwrap();
    assert_eq!(report, Report { sent: 1, received: 0, failed: 0, paused: true });
    assert_eq!(service.stored().len(), 1);
}

#[tokio::test]
async fn a_dead_service_ends_the_round_with_one_error() {
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let a = Pc::new();
    put(&a.player(), "recaps/100.json", "r", 100);
    let remote = Remote::new(&closed, TOKEN, RIOT_ID, KEY).unwrap();
    let started = Instant::now();
    let err = a.sync(&remote, &Arc::new(HashCache::default())).await.unwrap_err();
    assert!(matches!(err, Fail::Net(_)));
    assert!(started.elapsed() < Duration::from_secs(20));
}
