use crate::error::AppResult;
use crate::lookup::{self, Lookup};
use crate::players::RiotId;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/lookup/{platform}/{player}", get(show))
}

#[derive(Deserialize)]
struct LookupQuery {
    fresh: Option<String>,
}

async fn show(State(state): State<AppState>, Path((platform, player)): Path<(String, String)>, Query(query): Query<LookupQuery>) -> AppResult<Json<Lookup>> {
    let id = RiotId::from_path(&player)?;
    let fresh = matches!(query.fresh.as_deref(), Some("1" | "true"));
    Ok(Json(lookup::lookup(&state, &platform, &id, fresh).await?))
}
