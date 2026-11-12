pub mod account;
pub mod auth;
pub mod builds;
pub mod config;
pub mod crawl;
pub mod error;
pub mod features;
pub mod lookup;
pub mod npz;
pub mod players;
pub mod riot;
pub mod routes;
pub mod winprob;

use axum::Router;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub riot: Option<riot::Riot>,
    pub hashes: Arc<players::HashCache>,
    pub crawl: Option<Arc<crawl::Status>>,
    pub claims: Arc<tokio::sync::Mutex<()>>,
    pub fixed_now_ms: Option<u64>,
}

impl AppState {
    pub fn new(config: config::Config, riot: Option<riot::Riot>) -> Self {
        Self { config: Arc::new(config), riot, hashes: Arc::default(), crawl: None, claims: Arc::default(), fixed_now_ms: None }
    }

    pub fn now_ms(&self) -> u64 {
        self.fixed_now_ms.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64))
    }
}

pub fn app(state: AppState) -> Router {
    routes::router(state)
}
