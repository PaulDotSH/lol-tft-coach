use crate::config::CrawlConfig;
use crate::players::write_atomic;
use crate::riot::{Priority, Riot, RiotError, path_segment, region_of};
use crate::winprob::patch_number;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::future::Future;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, timeout_at};

pub const DDRAGON_BASE: &str = "https://ddragon.leagueoflegends.com";
const QUEUE: u32 = 420;
const DAYS: u64 = 14;
const IDS_PAGE: u32 = 100;
const LEAGUE_QUEUE: &str = "RANKED_SOLO_5x5";
const APEX: [(&str, &str); 3] = [
    ("CHALLENGER", "/lol/league/v4/challengerleagues/by-queue/RANKED_SOLO_5x5"),
    ("GRANDMASTER", "/lol/league/v4/grandmasterleagues/by-queue/RANKED_SOLO_5x5"),
    ("MASTER", "/lol/league/v4/masterleagues/by-queue/RANKED_SOLO_5x5"),
];
const TIERS: [&str; 2] = ["DIAMOND", "EMERALD"];
const DIVISIONS: [&str; 4] = ["I", "II", "III", "IV"];
const OTHER_KEY: &str = "Exception decrypting";
const KEY_WAIT: Duration = Duration::from_secs(600);
const RETRY_WAIT: Duration = Duration::from_secs(60);
const IDLE_WAIT: Duration = Duration::from_secs(60);
const PATCH_TTL: Duration = Duration::from_secs(3600);
const FIT_RETRY: Duration = Duration::from_secs(600);
const TOP_PASS_WAIT: Duration = Duration::from_secs(3600);
const STOP_WAIT: Duration = Duration::from_secs(30);
const STATE_CHARS: usize = 120;

pub type Command = Vec<String>;
pub type Runner = Arc<dyn Fn(Command) -> Pin<Box<dyn Future<Output = i32> + Send>> + Send + Sync>;

pub fn process_runner() -> Runner {
    Arc::new(|command: Command| {
        Box::pin(async move {
            let Some((program, args)) = command.split_first() else { return -1 };
            let tail = |bytes: &[u8]| {
                let text = String::from_utf8_lossy(bytes);
                let mut lines: Vec<&str> = text.lines().rev().take(20).collect();
                lines.reverse();
                lines.join("\n")
            };
            match tokio::process::Command::new(program).args(args).stdin(Stdio::null()).kill_on_drop(true).output().await {
                Ok(out) => {
                    let code = out.status.code().unwrap_or(-1);
                    tracing::info!(code, command = %command.join(" "), stdout = %tail(&out.stdout), stderr = %tail(&out.stderr), "fit step ended");
                    code
                }
                Err(err) => {
                    tracing::warn!(%err, command = %command.join(" "), "cannot start the fit step");
                    -1
                }
            }
        }) as Pin<Box<dyn Future<Output = i32> + Send>>
    })
}

#[derive(Clone)]
pub struct Env {
    pub config: CrawlConfig,
    pub data_dir: PathBuf,
    pub riot: Riot,
    pub ddragon: String,
    pub runner: Runner,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FitMemo {
    pub fit_games: u64,
    pub passed: bool,
    pub at: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct ServerStatus {
    pub platform: String,
    pub stored: u64,
    pub top: u64,
    pub lower: u64,
    pub players: usize,
    pub next_player: usize,
    pub lower_players: usize,
    pub next_lower: usize,
    pub state: String,
}

struct StatusInner {
    patch: Option<String>,
    older_games: u64,
    max_games: u64,
    servers: Vec<ServerStatus>,
    fit: Option<FitMemo>,
}

impl StatusInner {
    fn newest_games(&self) -> u64 {
        self.servers.iter().map(|s| s.stored).sum()
    }
}

pub struct Status(Mutex<StatusInner>);

impl Status {
    fn new(max_games: u64, platforms: &[String]) -> Self {
        let servers = platforms.iter().map(|p| ServerStatus { platform: p.clone(), stored: 0, top: 0, lower: 0, players: 0, next_player: 0, lower_players: 0, next_lower: 0, state: "starting".to_owned() }).collect();
        Self(Mutex::new(StatusInner { patch: None, older_games: 0, max_games, servers, fit: None }))
    }

    pub fn snapshot(&self) -> Value {
        let s = self.0.lock().unwrap();
        json!({ "patch": s.patch, "games": s.newest_games() + s.older_games, "max_games": s.max_games, "servers": s.servers, "fit": s.fit })
    }

    pub fn newest_games(&self) -> u64 {
        self.0.lock().unwrap().newest_games()
    }

    pub fn fit(&self) -> Option<FitMemo> {
        self.0.lock().unwrap().fit.clone()
    }

    fn update(&self, idx: usize, change: impl FnOnce(&mut ServerStatus)) {
        change(&mut self.0.lock().unwrap().servers[idx]);
    }

    fn begin_patch(&self, patch: &str, older_games: u64, fit: Option<FitMemo>) {
        let mut s = self.0.lock().unwrap();
        s.patch = Some(patch.to_owned());
        s.older_games = older_games;
        s.fit = fit;
        for server in &mut s.servers {
            (server.stored, server.top, server.lower) = (0, 0, 0);
        }
    }

    fn set_older_games(&self, games: u64) {
        self.0.lock().unwrap().older_games = games;
    }

    fn set_fit(&self, patch: &str, memo: FitMemo) {
        let mut s = self.0.lock().unwrap();
        if s.patch.as_deref() == Some(patch) {
            s.fit = Some(memo);
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Ladder {
    apex_done: Vec<String>,
    puuids: Vec<String>,
    next_puuid: usize,
    pass_start: u64,
    lower_puuids: Vec<LowerPlayer>,
    next_lower: usize,
    pages: BTreeMap<String, u32>,
    exhausted: Vec<String>,
    turn: usize,
}

#[derive(Serialize, Deserialize, Clone)]
struct LowerPlayer {
    puuid: String,
    tier: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    ladder: Ladder,
    seen: Vec<String>,
    stored: Vec<String>,
    bands: BTreeMap<String, String>,
    older: Vec<String>,
}

impl Saved {
    fn counts(&self) -> (u64, u64) {
        let lower = self.bands.len() as u64;
        ((self.stored.len() as u64).saturating_sub(lower), lower)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    Top,
    Lower,
}

pub fn may_take(band: u64, stored: u64, share: u64) -> bool {
    band < share.div_ceil(2) && stored < share
}

pub fn next_queue(top: u64, lower: u64, share: u64, top_ready: bool, lower_ready: bool, top_turn: bool) -> Option<Queue> {
    let stored = top + lower;
    let top_may = top_ready && may_take(top, stored, share);
    let lower_may = lower_ready && may_take(lower, stored, share);
    match (top_may, lower_may) {
        (true, true) => Some(if top_turn { Queue::Top } else { Queue::Lower }),
        (true, false) => Some(Queue::Top),
        (false, true) => Some(Queue::Lower),
        (false, false) => None,
    }
}

struct Loaded {
    patch: String,
    path: PathBuf,
    raw: PathBuf,
    saved: Saved,
    seen: HashSet<String>,
    puuids: HashSet<String>,
    lower_puuids: HashSet<String>,
    older: HashSet<String>,
    reread: bool,
    top_turn: bool,
    top_wait: Option<Instant>,
}

impl Loaded {
    async fn open(data_dir: &Path, patch: &str, platform: &str) -> Result<Self, String> {
        let dir = patch_dir(data_dir, patch);
        let path = dir.join(format!("{platform}.json"));
        let saved: Saved = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|err| format!("{} is unreadable: {err}", path.display()))?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Saved::default(),
            Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
        };
        let raw = dir.join("raw");
        tokio::fs::create_dir_all(&raw).await.map_err(|err| format!("cannot create {}: {err}", raw.display()))?;
        let (seen, puuids) = (saved.seen.iter().cloned().collect(), saved.ladder.puuids.iter().cloned().collect());
        let (lower_puuids, older) = (saved.ladder.lower_puuids.iter().map(|p| p.puuid.clone()).collect(), saved.older.iter().cloned().collect());
        Ok(Self { patch: patch.to_owned(), path, raw, saved, seen, puuids, lower_puuids, older, reread: false, top_turn: true, top_wait: None })
    }

    fn lower_used_up(&self) -> bool {
        let ladder = &self.saved.ladder;
        ladder.next_lower >= ladder.lower_puuids.len() && ladder_order().iter().all(|d| ladder.exhausted.contains(d))
    }

    fn add_lower(&mut self, tier: &str, puuids: impl IntoIterator<Item = String>) {
        for puuid in puuids {
            if self.lower_puuids.insert(puuid.clone()) {
                self.saved.ladder.lower_puuids.push(LowerPlayer { puuid, tier: tier.to_owned() });
            }
        }
    }

    fn mark_seen(&mut self, id: &str) {
        if self.seen.insert(id.to_owned()) {
            self.saved.seen.push(id.to_owned());
        }
    }

    fn add_players(&mut self, puuids: impl IntoIterator<Item = String>) {
        for puuid in puuids {
            if self.puuids.insert(puuid.clone()) {
                self.saved.ladder.puuids.push(puuid);
            }
        }
    }
}

struct Store {
    newest: Option<String>,
    older: Option<String>,
    older_games: u64,
}

struct Versions {
    list: Vec<String>,
    next_check: Option<Instant>,
}

struct Gate {
    running: bool,
    retry_at: Option<Instant>,
}

struct Shared {
    env: Env,
    http: reqwest::Client,
    status: Arc<Status>,
    servers: Vec<(String, String)>,
    versions: tokio::sync::Mutex<Versions>,
    store: tokio::sync::Mutex<Store>,
    gate: Mutex<Gate>,
    stop: watch::Receiver<bool>,
}

pub struct Crawl {
    pub status: Arc<Status>,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

pub fn start(env: Env) -> Crawl {
    let servers: Vec<(String, String)> = env.config.servers.iter().filter_map(|s| s.split_once('/')).map(|(p, r)| (p.to_owned(), r.to_owned())).collect();
    let platforms: Vec<String> = servers.iter().map(|(p, _)| p.clone()).collect();
    let status = Arc::new(Status::new(env.config.max_games, &platforms));
    let (stop_tx, stop_rx) = watch::channel(false);
    let http = reqwest::Client::builder()
        .user_agent(concat!("lol-coach-server/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default();
    let shared = Arc::new(Shared {
        env,
        http,
        status: status.clone(),
        servers,
        versions: tokio::sync::Mutex::new(Versions { list: Vec::new(), next_check: None }),
        store: tokio::sync::Mutex::new(Store { newest: None, older: None, older_games: 0 }),
        gate: Mutex::new(Gate { running: false, retry_at: None }),
        stop: stop_rx,
    });
    let tasks = (0..shared.servers.len()).map(|idx| tokio::spawn(serve(Ctx::new(shared.clone(), idx)))).collect();
    Crawl { status, stop: stop_tx, tasks }
}

impl Crawl {
    pub async fn stop(&mut self) {
        let _ = self.stop.send(true);
        wait_all(std::mem::take(&mut self.tasks), STOP_WAIT).await;
    }
}

pub async fn wait_all(tasks: Vec<JoinHandle<()>>, wait: Duration) {
    let deadline = Instant::now() + wait;
    for task in tasks {
        let abort = task.abort_handle();
        if timeout_at(deadline, task).await.is_err() {
            abort.abort();
        }
    }
}

pub fn due(games: u64, last: Option<u64>, fit_at: u64, refit_every: u64) -> bool {
    games >= last.map_or(fit_at, |last| last + refit_every)
}

pub fn ladder_order() -> Vec<String> {
    DIVISIONS.iter().flat_map(|d| TIERS.iter().map(move |t| format!("{t} {d}"))).collect()
}

pub fn patch_of_version(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let (major, minor) = (parts.next()?, parts.next()?);
    (major.parse::<u32>().is_ok() && minor.parse::<u32>().is_ok()).then(|| format!("{major}.{minor}"))
}

fn winprob_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("winprob")
}

fn patch_dir(data_dir: &Path, patch: &str) -> PathBuf {
    winprob_dir(data_dir).join(patch)
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[derive(Deserialize)]
struct Head {
    info: HeadInfo,
}

#[derive(Deserialize)]
struct HeadInfo {
    #[serde(rename = "gameVersion")]
    game_version: String,
}

fn game_patch(bytes: &[u8]) -> Option<String> {
    let head: Head = serde_json::from_slice(bytes).ok()?;
    patch_of_version(&head.info.game_version)
}

#[derive(Deserialize)]
struct Entry {
    puuid: String,
}

#[derive(Deserialize)]
struct League {
    entries: Vec<Entry>,
}

fn gzip(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
}

fn game_files(raw: &Path, id: &str) -> [PathBuf; 2] {
    [raw.join(format!("{id}.match.json.gz")), raw.join(format!("{id}.timeline.json.gz"))]
}

fn write_game(raw: &Path, id: &str, game: &[u8], timeline: &[u8], state_path: &Path, state: &[u8]) -> io::Result<()> {
    let [game_path, timeline_path] = game_files(raw, id);
    write_atomic(&timeline_path, &gzip(timeline)?)?;
    write_atomic(&game_path, &gzip(game)?)?;
    write_atomic(state_path, state)
}

fn state_files(dir: &Path) -> Vec<(PathBuf, Saved)> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Saved)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json") && p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| region_of(s).is_some()))
        .filter_map(|p| {
            let saved = serde_json::from_slice(&fs::read(&p).ok()?).inspect_err(|err| tracing::warn!(path = %p.display(), %err, "unreadable crawl state")).ok()?;
            Some((p, saved))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn evict_one(dir: &Path) -> io::Result<bool> {
    let mut best: Option<(PathBuf, Saved)> = None;
    for (path, saved) in state_files(dir) {
        if !saved.stored.is_empty() && best.as_ref().is_none_or(|(_, b)| saved.stored.len() > b.stored.len()) {
            best = Some((path, saved));
        }
    }
    let Some((path, mut saved)) = best else { return Ok(false) };
    let id = saved.stored.remove(0);
    saved.bands.remove(&id);
    write_atomic(&path, &serde_json::to_vec(&saved)?)?;
    for file in game_files(&dir.join("raw"), &id) {
        match fs::remove_file(file) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(true)
}

fn rearrange(root: &Path, patch: &str) -> (Option<String>, u64, Option<FitMemo>) {
    let newest = patch_number(patch);
    let mut earlier: Vec<(Vec<u64>, String)> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let number = patch_number(&name)?;
            (Some(&number) < newest.as_ref()).then_some((number, name))
        })
        .collect();
    earlier.sort();
    let older = earlier.pop().map(|(_, name)| name);
    for (_, name) in &earlier {
        let dir = root.join(name);
        if let Err(err) = fs::remove_dir_all(dir.join("raw")).or_else(|err| if err.kind() == io::ErrorKind::NotFound { Ok(()) } else { Err(err) }) {
            tracing::warn!(patch = %name, %err, "cannot remove the raw games of an old patch");
        }
        let _ = fs::remove_file(dir.join("rows.npz"));
        tracing::info!(patch = %name, "removed the raw games of an old patch");
    }
    let games = older.as_ref().map_or(0, |name| state_files(&root.join(name)).iter().map(|(_, s)| s.stored.len() as u64).sum());
    let fit = fs::read(root.join(patch).join("fit.json")).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
    (older, games, fit)
}

async fn save_state(path: &Path, saved: &Saved) {
    let Ok(bytes) = serde_json::to_vec(saved) else { return };
    let path = path.to_owned();
    match tokio::task::spawn_blocking(move || write_atomic(&path, &bytes).map_err(|err| (path, err))).await {
        Ok(Ok(())) => {}
        Ok(Err((path, err))) => tracing::warn!(path = %path.display(), %err, "cannot save the crawl state"),
        Err(err) => tracing::warn!(%err, "cannot save the crawl state"),
    }
}

async fn fetch_versions(sh: &Shared) -> Result<Vec<String>, String> {
    let url = format!("{}/api/versions.json", sh.env.ddragon);
    let response = sh.http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|err| err.without_url().to_string())?;
    let list: Vec<String> = response.json().await.map_err(|err| err.without_url().to_string())?;
    match list.first().map(|v| patch_of_version(v)) {
        Some(Some(_)) => Ok(list),
        _ => Err("no usable version in the list".to_owned()),
    }
}

async fn newest_patch(sh: &Shared) -> Result<String, String> {
    let mut versions = sh.versions.lock().await;
    if versions.next_check.is_none_or(|t| Instant::now() >= t) {
        match fetch_versions(sh).await {
            Ok(list) => {
                versions.list = list;
                versions.next_check = Some(Instant::now() + PATCH_TTL);
            }
            Err(err) => {
                versions.next_check = Some(Instant::now() + RETRY_WAIT);
                if versions.list.is_empty() {
                    return Err(err);
                }
                tracing::warn!(%err, "cannot read the Data Dragon versions, keeping the last list");
            }
        }
    }
    versions.list.first().and_then(|v| patch_of_version(v)).ok_or_else(|| "no versions yet".to_owned())
}

async fn items_file(sh: &Shared, patch: &str) -> Result<PathBuf, String> {
    let version = {
        let versions = sh.versions.lock().await;
        versions.list.iter().rev().find(|v| patch_of_version(v).as_deref() == Some(patch)).cloned().ok_or_else(|| format!("Data Dragon has no version of patch {patch}"))?
    };
    let path = winprob_dir(&sh.env.data_dir).join(format!("items-{version}.json"));
    if tokio::fs::metadata(&path).await.is_ok_and(|m| m.len() > 0) {
        return Ok(path);
    }
    let url = format!("{}/cdn/{version}/data/en_US/item.json", sh.env.ddragon);
    let response = sh.http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|err| err.without_url().to_string())?;
    let bytes = response.bytes().await.map_err(|err| err.without_url().to_string())?;
    serde_json::from_slice::<Value>(&bytes).map_err(|err| format!("item.json of {version} is not JSON: {err}"))?;
    let target = path.clone();
    tokio::task::spawn_blocking(move || write_atomic(&target, &bytes)).await.map_err(|err| err.to_string())?.map_err(|err| err.to_string())?;
    Ok(path)
}

async fn switch_patch(sh: &Shared, patch: &str) {
    let mut store = sh.store.lock().await;
    if store.newest.as_deref() == Some(patch) {
        return;
    }
    let (root, wanted) = (winprob_dir(&sh.env.data_dir), patch.to_owned());
    let (older, games, fit) = tokio::task::spawn_blocking(move || rearrange(&root, &wanted)).await.unwrap_or_default();
    tracing::info!(patch, older = ?older, older_games = games, "crawling a patch");
    *store = Store { newest: Some(patch.to_owned()), older, older_games: games };
    sh.status.begin_patch(patch, games, fit);
}

async fn evict(sh: &Shared, store: &mut Store) {
    let Some(older) = store.older.clone() else { return };
    while store.older_games > 0 && sh.status.newest_games() + store.older_games > sh.env.config.max_games {
        let dir = patch_dir(&sh.env.data_dir, &older);
        match tokio::task::spawn_blocking(move || evict_one(&dir)).await {
            Ok(Ok(true)) => store.older_games -= 1,
            Ok(Ok(false)) => store.older_games = 0,
            Ok(Err(err)) => {
                tracing::warn!(%err, patch = %older, "cannot delete a game of the older patch");
                break;
            }
            Err(err) => {
                tracing::warn!(%err, "cannot delete a game of the older patch");
                break;
            }
        }
        sh.status.set_older_games(store.older_games);
    }
}

fn maybe_fit(sh: &Arc<Shared>, patch: &str) {
    let games = sh.status.newest_games();
    let config = &sh.env.config;
    if !due(games, sh.status.fit().map(|f| f.fit_games), config.fit_at, config.refit_every) {
        return;
    }
    {
        let mut gate = sh.gate.lock().unwrap();
        if gate.running || gate.retry_at.is_some_and(|t| Instant::now() < t) {
            return;
        }
        gate.running = true;
    }
    tokio::spawn(run_fit(sh.clone(), patch.to_owned(), games));
}

async fn run_fit(sh: Arc<Shared>, patch: String, games: u64) {
    let mut stop = sh.stop.clone();
    let result = tokio::select! {
        _ = stop.wait_for(|stopped| *stopped) => None,
        result = fit(&sh, &patch, games) => Some(result),
    };
    let mut gate = sh.gate.lock().unwrap();
    gate.running = false;
    match result {
        Some(Err(err)) => {
            tracing::warn!(%err, patch, "cannot fit");
            gate.retry_at = Some(Instant::now() + FIT_RETRY);
        }
        Some(Ok(memo)) => {
            tracing::info!(patch, fit_games = memo.fit_games, passed = memo.passed, "fit ended");
            gate.retry_at = None;
        }
        None => {}
    }
}

async fn fit(sh: &Shared, patch: &str, games: u64) -> Result<FitMemo, String> {
    let dir = patch_dir(&sh.env.data_dir, patch);
    let items = items_file(sh, patch).await?;
    tokio::fs::create_dir_all(dir.join("fit")).await.map_err(|err| err.to_string())?;
    let config = &sh.env.config;
    let text = |path: &Path| path.to_string_lossy().into_owned();
    let (rows, table, cases) = (dir.join("rows.npz"), dir.join("fit").join("winprob.toml"), dir.join("fit").join("winprob_cases.json"));
    let features = vec![config.python.clone(), "-I".to_owned(), text(&config.tools_dir.join("features.py")), "--raw".to_owned(), text(&dir.join("raw")), "--out".to_owned(), text(&rows), "--patch".to_owned(), patch.to_owned(), "--items".to_owned(), text(&items)];
    let fitting = vec![config.python.clone(), "-I".to_owned(), text(&config.tools_dir.join("fit.py")), "--rows".to_owned(), text(&rows), "--out".to_owned(), text(&table), "--cases".to_owned(), text(&cases)];
    let passed = (sh.env.runner)(features).await == 0 && (sh.env.runner)(fitting).await == 0;
    match std::env::current_exe() {
        Ok(exe) => {
            let builds = vec![text(&exe), "builds".to_owned(), "--raw".to_owned(), text(&dir.join("raw")), "--bands".to_owned(), text(&dir), "--items".to_owned(), text(&items), "--patch".to_owned(), patch.to_owned(), "--out".to_owned(), text(&dir.join("builds.json"))];
            let code = (sh.env.runner)(builds).await;
            if code != 0 {
                tracing::warn!(code, patch, "the build tables were not made");
            }
        }
        Err(err) => tracing::warn!(%err, "cannot find the service's own executable for the build tables"),
    }
    let memo = FitMemo { fit_games: games, passed, at: now_secs() };
    let bytes = serde_json::to_vec(&memo).map_err(|err| err.to_string())?;
    let target = dir.join("fit.json");
    tokio::task::spawn_blocking(move || write_atomic(&target, &bytes)).await.map_err(|err| err.to_string())?.map_err(|err| err.to_string())?;
    sh.status.set_fit(patch, memo.clone());
    Ok(memo)
}

enum Next {
    Again,
    Wait(Duration),
    Stop(String),
}

enum Fail {
    Riot(RiotError),
    Disk(String),
}

impl From<RiotError> for Fail {
    fn from(err: RiotError) -> Self {
        Self::Riot(err)
    }
}

enum Game {
    Done,
    EndList,
    Halt,
}

struct Ctx {
    sh: Arc<Shared>,
    idx: usize,
    platform: String,
    region: String,
    share: u64,
}

fn other_key(err: &RiotError) -> bool {
    matches!(err, RiotError::Status(400, body) if body.contains(OTHER_KEY))
}

impl Ctx {
    fn new(sh: Arc<Shared>, idx: usize) -> Self {
        let (platform, region) = sh.servers[idx].clone();
        let share = sh.env.config.max_games.div_ceil(sh.servers.len() as u64);
        Self { sh, idx, platform, region, share }
    }

    fn halted(&self) -> bool {
        *self.sh.stop.borrow()
    }

    async fn pause(&self, wait: Duration) {
        let mut stop = self.sh.stop.clone();
        tokio::select! {
            () = sleep(wait) => {}
            _ = stop.wait_for(|stopped| *stopped) => {}
        }
    }

    fn state(&self, text: impl AsRef<str>) {
        let text: String = text.as_ref().chars().take(STATE_CHARS).collect();
        self.sh.status.update(self.idx, |s| s.state = text);
    }

    fn publish(&self, c: &Loaded) {
        let (top, lower) = c.saved.counts();
        self.sh.status.update(self.idx, |s| {
            s.stored = c.saved.stored.len() as u64;
            (s.top, s.lower) = (top, lower);
            s.players = c.saved.ladder.puuids.len();
            s.next_player = c.saved.ladder.next_puuid;
            s.lower_players = c.saved.ladder.lower_puuids.len();
            s.next_lower = c.saved.ladder.next_lower;
        });
    }

    async fn save(&self, c: &Loaded) {
        save_state(&c.path, &c.saved).await;
        self.publish(c);
    }

    async fn json<T: DeserializeOwned>(&self, host: &str, path: &str) -> Result<Option<T>, RiotError> {
        match self.sh.env.riot.get(host, path, Priority::Crawl).await {
            Ok(value) => Ok(Some(value)),
            Err(RiotError::NotFound) => Ok(None),
            Err(err) => Err(err),
        }
    }

    async fn bytes(&self, path: &str) -> Result<Option<Vec<u8>>, RiotError> {
        match self.sh.env.riot.get_bytes(&self.region, path, Priority::Crawl).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RiotError::NotFound) => Ok(None),
            Err(err @ RiotError::Status(..)) if !other_key(&err) => {
                tracing::warn!(%err, path, "skipping a game");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    async fn step(&self, c: &mut Loaded) -> Next {
        if c.saved.stored.len() as u64 >= self.share || self.sh.status.newest_games() >= self.sh.env.config.max_games {
            self.state("done");
            return Next::Wait(IDLE_WAIT);
        }
        let (top, lower) = c.saved.counts();
        let top_ready = c.top_wait.is_none_or(|t| Instant::now() >= t);
        let Some(queue) = next_queue(top, lower, self.share, top_ready, !c.lower_used_up(), c.top_turn) else {
            self.state(if c.lower_used_up() { "done: ladder used up" } else { "waiting: the top leagues are read again within the hour" });
            return Next::Wait(IDLE_WAIT);
        };
        self.state("crawling");
        let ladder = &c.saved.ladder;
        let result = match queue {
            Queue::Top if ladder.next_puuid >= ladder.puuids.len() => self.more_top(c).await,
            Queue::Lower if ladder.next_lower >= ladder.lower_puuids.len() => self.more_lower(c).await,
            queue => self.player(c, queue).await,
        };
        match result {
            Ok(next) => next,
            Err(Fail::Riot(err)) if other_key(&err) => self.reread(c).await,
            Err(fail) => self.failed(fail),
        }
    }

    async fn reread(&self, c: &mut Loaded) -> Next {
        if c.reread {
            return Next::Stop("stopped: the player ids were rejected again right after reading the ladder again".to_owned());
        }
        c.reread = true;
        tracing::info!(players = c.saved.ladder.puuids.len(), "the players were read with another key, reading the ladder again");
        c.saved.ladder = Ladder { pass_start: c.saved.counts().0, ..Ladder::default() };
        c.puuids.clear();
        c.lower_puuids.clear();
        c.top_wait = None;
        self.save(c).await;
        Next::Again
    }

    fn failed(&self, fail: Fail) -> Next {
        let (state, wait) = match fail {
            Fail::Riot(RiotError::KeyRejected) => ("waiting: key rejected".to_owned(), KEY_WAIT),
            Fail::Riot(RiotError::Unavailable(msg)) => (format!("waiting: riot unavailable: {msg}"), RETRY_WAIT),
            Fail::Riot(err) => (format!("waiting: {err}"), RETRY_WAIT),
            Fail::Disk(msg) => (format!("waiting: {msg}"), RETRY_WAIT),
        };
        tracing::warn!(platform = %self.platform, %state, "crawl waits");
        self.state(state);
        Next::Wait(wait)
    }

    async fn more_top(&self, c: &mut Loaded) -> Result<Next, Fail> {
        if let Some((tier, path)) = APEX.iter().find(|(tier, _)| !c.saved.ladder.apex_done.iter().any(|d| d == tier)) {
            let league: Option<League> = self.json(&self.platform, path).await?;
            c.add_players(league.into_iter().flat_map(|l| l.entries).map(|e| e.puuid));
            c.saved.ladder.apex_done.push((*tier).to_owned());
            self.save(c).await;
            return Ok(Next::Again);
        }
        let (top, _) = c.saved.counts();
        if top == c.saved.ladder.pass_start && c.top_wait.is_none() {
            c.top_wait = Some(Instant::now() + TOP_PASS_WAIT);
            return Ok(Next::Again);
        }
        c.top_wait = None;
        tracing::info!(platform = %self.platform, players = c.saved.ladder.puuids.len(), games = top.saturating_sub(c.saved.ladder.pass_start),"the top leagues are used up, starting over");
        let ladder = &mut c.saved.ladder;
        (ladder.apex_done, ladder.puuids, ladder.next_puuid, ladder.pass_start) = (Vec::new(), Vec::new(), 0, top);
        c.puuids.clear();
        self.save(c).await;
        Ok(Next::Again)
    }

    async fn more_lower(&self, c: &mut Loaded) -> Result<Next, Fail> {
        let all = ladder_order();
        let start = c.saved.ladder.turn % all.len();
        let Some(at) = (0..all.len()).map(|i| (start + i) % all.len()).find(|&i| !c.saved.ladder.exhausted.contains(&all[i])) else {
            return Ok(Next::Again);
        };
        let division = all[at].clone();
        let page = c.saved.ladder.pages.get(&division).copied().unwrap_or(1);
        let (tier, rank) = division.split_once(' ').unwrap_or((&division, "I"));
        let path = format!("/lol/league/v4/entries/{LEAGUE_QUEUE}/{tier}/{rank}?page={page}");
        let entries: Vec<Entry> = self.json(&self.platform, &path).await?.unwrap_or_default();
        c.saved.ladder.turn = at + 1;
        c.saved.ladder.pages.insert(division.clone(), page + 1);
        if entries.is_empty() {
            c.saved.ladder.exhausted.push(division.clone());
        }
        c.add_lower(tier, entries.into_iter().map(|e| e.puuid));
        self.save(c).await;
        Ok(Next::Again)
    }

    async fn player(&self, c: &mut Loaded, queue: Queue) -> Result<Next, Fail> {
        let ladder = &c.saved.ladder;
        let (puuid, band) = match queue {
            Queue::Top => (ladder.puuids[ladder.next_puuid].clone(), None),
            Queue::Lower => {
                let player = &ladder.lower_puuids[ladder.next_lower];
                (player.puuid.clone(), Some(player.tier.clone()))
            }
        };
        let since = now_secs().saturating_sub(DAYS * 86400);
        let path = format!("/lol/match/v5/matches/by-puuid/{}/ids?queue={QUEUE}&startTime={since}&start=0&count={IDS_PAGE}", path_segment(&puuid));
        let ids: Vec<String> = match self.json(&self.region, &path).await {
            Ok(ids) => ids.unwrap_or_default(),
            Err(err @ RiotError::Status(..)) if !other_key(&err) => {
                tracing::warn!(%err, "skipping a player");
                Vec::new()
            }
            Err(err) => return Err(err.into()),
        };
        c.reread = false;
        let max_games = self.sh.env.config.max_games;
        let mut halted = false;
        for id in ids {
            if c.older.contains(&id) {
                break;
            }
            if c.seen.contains(&id) {
                continue;
            }
            let (top, lower) = c.saved.counts();
            let held = if queue == Queue::Top { top } else { lower };
            if self.halted() || !may_take(held, top + lower, self.share) || self.sh.status.newest_games() >= max_games {
                halted = true;
                break;
            }
            match self.game(c, &id, band.as_deref()).await? {
                Game::Done => {}
                Game::EndList => break,
                Game::Halt => {
                    halted = true;
                    break;
                }
            }
        }
        if !halted {
            match queue {
                Queue::Top => c.saved.ladder.next_puuid += 1,
                Queue::Lower => c.saved.ladder.next_lower += 1,
            }
        }
        c.top_turn = queue == Queue::Lower;
        self.save(c).await;
        Ok(Next::Again)
    }

    async fn game(&self, c: &mut Loaded, id: &str, band: Option<&str>) -> Result<Game, Fail> {
        if !safe_id(id) {
            c.mark_seen(id);
            return Ok(Game::Done);
        }
        let encoded = path_segment(id);
        let Some(game) = self.bytes(&format!("/lol/match/v5/matches/{encoded}")).await? else {
            c.mark_seen(id);
            return Ok(Game::Done);
        };
        let found = game_patch(&game);
        if found.as_deref() != Some(c.patch.as_str()) {
            c.mark_seen(id);
            let older = found.is_some_and(|p| patch_number(&p) < patch_number(&c.patch));
            if older && c.older.insert(id.to_owned()) {
                c.saved.older.push(id.to_owned());
            }
            return Ok(if older { Game::EndList } else { Game::Done });
        }
        let Some(timeline) = self.bytes(&format!("/lol/match/v5/matches/{encoded}/timeline")).await? else {
            c.mark_seen(id);
            return Ok(Game::Done);
        };
        self.store_game(c, id, band, game, timeline).await
    }

    async fn store_game(&self, c: &mut Loaded, id: &str, band: Option<&str>, game: Vec<u8>, timeline: Vec<u8>) -> Result<Game, Fail> {
        let mut store = self.sh.store.lock().await;
        if store.newest.as_deref() != Some(c.patch.as_str()) || self.sh.status.newest_games() >= self.sh.env.config.max_games {
            return Ok(Game::Halt);
        }
        c.mark_seen(id);
        c.saved.stored.push(id.to_owned());
        if let Some(band) = band {
            c.saved.bands.insert(id.to_owned(), band.to_owned());
        }
        let state = serde_json::to_vec(&c.saved).map_err(|err| Fail::Disk(err.to_string()))?;
        let (raw, path, name) = (c.raw.clone(), c.path.clone(), id.to_owned());
        let written = tokio::task::spawn_blocking(move || write_game(&raw, &name, &game, &timeline, &path, &state)).await;
        if let Err(err) = written.map_err(io::Error::other).and_then(|r| r) {
            c.seen.remove(id);
            c.saved.seen.pop();
            c.saved.stored.pop();
            c.saved.bands.remove(id);
            return Err(Fail::Disk(format!("cannot store a game: {err}")));
        }
        self.publish(c);
        evict(&self.sh, &mut store).await;
        let count = c.saved.stored.len();
        if count.is_multiple_of(100) {
            tracing::info!(platform = %self.platform, games = count, players = c.saved.ladder.puuids.len(), "crawl progress");
        }
        Ok(Game::Done)
    }
}

async fn serve(ctx: Ctx) {
    let mut open: Option<Loaded> = None;
    while !ctx.halted() {
        let patch = match newest_patch(&ctx.sh).await {
            Ok(patch) => patch,
            Err(err) => {
                ctx.state(format!("waiting: Data Dragon: {err}"));
                ctx.pause(RETRY_WAIT).await;
                continue;
            }
        };
        if open.as_ref().is_none_or(|l| l.patch != patch) {
            if let Some(old) = &open {
                save_state(&old.path, &old.saved).await;
            }
            switch_patch(&ctx.sh, &patch).await;
            match Loaded::open(&ctx.sh.env.data_dir, &patch, &ctx.platform).await {
                Ok(loaded) => {
                    ctx.publish(&loaded);
                    open = Some(loaded);
                }
                Err(err) => {
                    tracing::error!(platform = %ctx.platform, %err, "crawl stopped");
                    ctx.state(format!("stopped: {err}"));
                    return;
                }
            }
        }
        let Some(loaded) = open.as_mut() else { continue };
        maybe_fit(&ctx.sh, &patch);
        match ctx.step(loaded).await {
            Next::Again => {}
            Next::Wait(wait) => ctx.pause(wait).await,
            Next::Stop(reason) => {
                tracing::error!(platform = %ctx.platform, %reason, "crawl stopped");
                ctx.state(reason);
                break;
            }
        }
    }
    if let Some(loaded) = &open {
        save_state(&loaded.path, &loaded.saved).await;
    }
}
