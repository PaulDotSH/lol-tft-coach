//! Keeps this account's files on the minipc service (`crates/server`): the proof that the account
//! is the player's (the claim), then a two-way copy of `history`, `recaps` and `ranked` and a
//! settings copy for a new PC. The local files stay the working copy; everything here runs in
//! its own task and never while a game is on.

use crate::account;
use crate::state::Shared;
use model::Phase;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EVERY: Duration = Duration::from_secs(600);
const LOOK_EVERY: Duration = Duration::from_secs(5);
const CLAIM_RETRY: Duration = Duration::from_secs(30);
const CLAIM_GIVE_UP: Duration = Duration::from_secs(600);
const CLAIM_RECHECK: Duration = Duration::from_secs(60);
const CLAIM_MAX_AGE_MS: u64 = 24 * 3_600_000;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TIMEOUT: Duration = Duration::from_secs(60);
const SETTINGS: &str = "settings";
const SETTINGS_FILE: &str = "settings.toml";
const SECRET_SETTINGS: [&str; 2] = ["server_url", "server_token"];
const RANKED: (&str, &str) = ("ranked", "snapshots.json");

/// Why a request failed. The text holds neither the token nor the account key.
#[derive(Debug)]
pub enum Fail {
    Status(u16, String),
    Net(String),
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Status(code, msg) if msg.is_empty() => write!(f, "the service answered {code}"),
            Fail::Status(code, msg) => write!(f, "the service answered {code}: {msg}"),
            Fail::Net(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for Fail {}

/// One file as the service lists it, or as found on this PC.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Item {
    pub kind: String,
    pub name: String,
    pub sha256: String,
    pub modified: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub up: Vec<(String, String)>,
    pub down: Vec<(String, String)>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub sent: usize,
    pub received: usize,
    pub failed: usize,
    pub paused: bool,
}

fn valid_name(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=128).contains(&b.len()) && b[0].is_ascii_alphanumeric() && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn wanted(kind: &str, name: &str) -> bool {
    match kind {
        SETTINGS => name == SETTINGS_FILE,
        k => account::KINDS.contains(&k) && valid_name(name),
    }
}

/// What goes up and what comes down. Same hash: nothing. Only here: up. Only there: down. Both,
/// different: the newer wins (this PC's modified time against the service's, which is when that
/// version was uploaded) and a tie is left alone. Settings are upload only; one comes down only
/// where this PC has none. Nothing is ever deleted.
pub fn plan(local: &[Item], remote: &[Item]) -> Plan {
    let by = |items: &'_ [Item]| -> BTreeMap<(String, String), Item> {
        items.iter().filter(|i| wanted(&i.kind, &i.name)).map(|i| ((i.kind.clone(), i.name.clone()), i.clone())).collect()
    };
    let (here, there) = (by(local), by(remote));
    let mut plan = Plan::default();
    for key in here.keys().chain(there.keys()).collect::<std::collections::BTreeSet<_>>() {
        match (here.get(key), there.get(key)) {
            (Some(a), Some(b)) if a.sha256 == b.sha256 => {}
            (Some(_), Some(_)) if key.0 == SETTINGS => plan.up.push(key.clone()),
            (Some(a), Some(b)) if a.modified > b.modified => plan.up.push(key.clone()),
            (Some(a), Some(b)) if a.modified < b.modified => plan.down.push(key.clone()),
            (Some(_), Some(_)) => {}
            (Some(_), None) => plan.up.push(key.clone()),
            (None, Some(_)) => plan.down.push(key.clone()),
            (None, None) => {}
        }
    }
    plan
}

/// Two rank logs as one: every snapshot of either, once, by time. None when either is not a list
/// of snapshots. The log is the one file both PCs keep writing, so the newer must not replace
/// the other.
pub fn merge_snapshots(a: &[u8], b: &[u8]) -> Option<Vec<u8>> {
    let (a, b): (Vec<Value>, Vec<Value>) = (serde_json::from_slice(a).ok()?, serde_json::from_slice(b).ok()?);
    let mut all = BTreeMap::new();
    for snap in a.into_iter().chain(b) {
        let key = (snap["time"].as_u64()?, snap["queue"].as_str()?.to_owned());
        all.entry(key).or_insert(snap);
    }
    serde_json::to_vec(&all.into_values().collect::<Vec<_>>()).ok()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// `settings.toml` as it goes to the service: without the address and the token.
pub fn settings_copy(text: &str) -> Option<String> {
    let mut table: toml::Table = text.parse().ok()?;
    for key in SECRET_SETTINGS {
        table.remove(key);
    }
    toml::to_string(&table).ok()
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Hashes of local files, kept by size and modified time.
#[derive(Default)]
pub struct HashCache {
    seen: Mutex<HashMap<PathBuf, (u64, SystemTime, String)>>,
    hashed: std::sync::atomic::AtomicUsize,
}

impl HashCache {
    #[cfg(test)]
    pub fn hashed(&self) -> usize {
        self.hashed.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn hash(&self, path: &Path, size: u64, modified: SystemTime) -> std::io::Result<String> {
        if let Some((s, m, sha)) = self.seen.lock().unwrap().get(path)
            && *s == size
            && *m == modified
        {
            return Ok(sha.clone());
        }
        let sha = sha256_hex(&std::fs::read(path)?);
        self.hashed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.seen.lock().unwrap().insert(path.to_owned(), (size, modified, sha.clone()));
        Ok(sha)
    }
}

/// This account's files: the regular files of its three directories whose names the service
/// accepts and that are small enough, and the settings copy.
pub fn local_items(player_dir: &Path, settings_path: &Path, cache: &HashCache) -> Vec<Item> {
    let mut out = Vec::new();
    for kind in account::KINDS {
        let Ok(rd) = std::fs::read_dir(player_dir.join(kind)) else { continue };
        for entry in rd.filter_map(|e| e.ok()) {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.len() > MAX_FILE_BYTES || !valid_name(&name) {
                continue;
            }
            let Some(modified) = meta.modified().ok() else { continue };
            if let Ok(sha256) = cache.hash(&entry.path(), meta.len(), modified) {
                out.push(Item { kind: kind.into(), name, sha256, modified: unix_secs(modified) });
            }
        }
    }
    if let Some(copy) = std::fs::read_to_string(settings_path).ok().and_then(|t| settings_copy(&t)) {
        let modified = std::fs::metadata(settings_path).and_then(|m| m.modified()).map_or(0, unix_secs);
        out.push(Item { kind: SETTINGS.into(), name: SETTINGS_FILE.into(), sha256: sha256_hex(copy.as_bytes()), modified });
    }
    out
}

/// A game of my match history that can prove the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimGame {
    pub platform: String,
    pub game_id: u64,
    pub champion_id: u32,
    pub win: bool,
    pub duration_s: u64,
    pub ended_ms: u64,
}

/// The newest matchmade game of a `/matches` reply.
pub fn newest_game(list: &Value) -> Option<ClaimGame> {
    let game = clients::history::list_games(list).filter(|g| clients::history::wanted(g)).max_by_key(|g| g["gameCreation"].as_u64().unwrap_or(0))?;
    let me = game["participants"].as_array()?.first()?;
    let duration_s = game["gameDuration"].as_u64()?;
    let duration_s = if duration_s > 20_000 { duration_s / 1000 } else { duration_s };
    Some(ClaimGame {
        platform: game["platformId"].as_str()?.to_ascii_lowercase(),
        game_id: game["gameId"].as_u64()?,
        champion_id: u32::try_from(me["championId"].as_u64()?).ok()?,
        win: me.pointer("/stats/win")?.as_bool()?,
        duration_s,
        ended_ms: game["gameCreation"].as_u64()? + duration_s * 1000,
    })
}

impl ClaimGame {
    pub fn recent(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.ended_ms) < CLAIM_MAX_AGE_MS
    }
}

#[derive(Serialize)]
pub struct ClaimBody<'a> {
    pub platform: &'a str,
    pub game_id: u64,
    pub champion_id: u32,
    pub win: bool,
    pub duration_s: u64,
    pub account_key: &'a str,
}

pub enum Claimed {
    Accepted,
    Later,
}

pub struct Remote {
    http: reqwest::Client,
    base: String,
    player: String,
    token: String,
    key: String,
}

impl Remote {
    /// `base` without a trailing slash; `riot_id` as the client gives it.
    pub fn new(base: &str, token: &str, riot_id: &str, key: &str) -> Result<Remote, Fail> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("lol-coach")
            .build()
            .map_err(|e| Fail::Net(e.to_string()))?;
        Ok(Remote { http, base: base.trim_end_matches('/').to_owned(), player: account::url_player(riot_id), token: token.to_owned(), key: key.to_owned() })
    }

    fn redact(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for secret in [&self.token, &self.key] {
            if !secret.is_empty() {
                out = out.replace(secret.as_str(), "***");
            }
        }
        out
    }

    fn net(&self, e: reqwest::Error) -> Fail {
        let mut text = e.to_string();
        let mut source = std::error::Error::source(&e);
        while let Some(s) = source {
            text.push_str(": ");
            text.push_str(&s.to_string());
            source = s.source();
        }
        Fail::Net(self.redact(&text))
    }

    async fn call(&self, req: reqwest::RequestBuilder, with_key: bool) -> Result<reqwest::Response, Fail> {
        let req = req.bearer_auth(&self.token);
        let req = if with_key { req.header("X-Account-Key", &self.key) } else { req };
        let res = req.send().await.map_err(|e| self.net(e))?;
        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        let body = res.text().await.unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| v["error"].as_str().map(str::to_owned)).unwrap_or_default();
        Err(Fail::Status(status.as_u16(), self.redact(&msg.chars().take(200).collect::<String>())))
    }

    fn files_url(&self, rest: &str) -> String {
        format!("{}/v1/players/{}/files{rest}", self.base, self.player)
    }

    pub async fn list(&self) -> Result<Vec<Item>, Fail> {
        let res = self.call(self.http.get(self.files_url("")), true).await?;
        res.json().await.map_err(|e| self.net(e))
    }

    pub async fn fetch(&self, kind: &str, name: &str) -> Result<Vec<u8>, Fail> {
        let res = self.call(self.http.get(self.files_url(&format!("/{kind}/{name}"))), true).await?;
        res.bytes().await.map(|b| b.to_vec()).map_err(|e| self.net(e))
    }

    pub async fn store(&self, kind: &str, name: &str, body: Vec<u8>) -> Result<(), Fail> {
        self.call(self.http.put(self.files_url(&format!("/{kind}/{name}"))).body(body), true).await.map(|_| ())
    }

    pub async fn claim(&self, game: &ClaimGame) -> Result<Claimed, Fail> {
        let body = ClaimBody { platform: &game.platform, game_id: game.game_id, champion_id: game.champion_id, win: game.win, duration_s: game.duration_s, account_key: &self.key };
        let url = format!("{}/v1/players/{}/claim", self.base, self.player);
        let res = self.call(self.http.post(url).json(&body), false).await?;
        Ok(if res.status().as_u16() == 202 { Claimed::Later } else { Claimed::Accepted })
    }
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (dir, name) = (path.parent().unwrap_or(Path::new(".")), path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    tokio::fs::create_dir_all(dir).await?;
    let part = dir.join(format!(".{name}.part"));
    tokio::fs::write(&part, bytes).await?;
    if let Err(e) = tokio::fs::rename(&part, path).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    Ok(())
}

async fn upload(remote: &Remote, player_dir: &Path, settings_path: &Path, kind: &str, name: &str) -> Result<(), Fail> {
    let bytes = if kind == SETTINGS {
        std::fs::read_to_string(settings_path).ok().and_then(|t| settings_copy(&t)).map(String::into_bytes)
    } else {
        tokio::fs::read(player_dir.join(kind).join(name)).await.ok()
    };
    let bytes = bytes.ok_or_else(|| Fail::Net(format!("{kind}/{name} cannot be read")))?;
    remote.store(kind, name, bytes).await
}

async fn download(remote: &Remote, player_dir: &Path, settings_path: &Path, item: &Item) -> Result<(), Fail> {
    let bytes = remote.fetch(&item.kind, &item.name).await?;
    if sha256_hex(&bytes) != item.sha256 {
        return Err(Fail::Net(format!("{}/{} arrived damaged", item.kind, item.name)));
    }
    let path = if item.kind == SETTINGS { settings_path.to_owned() } else { player_dir.join(&item.kind).join(&item.name) };
    write_atomic(&path, &bytes).await.map_err(|e| Fail::Net(format!("{}/{} not saved: {e}", item.kind, item.name)))
}

/// One sync. `stop` is asked before each file: when it says true the round ends there.
pub async fn round(remote: &Remote, player_dir: &Path, settings_path: &Path, cache: &Arc<HashCache>, stop: &(dyn Fn() -> bool + Sync)) -> Result<Report, Fail> {
    let there = remote.list().await?;
    let (pd, sp, c) = (player_dir.to_owned(), settings_path.to_owned(), cache.clone());
    let here = tokio::task::spawn_blocking(move || local_items(&pd, &sp, &c)).await.map_err(|e| Fail::Net(e.to_string()))?;
    let mut plan = plan(&here, &there);
    let mut report = Report::default();
    let ranked = |i: &&Item| (i.kind.as_str(), i.name.as_str()) == RANKED;
    if let (Some(mine), Some(theirs)) = (here.iter().find(ranked), there.iter().find(ranked))
        && mine.sha256 != theirs.sha256
        && !stop()
    {
        let path = player_dir.join(RANKED.0).join(RANKED.1);
        let theirs = remote.fetch(RANKED.0, RANKED.1).await?;
        let mine = tokio::fs::read(&path).await.map_err(|e| Fail::Net(format!("{}/{} cannot be read: {e}", RANKED.0, RANKED.1)))?;
        if let Some(both) = merge_snapshots(&mine, &theirs) {
            plan.up.retain(|(k, n)| (k.as_str(), n.as_str()) != RANKED);
            plan.down.retain(|(k, n)| (k.as_str(), n.as_str()) != RANKED);
            if both != mine {
                write_atomic(&path, &both).await.map_err(|e| Fail::Net(format!("{}/{} not saved: {e}", RANKED.0, RANKED.1)))?;
                report.received += 1;
            }
            if both != theirs {
                remote.store(RANKED.0, RANKED.1, both).await?;
                report.sent += 1;
            }
        }
    }
    let steps = plan.up.iter().map(|k| (true, k)).chain(plan.down.iter().map(|k| (false, k)));
    for (up, (kind, name)) in steps {
        if stop() {
            report.paused = true;
            break;
        }
        let done = if up {
            upload(remote, player_dir, settings_path, kind, name).await
        } else {
            match there.iter().find(|i| i.kind == *kind && i.name == *name) {
                Some(item) => download(remote, player_dir, settings_path, item).await,
                None => continue,
            }
        };
        match done {
            Ok(()) if up => report.sent += 1,
            Ok(()) => report.received += 1,
            Err(e @ (Fail::Net(_) | Fail::Status(401 | 403, _))) => return Err(e),
            Err(e) => {
                tracing::info!("sync of {kind}/{name}: {e}");
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

fn running(phase: Phase) -> bool {
    matches!(phase, Phase::ChampSelect | Phase::Loading | Phase::InGame)
}

#[cfg(windows)]
fn clock() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:02}:{:02}", t.wHour, t.wMinute)
}

#[cfg(not(windows))]
fn clock() -> String {
    let s = unix_secs(SystemTime::now()) % 86_400;
    format!("{:02}:{:02} UTC", s / 3600, s % 3600 / 60)
}

fn sentence(r: &Report) -> String {
    let failed = if r.failed > 0 { format!(", {} could not be copied", r.failed) } else { String::new() };
    format!("Synced at {}: {} sent, {} received{failed}.", clock(), r.sent, r.received)
}

#[derive(Default)]
struct Claim {
    tried: HashSet<u64>,
    waiting: Option<(ClaimGame, Instant)>,
    next: Option<Instant>,
}

#[derive(Default)]
struct Worker {
    seen: Option<(String, String, String)>,
    last_round: Option<Instant>,
    claim: Claim,
    cache: Arc<HashCache>,
    last_phase: Option<Phase>,
}

fn say(sh: &Shared, text: impl Into<String>) {
    let text = text.into();
    let mut line = sh.sync_status.lock().unwrap();
    if *line != text {
        tracing::info!("sync: {text}");
        *line = text;
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub async fn run(sh: Arc<Shared>) {
    let mut worker = Worker::default();
    loop {
        worker.step(&sh).await;
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

impl Worker {
    async fn step(&mut self, sh: &Arc<Shared>) {
        let settings = sh.settings();
        let (base, token) = (settings.server_url.trim().trim_end_matches('/').to_owned(), settings.server_token.trim().to_owned());
        if base.is_empty() {
            return say(sh, "Off: no server address set.");
        }
        if token.is_empty() {
            return say(sh, "Off: no access token set.");
        }
        let phase = sh.ui.lock().unwrap().phase;
        let after_game = phase == Phase::PostGame && self.last_phase != Some(Phase::PostGame);
        self.last_phase = Some(phase);
        if running(phase) {
            return;
        }
        let Some(player) = account::global().named() else {
            return say(sh, "Waiting for the League client to say which account this is.");
        };
        let id = (base.clone(), token.clone(), player.key.clone());
        if self.seen.as_ref() != Some(&id) {
            self.seen = Some(id);
            self.last_round = None;
            self.claim = Claim::default();
        }
        let dir = account::global().player_dir(&player.key);
        let accepted = account::accepted(&dir, &base);
        let waiting = if accepted { self.last_round.is_some_and(|t| t.elapsed() < EVERY) } else { !after_game && self.claim.next.is_some_and(|t| Instant::now() < t) };
        if waiting {
            return;
        }
        let key = match account::account_key(&dir) {
            Ok(key) => key,
            Err(e) => return say(sh, format!("Could not make this PC's account key: {e}")),
        };
        let remote = match Remote::new(&base, &token, &player.riot_id, &key) {
            Ok(r) => r,
            Err(e) => return say(sh, format!("Could not sync: {e}")),
        };
        if !accepted {
            if after_game {
                self.claim.next = None;
            }
            if !self.prove(sh, &remote, &dir, &base).await {
                return;
            }
        }
        if self.last_round.is_some_and(|t| t.elapsed() < EVERY) {
            return;
        }
        self.last_round = Some(Instant::now());
        let settings_path = crate::settings::config_dir().join(SETTINGS_FILE);
        let stop = || running(sh.ui.lock().unwrap().phase);
        match round(&remote, &dir, &settings_path, &self.cache, &stop).await {
            Ok(r) if r.paused => {
                self.last_round = None;
            }
            Ok(r) => {
                if r.received > 0 {
                    account::global().reread();
                }
                say(sh, sentence(&r))
            }
            Err(Fail::Status(403, _)) => {
                let _ = std::fs::remove_file(dir.join("account.accepted"));
                self.claim = Claim::default();
                say(sh, "The service no longer knows this PC's key. A finished game will prove the account again.");
            }
            Err(Fail::Status(401, _)) => say(sh, "The service refused the access token."),
            Err(e) => say(sh, format!("Could not sync: {e}")),
        }
    }

    /// Sends the claim for the newest game when one is due; true when the service has the key.
    async fn prove(&mut self, sh: &Arc<Shared>, remote: &Remote, dir: &Path, base: &str) -> bool {
        let now = Instant::now();
        if self.claim.next.is_some_and(|t| now < t) {
            return false;
        }
        let game = match &self.claim.waiting {
            Some((game, _)) => game.clone(),
            None => {
                self.claim.next = Some(now + CLAIM_RECHECK);
                let lcu = sh.lcu.lock().unwrap().clone();
                let Some(lcu) = lcu else {
                    say(sh, "Waiting for a finished game to prove the account (the League client is not running).");
                    return false;
                };
                let path = format!("/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex={}", clients::history::RECENT);
                let found = lcu.get(&path).await.ok().flatten().as_ref().and_then(newest_game);
                match found {
                    Some(g) if self.claim.tried.contains(&g.game_id) => return false,
                    Some(g) if g.recent(now_ms()) => g,
                    _ => {
                        say(sh, "Waiting for a game to finish: the service wants a game from the last 24 hours to prove the account.");
                        return false;
                    }
                }
            }
        };
        match remote.claim(&game).await {
            Ok(Claimed::Accepted) => {
                if let Err(e) = account::mark_accepted(dir, base) {
                    tracing::warn!("account acceptance not saved: {e}");
                }
                self.claim = Claim::default();
                say(sh, "The service accepted this account.");
                true
            }
            Ok(Claimed::Later) => {
                let since = self.claim.waiting.as_ref().map_or(now, |w| w.1);
                if since.elapsed() >= CLAIM_GIVE_UP {
                    self.claim.tried.insert(game.game_id);
                    self.claim.waiting = None;
                    self.claim.next = Some(now + CLAIM_RECHECK);
                    say(sh, "Riot did not list the finished game in time. The next finished game will prove the account.");
                } else {
                    self.claim.waiting = Some((game, since));
                    self.claim.next = Some(now + CLAIM_RETRY);
                    say(sh, "Waiting for Riot to list the finished game.");
                }
                false
            }
            Err(Fail::Status(code @ (403 | 404 | 409), _)) => {
                self.claim.tried.insert(game.game_id);
                self.claim.waiting = None;
                self.claim.next = Some(now + CLAIM_RECHECK);
                let why = match code {
                    403 => "the game does not match this player",
                    409 => "that game was used for a claim before",
                    _ => "Riot does not know this player",
                };
                say(sh, format!("The service did not accept this account: {why}. The next finished game will be tried."));
                false
            }
            Err(Fail::Status(401, _)) => {
                self.claim.next = Some(now + CLAIM_RECHECK);
                say(sh, "The service refused the access token.");
                false
            }
            Err(e) => {
                self.claim.next = Some(now + CLAIM_RECHECK);
                say(sh, format!("Could not prove the account: {e}"));
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
