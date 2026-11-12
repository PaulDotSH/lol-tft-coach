mod files;
mod lookup;
mod meta;

use crate::{AppState, auth, error::AppError};
use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use std::time::Duration;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .merge(files::router())
        .merge(lookup::router())
        .merge(meta::router())
        .route_layer(from_fn_with_state(state.clone(), auth::require));
    Router::new()
        .route("/v1/health", get(meta::health))
        .merge(protected)
        .fallback(|| async { AppError::NotFound })
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(90)))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
