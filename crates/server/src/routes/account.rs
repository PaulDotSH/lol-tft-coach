use crate::error::AppResult;
use crate::players::RiotId;
use crate::{AppState, account};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::Value;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/players/{player}/claim", post(claim))
}

async fn claim(State(state): State<AppState>, Path(player): Path<String>, body: Bytes) -> AppResult<(StatusCode, Json<Value>)> {
    let id = RiotId::from_path(&player)?;
    account::claim(&state, &id, &body).await
}
