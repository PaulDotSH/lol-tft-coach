//! Remote static and statistical data: Data Dragon, lolalytics, u.gg (Mayhem).
//! Every fetch goes through [`Http`], which keeps a disk cache.

pub mod ddragon;
pub mod lolalytics;
pub mod ugg;

use std::path::PathBuf;
use std::time::Duration;

/// Shared HTTP client + on-disk cache (`<cache_dir>/<hash>.json`).
#[derive(Clone)]
pub struct Http {
    pub client: reqwest::Client,
    pub cache_dir: PathBuf,
}

impl Http {
    pub fn new(cache_dir: PathBuf) -> anyhow::Result<Http> {
        todo!("{cache_dir:?}")
    }

    /// GET `url` as text, served from cache if younger than `ttl`.
    /// On network failure a stale cache entry is returned if one exists.
    pub async fn get_cached(&self, url: &str, ttl: Duration) -> anyhow::Result<String> {
        todo!("{url} {ttl:?}")
    }
}
