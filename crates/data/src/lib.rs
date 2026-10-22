//! Remote static and statistical data: Data Dragon, lolalytics, u.gg (Mayhem).
//! Every fetch goes through [`Http`], which keeps a disk cache.

pub mod ddragon;
pub mod icons;
pub mod lolalytics;
pub mod lolalytics_aram;
pub mod lolalytics_team;
pub mod qwik;
pub mod ugg;
pub mod ults;

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

/// Shared HTTP client + on-disk cache (`<cache_dir>/<hash>.json`).
#[derive(Clone)]
pub struct Http {
    pub client: reqwest::Client,
    pub cache_dir: PathBuf,
    /// One lolalytics request in flight at a time.
    polite: Arc<tokio::sync::Mutex<()>>,
}

impl Http {
    pub fn new(cache_dir: PathBuf) -> anyhow::Result<Http> {
        std::fs::create_dir_all(&cache_dir).with_context(|| format!("create {cache_dir:?}"))?;
        let client =
            reqwest::Client::builder().user_agent(USER_AGENT).gzip(true).timeout(Duration::from_secs(15)).build()?;
        Ok(Http { client, cache_dir, polite: Arc::default() })
    }

    /// GET `url` as text, served from cache if younger than `ttl`.
    /// On network failure a stale cache entry is returned if one exists.
    pub async fn get_cached(&self, url: &str, ttl: Duration) -> anyhow::Result<String> {
        self.get_parsed(url, ttl, |s| Ok(s.to_owned())).await
    }

    /// Like [`Http::get_cached`], but a body is only cached (and only served from
    /// cache) if `parse` accepts it, so error pages never poison the cache.
    pub async fn get_parsed<T>(
        &self,
        url: &str,
        ttl: Duration,
        parse: impl Fn(&str) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let path = self.cache_path(url);
        let cached = read_cache(&path).await;
        if let Some((body, age)) = &cached
            && *age < ttl
            && let Ok(v) = parse(body)
        {
            return Ok(v);
        }
        let fresh = self.fetch(url).await.and_then(|body| parse(&body).map(|v| (body, v)));
        match fresh {
            Ok((body, v)) => {
                if let Err(e) = write_cache(&path, &body).await {
                    tracing::warn!("cache write {path:?}: {e:#}");
                }
                Ok(v)
            }
            Err(e) => match cached.and_then(|(body, _)| parse(&body).ok()) {
                Some(v) => {
                    tracing::warn!("{url}: {e:#}; serving stale cache");
                    Ok(v)
                }
                None => Err(e.context(url.to_owned())),
            },
        }
    }

    pub fn cache_path(&self, url: &str) -> PathBuf {
        self.cache_dir.join(format!("{:016x}.json", fnv1a(url.as_bytes())))
    }

    async fn fetch(&self, url: &str) -> anyhow::Result<String> {
        let _turn = if url.contains("lolalytics.com") { Some(self.polite.lock().await) } else { None };
        let t = std::time::Instant::now();
        let resp = self.client.get(url).send().await?;
        tracing::debug!("GET {url} → {} in {:?}", resp.status(), t.elapsed());
        Ok(resp.error_for_status()?.text().await?)
    }
}

/// Stable 64-bit FNV-1a (std's hasher is not stable across releases).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

async fn read_cache(path: &Path) -> Option<(String, Duration)> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).unwrap_or_default();
    Some((tokio::fs::read_to_string(path).await.ok()?, age))
}

async fn write_cache(path: &Path, body: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, body).await?;
    tokio::fs::rename(&tmp, path).await
}

/// JSON numbers that lolalytics sometimes sends as strings ("52.36").
pub(crate) fn num(v: &serde_json::Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_http(name: &str) -> Http {
        let dir = std::env::temp_dir().join(format!("lol-coach-data-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Http::new(dir).unwrap()
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[tokio::test]
    async fn fresh_cache_is_served_without_network() {
        let http = tmp_http("fresh");
        let url = "http://127.0.0.1:9/never";
        write_cache(&http.cache_path(url), "hello").await.unwrap();
        assert_eq!(http.get_cached(url, Duration::from_secs(60)).await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn stale_cache_on_network_error() {
        let http = tmp_http("stale");
        let url = "http://127.0.0.1:9/never";
        write_cache(&http.cache_path(url), "old").await.unwrap();
        // ttl 0 forces a fetch, which fails (discard port), so the stale body is served.
        assert_eq!(http.get_cached(url, Duration::ZERO).await.unwrap(), "old");
        let rejected = http.get_parsed(url, Duration::ZERO, |_| -> anyhow::Result<()> { anyhow::bail!("bad") }).await;
        assert!(rejected.is_err());
    }

    #[tokio::test]
    async fn error_without_cache() {
        let http = tmp_http("none");
        assert!(http.get_cached("http://127.0.0.1:9/x", Duration::ZERO).await.is_err());
    }
}
