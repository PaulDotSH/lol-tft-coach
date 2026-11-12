use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::players::{LOOKUP_KIND, RiotId, player_dir, write_atomic};
use crate::riot::{Priority, Riot, RiotError, path_segment, region_of};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rank {
    pub tier: String,
    pub division: String,
    pub lp: u32,
    pub wins: u32,
    pub losses: u32,
    pub hot_streak: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Mastery {
    pub champion_id: u32,
    pub level: u32,
    pub points: u64,
    pub last_played: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Lookup {
    pub riot_id: String,
    pub platform: String,
    pub fetched_at: u64,
    pub level: u64,
    pub solo: Option<Rank>,
    pub flex: Option<Rank>,
    pub mastery: Vec<Mastery>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    #[serde(flatten)]
    lookup: Lookup,
    puuid: String,
    key_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    puuid: String,
    game_name: String,
    tag_line: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    queue_type: String,
    tier: String,
    rank: String,
    league_points: u32,
    wins: u32,
    losses: u32,
    #[serde(default)]
    hot_streak: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Champion {
    champion_id: u32,
    champion_level: u32,
    champion_points: u64,
    #[serde(default)]
    last_play_time: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summoner {
    summoner_level: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn rank_of(entries: &[Entry], queue: &str) -> Option<Rank> {
    let e = entries.iter().find(|e| e.queue_type == queue)?;
    Some(Rank { tier: e.tier.clone(), division: e.rank.clone(), lp: e.league_points, wins: e.wins, losses: e.losses, hot_streak: e.hot_streak })
}

fn is_decrypt_failure(err: &RiotError) -> bool {
    matches!(err, RiotError::Status(400, body) if body.contains("Exception decrypting"))
}

async fn by_puuid(riot: &Riot, platform: &str, puuid: &str) -> Result<(Vec<Entry>, Vec<Champion>, Summoner), RiotError> {
    let puuid = path_segment(puuid);
    let league = format!("/lol/league/v4/entries/by-puuid/{puuid}");
    let mastery = format!("/lol/champion-mastery/v4/champion-masteries/by-puuid/{puuid}/top?count=5");
    let summoner = format!("/lol/summoner/v4/summoners/by-puuid/{puuid}");
    tokio::try_join!(
        riot.get::<Vec<Entry>>(platform, &league, Priority::Lookup),
        riot.get::<Vec<Champion>>(platform, &mastery, Priority::Lookup),
        riot.get::<Summoner>(platform, &summoner, Priority::Lookup),
    )
}

fn assemble(riot_id: String, platform: &str, parts: (Vec<Entry>, Vec<Champion>, Summoner)) -> Lookup {
    let (entries, champions, summoner) = parts;
    Lookup {
        riot_id,
        platform: platform.to_owned(),
        fetched_at: now(),
        level: summoner.summoner_level,
        solo: rank_of(&entries, "RANKED_SOLO_5x5"),
        flex: rank_of(&entries, "RANKED_FLEX_SR"),
        mastery: champions.into_iter().map(|c| Mastery { champion_id: c.champion_id, level: c.champion_level, points: c.champion_points, last_played: c.last_play_time / 1000 }).collect(),
        stale: false,
    }
}

async fn fetch(riot: &Riot, platform: &str, region: &str, id: &RiotId, stored: Option<&Stored>) -> Result<Stored, RiotError> {
    let key_id = riot.key_id().to_owned();
    if let Some(old) = stored.filter(|s| s.key_id == key_id) {
        match by_puuid(riot, platform, &old.puuid).await {
            Ok(parts) => return Ok(Stored { lookup: assemble(old.lookup.riot_id.clone(), platform, parts), puuid: old.puuid.clone(), key_id }),
            Err(err) if is_decrypt_failure(&err) => tracing::info!("stored puuid does not decrypt with this key, reading it again"),
            Err(err) => return Err(err),
        }
    }
    let path = format!("/riot/account/v1/accounts/by-riot-id/{}/{}", path_segment(&id.name), path_segment(&id.tag));
    let account: Account = riot.get(region, &path, Priority::Lookup).await?;
    let parts = by_puuid(riot, platform, &account.puuid).await?;
    Ok(Stored { lookup: assemble(format!("{}#{}", account.game_name, account.tag_line), platform, parts), puuid: account.puuid, key_id })
}

async fn read_stored(path: &PathBuf) -> Option<Stored> {
    let bytes = tokio::fs::read(path).await.ok()?;
    serde_json::from_slice(&bytes).inspect_err(|err| tracing::warn!(path = %path.display(), %err, "unreadable lookup file")).ok()
}

pub async fn lookup(state: &AppState, platform: &str, id: &RiotId, fresh: bool) -> AppResult<Lookup> {
    let platform = platform.to_ascii_lowercase();
    let region = region_of(&platform).ok_or_else(|| AppError::bad_request("unknown platform"))?;
    let riot = state.riot.as_ref().ok_or_else(|| AppError::Unavailable("lookups are off: no Riot key".to_owned()))?;
    let path = player_dir(&state.config.data_dir, &id.key).join(LOOKUP_KIND).join(format!("{platform}.json"));
    let stored = read_stored(&path).await;
    let ttl = state.config.lookup_ttl_hours.saturating_mul(3600);
    if let Some(s) = stored.as_ref().filter(|s| !fresh && now().saturating_sub(s.lookup.fetched_at) < ttl) {
        return Ok(s.lookup.clone());
    }
    match fetch(riot, &platform, region, id, stored.as_ref()).await {
        Ok(new) => {
            let bytes = serde_json::to_vec(&new).map_err(AppError::internal)?;
            let target = path.clone();
            if let Err(err) = tokio::task::spawn_blocking(move || write_atomic(&target, &bytes)).await? {
                tracing::warn!(path = %path.display(), %err, "cannot store lookup");
            }
            Ok(new.lookup)
        }
        Err(RiotError::NotFound) => Err(AppError::NotFound),
        Err(err) => match stored {
            Some(old) => {
                tracing::warn!(%err, "lookup failed, serving the older one");
                Ok(Lookup { stale: true, ..old.lookup })
            }
            None => Err(match err {
                RiotError::KeyRejected => AppError::Unavailable("the Riot key was rejected".to_owned()),
                other => AppError::BadGateway(other.to_string()),
            }),
        },
    }
}
