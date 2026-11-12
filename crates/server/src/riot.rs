use crate::players::{percent_encode, sha256_hex};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{Instant, sleep};

pub const DEFAULT_BASE: &str = "https://{host}.api.riotgames.com";
pub const DEFAULT_WINDOWS: [(u32, u64); 2] = [(20, 1), (100, 120)];
const MAX_RETRIES: u32 = 3;
const LOOKUP_POLL: Duration = Duration::from_millis(20);

pub fn region_of(platform: &str) -> Option<&'static str> {
    match platform {
        "br1" | "la1" | "la2" | "na1" => Some("americas"),
        "euw1" | "eun1" | "tr1" | "ru" | "me1" => Some("europe"),
        "kr" | "jp1" => Some("asia"),
        "oc1" | "sg2" | "tw2" | "vn2" => Some("sea"),
        _ => None,
    }
}

pub fn parse_limits(header: &str) -> Vec<(u32, u64)> {
    header
        .split(',')
        .filter_map(|part| {
            let (limit, secs) = part.trim().split_once(':')?;
            let (limit, secs) = (limit.trim().parse::<u32>().ok()?, secs.trim().parse::<u64>().ok()?);
            (limit > 0 && secs > 0).then_some((limit, secs))
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    Lookup,
    Crawl,
}

struct Windows {
    limits: Vec<(u32, u64)>,
    sent: VecDeque<Instant>,
    held_until: Option<Instant>,
    last_crawl: Option<Instant>,
}

pub struct Limiter {
    state: Mutex<Windows>,
    lookups_waiting: AtomicUsize,
}

struct Waiting<'a>(&'a AtomicUsize);

impl<'a> Waiting<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Windows {
    fn longest(&self) -> (u32, u64) {
        self.limits.iter().copied().max_by_key(|&(_, secs)| secs).unwrap_or(DEFAULT_WINDOWS[1])
    }

    fn delay(&mut self, now: Instant, crawl: bool) -> Duration {
        let longest = Duration::from_secs(self.longest().1);
        while self.sent.front().is_some_and(|&t| t + longest <= now) {
            self.sent.pop_front();
        }
        let mut wait = self.held_until.map_or(Duration::ZERO, |h| h.saturating_duration_since(now));
        for &(limit, secs) in &self.limits {
            let window = Duration::from_secs(secs);
            let inside = self.sent.len() - self.sent.partition_point(|&t| t + window <= now);
            if inside >= limit as usize {
                let oldest = self.sent[self.sent.len() - limit as usize];
                wait = wait.max((oldest + window).saturating_duration_since(now));
            }
        }
        if let (true, Some(last)) = (crawl, self.last_crawl) {
            let (limit, secs) = self.longest();
            wait = wait.max((last + Duration::from_secs_f64(secs as f64 / limit as f64)).saturating_duration_since(now));
        }
        wait
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Limiter {
    pub fn new() -> Self {
        let state = Windows { limits: DEFAULT_WINDOWS.to_vec(), sent: VecDeque::new(), held_until: None, last_crawl: None };
        Self { state: Mutex::new(state), lookups_waiting: AtomicUsize::new(0) }
    }

    pub fn set_windows(&self, limits: Vec<(u32, u64)>) {
        if !limits.is_empty() {
            self.state.lock().unwrap().limits = limits;
        }
    }

    pub fn windows(&self) -> Vec<(u32, u64)> {
        self.state.lock().unwrap().limits.clone()
    }

    pub fn hold(&self, duration: Duration) {
        let until = Instant::now() + duration;
        let mut state = self.state.lock().unwrap();
        state.held_until = Some(state.held_until.map_or(until, |h| h.max(until)));
    }

    pub async fn acquire(&self, priority: Priority) {
        let crawl = priority == Priority::Crawl;
        let _waiting = (!crawl).then(|| Waiting::new(&self.lookups_waiting));
        loop {
            if crawl && self.lookups_waiting.load(Ordering::SeqCst) > 0 {
                sleep(LOOKUP_POLL).await;
                continue;
            }
            let wait = {
                let mut state = self.state.lock().unwrap();
                let now = Instant::now();
                let wait = state.delay(now, crawl);
                if wait.is_zero() {
                    state.sent.push_back(now);
                    if crawl {
                        state.last_crawl = Some(now);
                    }
                    return;
                }
                wait
            };
            sleep(wait).await;
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RiotError {
    #[error("not found")]
    NotFound,
    #[error("riot key rejected")]
    KeyRejected,
    #[error("riot answered {0}: {1}")]
    Status(u16, String),
    #[error("riot unavailable: {0}")]
    Unavailable(String),
}

#[derive(Clone)]
pub struct Riot {
    client: reqwest::Client,
    base: Arc<str>,
    key_id: Arc<str>,
    limiters: Arc<Mutex<HashMap<String, Arc<Limiter>>>>,
    pub retry_base: Duration,
}

impl Riot {
    pub fn new(key: &str) -> Result<Self, String> {
        Self::with_base(key, DEFAULT_BASE)
    }

    pub fn with_base(key: &str, base: &str) -> Result<Self, String> {
        let mut value = reqwest::header::HeaderValue::from_str(key.trim()).map_err(|_| "the Riot key is not a valid header value".to_owned())?;
        value.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-Riot-Token", value);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("lol-coach-server/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| err.to_string())?;
        let key_id = sha256_hex(key.trim().as_bytes())[..16].to_owned();
        Ok(Self { client, base: base.into(), key_id: key_id.into(), limiters: Arc::default(), retry_base: Duration::from_millis(500) })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn limiter(&self, host: &str) -> Arc<Limiter> {
        self.limiters.lock().unwrap().entry(host.to_owned()).or_default().clone()
    }

    pub fn url(&self, host: &str, path: &str) -> String {
        format!("{}{path}", self.base.replace("{host}", host))
    }

    async fn backoff(&self, failures: u32) {
        sleep(self.retry_base * 2u32.pow(failures - 1)).await;
    }

    pub async fn get<T: DeserializeOwned>(&self, host: &str, path: &str, priority: Priority) -> Result<T, RiotError> {
        let limiter = self.limiter(host);
        let url = self.url(host, path);
        let (mut failures, mut limited) = (0u32, 0u32);
        loop {
            limiter.acquire(priority).await;
            let resp = match self.client.get(&url).send().await {
                Ok(resp) => resp,
                Err(err) => {
                    failures += 1;
                    if failures > MAX_RETRIES {
                        return Err(RiotError::Unavailable(err.without_url().to_string()));
                    }
                    self.backoff(failures).await;
                    continue;
                }
            };
            if let Some(header) = resp.headers().get("x-app-rate-limit").and_then(|v| v.to_str().ok()) {
                limiter.set_windows(parse_limits(header));
            }
            let status = resp.status().as_u16();
            match status {
                200..=299 => {
                    let bytes = resp.bytes().await.map_err(|err| RiotError::Unavailable(err.without_url().to_string()))?;
                    return serde_json::from_slice(&bytes).map_err(|err| RiotError::Unavailable(format!("unreadable reply: {err}")));
                }
                404 => return Err(RiotError::NotFound),
                401 | 403 => return Err(RiotError::KeyRejected),
                429 => {
                    limited += 1;
                    let seconds = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(2);
                    limiter.hold(Duration::from_secs(seconds));
                    if limited > MAX_RETRIES {
                        return Err(RiotError::Unavailable("rate limited".to_owned()));
                    }
                }
                500..=599 => {
                    failures += 1;
                    if failures > MAX_RETRIES {
                        return Err(RiotError::Unavailable(format!("status {status}")));
                    }
                    self.backoff(failures).await;
                }
                _ => {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(RiotError::Status(status, body.chars().take(500).collect()));
                }
            }
        }
    }
}

pub fn path_segment(s: &str) -> String {
    percent_encode(s, |b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
}
