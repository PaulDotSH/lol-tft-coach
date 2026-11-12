use crate::AppState;
use crate::error::AppError;
use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

pub fn token_ok(tokens: &[String], presented: &str) -> bool {
    let mut ok = subtle::Choice::from(0);
    for token in tokens {
        ok |= token.as_bytes().ct_eq(presented.as_bytes());
    }
    ok.into()
}

fn bearer(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then_some(token.trim())
}

pub async fn require(State(state): State<AppState>, req: Request, next: Next) -> Result<Response, AppError> {
    let presented = req.headers().get(AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(bearer);
    match presented {
        Some(token) if token_ok(&state.config.tokens, token) => Ok(next.run(req).await),
        _ => Err(AppError::Unauthorized),
    }
}
