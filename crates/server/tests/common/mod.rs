#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Request, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use lol_coach_server::config::Config;
use lol_coach_server::players::{RiotId, sha256_hex};
use lol_coach_server::riot::Riot;
use lol_coach_server::{AppState, app};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tower::ServiceExt;

pub const TOKEN: &str = "test-token-0123456789abcdef0123456789";
pub const RIOT_KEY: &str = "RGAPI-test-key-not-a-real-one";
pub const ACCOUNT_KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub const FIXTURE_PUUID: &str = "PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP";

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub dir: TempDir,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("not json: {:?}", String::from_utf8_lossy(&self.body)))
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

impl TestApp {
    pub fn new() -> Self {
        Self::with(None, |_| {})
    }

    pub fn with(riot: Option<Riot>, tweak: impl FnOnce(&mut Config)) -> Self {
        let dir = TempDir::new().unwrap();
        let mut config = Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            data_dir: dir.path().to_owned(),
            tokens: vec![TOKEN.to_owned(), "second-token-0123456789abcdef012345".to_owned()],
            riot_key_file: None,
            lookup_ttl_hours: 12,
            max_file_mb: 1,
            claim_max_age_hours: 24,
            crawl: None,
        };
        tweak(&mut config);
        let state = AppState::new(config, riot);
        Self { router: app(state.clone()), state, dir }
    }

    pub fn with_crawl(status: Arc<lol_coach_server::crawl::Status>) -> Self {
        let dir = TempDir::new().unwrap();
        let config = Config { listen: "127.0.0.1:0".parse().unwrap(), data_dir: dir.path().to_owned(), tokens: vec![TOKEN.to_owned()], riot_key_file: None, lookup_ttl_hours: 12, max_file_mb: 1, claim_max_age_hours: 24, crawl: None };
        let mut state = AppState::new(config, None);
        state.crawl = Some(status);
        Self { router: app(state.clone()), state, dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn at(&mut self, now_ms: u64) {
        self.state.fixed_now_ms = Some(now_ms);
        self.router = app(self.state.clone());
    }

    pub fn register(&self, player: &str, key: &str) {
        let id = RiotId::from_path(player).unwrap();
        let dir = self.path().join("players").join(&id.key);
        std::fs::create_dir_all(&dir).unwrap();
        let account = serde_json::json!({ "keys": [sha256_hex(key.as_bytes())], "matches": [] });
        std::fs::write(dir.join("account.json"), account.to_string()).unwrap();
    }

    fn register_files_player(&self, uri: &str) {
        let Some(rest) = uri.strip_prefix("/v1/players/").filter(|r| r.contains("/files")) else { return };
        let Ok(id) = RiotId::from_path(rest.split('/').next().unwrap_or_default()) else { return };
        if !self.path().join("players").join(&id.key).join("account.json").exists() {
            self.register(rest.split('/').next().unwrap(), ACCOUNT_KEY);
        }
    }

    pub async fn send(&self, method: Method, uri: &str, token: Option<&str>, body: Vec<u8>) -> Reply {
        self.register_files_player(uri);
        self.send_as(method, uri, token, uri.starts_with("/v1/players/").then_some(ACCOUNT_KEY), body).await
    }

    pub async fn send_as(&self, method: Method, uri: &str, token: Option<&str>, key: Option<&str>, body: Vec<u8>) -> Reply {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(key) = key {
            req = req.header("X-Account-Key", key);
        }
        let res = self.router.clone().oneshot(req.body(Body::from(body)).unwrap()).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        Reply { status, headers, body }
    }

    pub async fn get(&self, uri: &str) -> Reply {
        self.send(Method::GET, uri, Some(TOKEN), Vec::new()).await
    }

    pub async fn put(&self, uri: &str, body: impl Into<Vec<u8>>) -> Reply {
        self.send(Method::PUT, uri, Some(TOKEN), body.into()).await
    }

    pub async fn delete(&self, uri: &str) -> Reply {
        self.send(Method::DELETE, uri, Some(TOKEN), Vec::new()).await
    }
}

pub fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else if entry.file_name() != "account.json" {
                out.push(entry.path());
            }
        }
    }
    out
}

pub fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/riot").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[derive(Clone)]
pub struct Mode {
    pub puuid: String,
    pub fail: Option<u16>,
    pub fail_first: u32,
    pub limited_once: bool,
    pub rate_limit: Option<String>,
    pub delay: Duration,
}

pub struct FakeState {
    pub hits: Mutex<Vec<String>>,
    pub mode: Mutex<Mode>,
    pub rejected: Mutex<u32>,
    pub tokens: Mutex<Vec<String>>,
    pub routes: Mutex<HashMap<String, (u16, Vec<u8>)>>,
}

pub struct FakeRiot {
    pub base: String,
    pub state: Arc<FakeState>,
}

impl FakeRiot {
    pub fn hits_with(&self, prefix: &str) -> usize {
        self.state.hits.lock().unwrap().iter().filter(|h| h.starts_with(prefix)).count()
    }

    pub fn hits_exact(&self, path: &str) -> usize {
        self.state.hits.lock().unwrap().iter().filter(|h| h.as_str() == path).count()
    }

    pub fn hit_list(&self) -> Vec<String> {
        self.state.hits.lock().unwrap().clone()
    }

    pub fn route(&self, path: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.state.routes.lock().unwrap().insert(path.to_owned(), (status, body.into()));
    }

    pub fn total(&self) -> usize {
        self.state.hits.lock().unwrap().len()
    }

    pub fn set(&self, f: impl FnOnce(&mut Mode)) {
        f(&mut self.state.mode.lock().unwrap());
    }

    pub fn hosted_riot(&self) -> Riot {
        let mut riot = Riot::with_base(RIOT_KEY, &format!("{}/{{host}}", self.base)).unwrap();
        riot.retry_base = Duration::from_millis(1);
        riot
    }

    pub fn riot(&self) -> Riot {
        let mut riot = Riot::with_base(RIOT_KEY, &self.base).unwrap();
        riot.retry_base = Duration::from_millis(1);
        riot
    }
}

fn reply(status: u16, extra: Option<&str>, body: String) -> Response {
    let mut res = (StatusCode::from_u16(status).unwrap(), [(header::CONTENT_TYPE, "application/json")], body).into_response();
    if let Some(limit) = extra {
        res.headers_mut().insert("x-app-rate-limit", limit.parse().unwrap());
    }
    res
}

fn find_route(state: &FakeState, path: &str) -> Option<(u16, Vec<u8>)> {
    let routes = state.routes.lock().unwrap();
    if let Some(found) = routes.get(path) {
        return Some(found.clone());
    }
    routes.iter().filter(|(key, _)| key.strip_suffix('*').is_some_and(|prefix| path.starts_with(prefix))).max_by_key(|(key, _)| key.len()).map(|(_, found)| found.clone())
}

async fn fake_handler(State(state): State<Arc<FakeState>>, uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path_and_query().map_or(uri.path().to_owned(), |p| p.as_str().to_owned());
    state.hits.lock().unwrap().push(path.clone());
    if let Some(token) = headers.get("x-riot-token").and_then(|v| v.to_str().ok()) {
        state.tokens.lock().unwrap().push(token.to_owned());
    }
    let (mode, first_failure) = {
        let mut mode = state.mode.lock().unwrap();
        let first = mode.fail_first > 0;
        mode.fail_first = mode.fail_first.saturating_sub(1);
        let limited = std::mem::take(&mut mode.limited_once);
        let mut out = mode.clone();
        out.limited_once = limited;
        (out, first)
    };
    let limit = mode.rate_limit.as_deref();
    if mode.limited_once {
        let mut res = reply(429, limit, fixture("rate_limited.json"));
        res.headers_mut().insert(header::RETRY_AFTER, "1".parse().unwrap());
        return res;
    }
    if let Some(code) = mode.fail.or(first_failure.then_some(500)) {
        return reply(code, limit, "{}".to_owned());
    }
    if let Some((code, body)) = find_route(&state, &path) {
        let mut res = (StatusCode::from_u16(code).unwrap(), [(header::CONTENT_TYPE, "application/json")], body).into_response();
        if let Some(limit) = limit {
            res.headers_mut().insert("x-app-rate-limit", limit.parse().unwrap());
        }
        return res;
    }
    let with_puuid = |name: &str| fixture(name).replace(FIXTURE_PUUID, &mode.puuid);
    let decrypt = || reply(400, limit, r#"{"status":{"message":"Exception decrypting PPPP","status_code":400}}"#.to_owned());
    let path_only = uri.path();
    if path_only.starts_with("/riot/account/v1/accounts/by-riot-id/") {
        return if path_only.contains("zzzz-no-such") { reply(404, limit, fixture("account_not_found.json")) } else { reply(200, limit, with_puuid("account_by_riot_id.json")) };
    }
    let by_puuid = [("/lol/league/v4/entries/by-puuid/", "league_entries_by_puuid.json"), ("/lol/champion-mastery/v4/champion-masteries/by-puuid/", "mastery_top.json"), ("/lol/summoner/v4/summoners/by-puuid/", "summoner_by_puuid.json")];
    for (prefix, file) in by_puuid {
        if let Some(rest) = path_only.strip_prefix(prefix) {
            tokio::time::sleep(mode.delay).await;
            if rest.split('/').next() != Some(mode.puuid.as_str()) {
                *state.rejected.lock().unwrap() += 1;
                return decrypt();
            }
            return reply(200, limit, with_puuid(file));
        }
    }
    reply(404, limit, "{}".to_owned())
}

pub async fn fake_riot() -> FakeRiot {
    let mode = Mode { puuid: FIXTURE_PUUID.to_owned(), fail: None, fail_first: 0, limited_once: false, rate_limit: None, delay: Duration::ZERO };
    let state = Arc::new(FakeState { hits: Mutex::default(), mode: Mutex::new(mode), rejected: Mutex::new(0), tokens: Mutex::default(), routes: Mutex::default() });
    let router = Router::new().fallback(fake_handler).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    FakeRiot { base, state }
}

pub struct FakeDdragon {
    pub base: String,
    pub versions: Arc<Mutex<Vec<String>>>,
    pub hits: Arc<Mutex<Vec<String>>>,
    pub fail: Arc<Mutex<bool>>,
}

impl FakeDdragon {
    pub fn hits_with(&self, needle: &str) -> usize {
        self.hits.lock().unwrap().iter().filter(|h| h.contains(needle)).count()
    }

    pub fn set_versions(&self, versions: &[&str]) {
        *self.versions.lock().unwrap() = versions.iter().map(|v| (*v).to_owned()).collect();
    }
}

pub const ITEM_JSON: &str = r#"{"type":"item","version":"x","data":{"1001":{"name":"Boots"}}}"#;

pub async fn fake_ddragon(versions: &[&str]) -> FakeDdragon {
    type Shared = (Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>, Arc<Mutex<bool>>);
    let shared: Shared = (Arc::new(Mutex::new(versions.iter().map(|v| (*v).to_owned()).collect())), Arc::default(), Arc::default());
    async fn handler(State((versions, hits, fail)): State<Shared>, uri: Uri) -> Response {
        hits.lock().unwrap().push(uri.path().to_owned());
        if *fail.lock().unwrap() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        if uri.path() == "/api/versions.json" {
            return ([(header::CONTENT_TYPE, "application/json")], serde_json::to_string(&*versions.lock().unwrap()).unwrap()).into_response();
        }
        if uri.path().starts_with("/cdn/") && uri.path().ends_with("/data/en_US/item.json") {
            return ([(header::CONTENT_TYPE, "application/json")], ITEM_JSON).into_response();
        }
        StatusCode::NOT_FOUND.into_response()
    }
    let router = Router::new().fallback(handler).with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    FakeDdragon { base, versions: shared.0, hits: shared.1, fail: shared.2 }
}
