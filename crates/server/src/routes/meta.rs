use crate::error::{AppError, AppResult};
use crate::{AppState, players, winprob};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/status", get(status))
}

pub fn public() -> Router<AppState> {
    Router::new().route("/v1/health", get(health)).route("/v1/winprob.toml", get(table))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn status(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let players = players::count(&state.config.data_dir).await?;
    Ok(Json(json!({ "version": env!("CARGO_PKG_VERSION"), "players": players, "riot_key": state.riot.is_some(), "crawl": state.crawl.as_ref().map(|crawl| crawl.snapshot()) })))
}

async fn table(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let bytes = winprob::newest_table(&state.config.data_dir).await?.ok_or(AppError::NotFound)?;
    Ok(([(header::CONTENT_TYPE, "application/toml")], bytes))
}
