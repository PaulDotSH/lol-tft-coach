use crate::riot::region_of;
use serde::Deserialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, source: std::io::Error },
    #[error("{0}")]
    Parse(String),
    #[error("{key}: {msg}")]
    Invalid { key: &'static str, msg: String },
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub tokens: Vec<String>,
    pub riot_key_file: Option<PathBuf>,
    #[serde(default = "default_ttl_hours")]
    pub lookup_ttl_hours: u64,
    #[serde(default = "default_max_file_mb")]
    pub max_file_mb: u64,
    #[serde(default = "default_claim_max_age_hours")]
    pub claim_max_age_hours: u64,
    pub crawl: Option<CrawlConfig>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CrawlConfig {
    pub servers: Vec<String>,
    #[serde(default = "default_max_games")]
    pub max_games: u64,
    #[serde(default = "default_fit_at")]
    pub fit_at: u64,
    #[serde(default = "default_refit_every")]
    pub refit_every: u64,
    #[serde(default = "default_python")]
    pub python: String,
    pub tools_dir: PathBuf,
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8787))
}

fn default_ttl_hours() -> u64 {
    12
}

fn default_max_file_mb() -> u64 {
    32
}

fn default_claim_max_age_hours() -> u64 {
    24
}

fn default_max_games() -> u64 {
    100_000
}

fn default_fit_at() -> u64 {
    10_000
}

fn default_refit_every() -> u64 {
    2_500
}

fn default_python() -> String {
    "python3".to_owned()
}

fn parse_error(text: &str, err: &toml::de::Error) -> ConfigError {
    let message = err.message();
    let message = message.split('"').next().unwrap_or(message);
    let message = message.trim_end();
    let Some(span) = err.span() else { return ConfigError::Parse(message.to_owned()) };
    let before = &text[..span.start.min(text.len())];
    let line = before.matches('\n').count() + 1;
    match before.rsplit('\n').next().and_then(|l| l.split_once('=')) {
        Some((key, _)) => ConfigError::Parse(format!("line {line}, key `{}`: {message}", key.trim())),
        None => ConfigError::Parse(format!("line {line}: {message}")),
    }
}

fn invalid(key: &'static str, msg: impl Into<String>) -> ConfigError {
    ConfigError::Invalid { key, msg: msg.into() }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read { path: path.to_owned(), source })?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|err| parse_error(text, &err))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.data_dir.as_os_str().is_empty() {
            return Err(invalid("data_dir", "must not be empty"));
        }
        if self.tokens.is_empty() {
            return Err(invalid("tokens", "needs at least one token"));
        }
        if let Some(i) = self.tokens.iter().position(|t| t.chars().count() < 32) {
            return Err(invalid("tokens", format!("token {} has fewer than 32 characters", i + 1)));
        }
        if !(1..=1024).contains(&self.max_file_mb) {
            return Err(invalid("max_file_mb", "must be between 1 and 1024"));
        }
        if self.claim_max_age_hours == 0 {
            return Err(invalid("claim_max_age_hours", "must be above 0"));
        }
        if let Some(crawl) = &self.crawl {
            crawl.validate()?;
        }
        Ok(())
    }

    pub fn max_file_bytes(&self) -> usize {
        usize::try_from(self.max_file_mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
    }
}

impl CrawlConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.servers.is_empty() {
            return Err(invalid("crawl.servers", "needs at least one platform/region"));
        }
        let mut regions = HashSet::new();
        for server in &self.servers {
            let Some((platform, region)) = server.split_once('/') else {
                return Err(invalid("crawl.servers", format!("{server:?} is not platform/region")));
            };
            if region_of(platform) != Some(region) {
                return Err(invalid("crawl.servers", format!("{server:?}: {platform:?} is not a platform of region {region:?}")));
            }
            if !regions.insert(region) {
                return Err(invalid("crawl.servers", format!("two servers of region {region:?}; one server for each region")));
            }
        }
        for (key, value) in [("crawl.max_games", self.max_games), ("crawl.fit_at", self.fit_at), ("crawl.refit_every", self.refit_every)] {
            if value == 0 {
                return Err(invalid(key, "must be above 0"));
            }
        }
        if self.python.trim().is_empty() {
            return Err(invalid("crawl.python", "must not be empty"));
        }
        if self.tools_dir.as_os_str().is_empty() {
            return Err(invalid("crawl.tools_dir", "must not be empty"));
        }
        Ok(())
    }
}
