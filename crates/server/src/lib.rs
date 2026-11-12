pub mod auth;
pub mod config;
pub mod error;
pub mod lookup;
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
}

pub fn app(state: AppState) -> Router {
    routes::router(state)
}
