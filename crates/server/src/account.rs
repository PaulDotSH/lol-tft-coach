use crate::AppState;
use crate::auth::token_ok;
use crate::error::{AppError, AppResult};
use crate::lookup::{puuid_of, riot_failure};
use crate::players::{ACCOUNT_FILE, RiotId, player_dir, sha256_hex, write_atomic};
use crate::riot::{Priority, RiotError, path_segment, region_of};
use axum::extract::{RawPathParams, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const KEY_HEX_CHARS: usize = 64;
const DURATION_SLACK_S: u64 = 10;
const KEY_HEADER: &str = "x-account-key";
const WRONG_KEY: &str = "missing or wrong account key";
const NO_PROOF: &str = "the match does not prove this account";

#[derive(Serialize, Deserialize, Default)]
pub struct Account {
    keys: Vec<String>,
    matches: Vec<String>,
}

#[derive(Deserialize)]
pub struct Claim {
    platform: String,
    game_id: u64,
    champion_id: u32,
    win: bool,
    duration_s: u64,
    account_key: String,
}

#[derive(Deserialize)]
struct Match {
    metadata: Metadata,
    info: Info,
}

#[derive(Deserialize)]
struct Metadata {
    participants: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    game_duration: u64,
    game_end_timestamp: u64,
    participants: Vec<Participant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Participant {
    puuid: String,
    champion_id: u32,
    win: bool,
}

pub fn valid_key(key: &str) -> bool {
    key.len() == KEY_HEX_CHARS && key.bytes().all(|b| b.is_ascii_hexdigit())
}

fn account_path(root: &Path, player: &str) -> PathBuf {
    player_dir(root, player).join(ACCOUNT_FILE)
}

async fn read(root: &Path, player: &str) -> AppResult<Account> {
    match tokio::fs::read(account_path(root, player)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(AppError::internal),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Account::default()),
        Err(err) => Err(err.into()),
    }
}

fn holds(account: &Account, key: &str) -> bool {
    valid_key(key) && token_ok(&account.keys, &sha256_hex(key.as_bytes()))
}

pub async fn require(State(state): State<AppState>, params: RawPathParams, req: Request, next: Next) -> Result<Response, AppError> {
    let wrong = || AppError::Forbidden(WRONG_KEY.to_owned());
    let (_, player) = params.iter().find(|(name, _)| *name == "player").ok_or_else(wrong)?;
    let id = RiotId::from_path(player)?;
    let presented = req.headers().get(KEY_HEADER).and_then(|v| v.to_str().ok()).ok_or_else(wrong)?;
    let account = read(&state.config.data_dir, &id.key).await?;
    if !holds(&account, presented) {
        return Err(wrong());
    }
    Ok(next.run(req).await)
}

fn proves(found: &Match, puuid: &str, claim: &Claim, now_ms: u64, max_age_ms: u64) -> bool {
    let listed = found.metadata.participants.iter().any(|p| p == puuid);
    let Some(me) = found.info.participants.iter().find(|p| p.puuid == puuid) else { return false };
    listed
        && me.champion_id == claim.champion_id
        && me.win == claim.win
        && found.info.game_duration.abs_diff(claim.duration_s) <= DURATION_SLACK_S
        && now_ms.saturating_sub(found.info.game_end_timestamp) <= max_age_ms
}

fn claimed() -> (StatusCode, axum::Json<Value>) {
    (StatusCode::OK, axum::Json(json!({ "claimed": true })))
}

pub async fn claim(state: &AppState, id: &RiotId, body: &[u8]) -> AppResult<(StatusCode, axum::Json<Value>)> {
    let claim: Claim = serde_json::from_slice(body).map_err(|_| AppError::bad_request("the claim is not the expected JSON"))?;
    if !valid_key(&claim.account_key) {
        return Err(AppError::bad_request("account_key must be 64 hex characters"));
    }
    let platform = claim.platform.to_ascii_lowercase();
    let region = region_of(&platform).ok_or_else(|| AppError::bad_request("unknown platform"))?;
    let root = &state.config.data_dir;
    if holds(&read(root, &id.key).await?, &claim.account_key) {
        return Ok(claimed());
    }
    let riot = state.riot.as_ref().ok_or_else(|| AppError::Unavailable("claims are off: no Riot key".to_owned()))?;
    let puuid = match puuid_of(riot, region, id).await {
        Ok(puuid) => puuid,
        Err(RiotError::NotFound) => return Err(AppError::NotFound),
        Err(err) => return Err(riot_failure(err)),
    };
    let match_id = format!("{}_{}", platform.to_ascii_uppercase(), claim.game_id);
    let found: Match = match riot.get(region, &format!("/lol/match/v5/matches/{}", path_segment(&match_id)), Priority::Lookup).await {
        Ok(found) => found,
        Err(RiotError::NotFound) => return Ok((StatusCode::ACCEPTED, axum::Json(json!({ "claimed": false, "retry": true })))),
        Err(err) => return Err(riot_failure(err)),
    };
    let max_age_ms = state.config.claim_max_age_hours.saturating_mul(3_600_000);
    if !proves(&found, &puuid, &claim, state.now_ms(), max_age_ms) {
        return Err(AppError::Forbidden(NO_PROOF.to_owned()));
    }
    let _guard = state.claims.lock().await;
    let mut account = read(root, &id.key).await?;
    if holds(&account, &claim.account_key) {
        return Ok(claimed());
    }
    if account.matches.contains(&match_id) {
        return Err(AppError::Conflict("this match was used for a claim before".to_owned()));
    }
    account.keys.push(sha256_hex(claim.account_key.as_bytes()));
    account.matches.push(match_id);
    let (path, bytes) = (account_path(root, &id.key), serde_json::to_vec(&account).map_err(AppError::internal)?);
    tokio::task::spawn_blocking(move || write_atomic(&path, &bytes)).await??;
    Ok(claimed())
}
