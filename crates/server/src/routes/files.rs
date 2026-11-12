use crate::error::{AppError, AppResult};
use crate::account;
use crate::players::{self, FileInfo, RiotId, Stored};
use crate::AppState;
use axum::body::{Body, to_bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::middleware::from_fn_with_state;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

pub fn router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/players/{player}/files", get(list))
        .route("/v1/players/{player}/files/{kind}/{name}", get(fetch).put(store).delete(remove))
        .route_layer(from_fn_with_state(state.clone(), account::require))
        .layer(DefaultBodyLimit::disable())
}

#[derive(Deserialize)]
struct ListQuery {
    kind: Option<String>,
}

async fn list(State(state): State<AppState>, Path(player): Path<String>, Query(query): Query<ListQuery>) -> AppResult<Json<Vec<FileInfo>>> {
    let id = RiotId::from_path(&player)?;
    Ok(Json(players::list(&state.config.data_dir, &id.key, query.kind.as_deref(), &state.hashes).await?))
}

async fn fetch(State(state): State<AppState>, Path((player, kind, name)): Path<(String, String, String)>) -> AppResult<Response> {
    let id = RiotId::from_path(&player)?;
    let (bytes, sha256) = players::read(&state.config.data_dir, &id.key, &kind, &name).await?;
    let etag = HeaderValue::from_str(&format!("\"{sha256}\"")).map_err(AppError::internal)?;
    Ok(([(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")), (header::ETAG, etag)], bytes).into_response())
}

async fn store(State(state): State<AppState>, Path((player, kind, name)): Path<(String, String, String)>, body: Body) -> AppResult<Json<Stored>> {
    let id = RiotId::from_path(&player)?;
    players::check_kind(&kind)?;
    players::check_name(&name)?;
    let limit = state.config.max_file_bytes();
    let bytes = to_bytes(body, limit).await.map_err(|_| AppError::TooLarge(format!("file is larger than {} MB", state.config.max_file_mb)))?;
    Ok(Json(players::write(&state.config.data_dir, &id.key, &kind, &name, bytes.into(), &state.hashes).await?))
}

async fn remove(State(state): State<AppState>, Path((player, kind, name)): Path<(String, String, String)>) -> AppResult<StatusCode> {
    let id = RiotId::from_path(&player)?;
    players::delete(&state.config.data_dir, &id.key, &kind, &name, &state.hashes).await?;
    Ok(StatusCode::NO_CONTENT)
}
