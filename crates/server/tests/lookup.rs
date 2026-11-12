mod common;

use axum::http::StatusCode;
use common::{FIXTURE_PUUID, RIOT_KEY, TestApp, fake_riot};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const URI: &str = "/v1/lookup/euw1/Some%20Player%23EUW";
const OTHER_PUUID: &str = "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ";

async fn setup() -> (common::FakeRiot, TestApp) {
    let fake = fake_riot().await;
    let app = TestApp::with(Some(fake.riot()), |_| {});
    (fake, app)
}

fn lookup_file(app: &TestApp) -> PathBuf {
    app.path().join("players").join("some%20player%23euw").join("lookup").join("euw1.json")
}

fn edit_file(app: &TestApp, f: impl FnOnce(&mut Value)) {
    let path = lookup_file(app);
    let mut json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    f(&mut json);
    std::fs::write(path, serde_json::to_vec(&json).unwrap()).unwrap();
}

#[tokio::test]
async fn a_new_player_takes_four_requests_and_has_the_documented_shape() {
    let (fake, app) = setup().await;
    let res = app.get(URI).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    let body = res.json();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert!(body["fetched_at"].as_u64().unwrap().abs_diff(now) < 60);
    let mut shape = body.clone();
    shape["fetched_at"] = json!(0);
    assert_eq!(
        shape,
        json!({
            "riot_id": "Some Player#EUW", "platform": "euw1", "fetched_at": 0, "level": 971,
            "solo": {"tier": "CHALLENGER", "division": "I", "lp": 3112, "wins": 506, "losses": 408, "hot_streak": false},
            "flex": {"tier": "EMERALD", "division": "I", "lp": 97, "wins": 4, "losses": 1, "hot_streak": false},
            "mastery": [
                {"champion_id": 119, "level": 68, "points": 720889, "last_played": 1794421969},
                {"champion_id": 145, "level": 57, "points": 596715, "last_played": 1794405861},
                {"champion_id": 202, "level": 50, "points": 518155, "last_played": 1794146065},
                {"champion_id": 81, "level": 48, "points": 496459, "last_played": 1794060385},
                {"champion_id": 429, "level": 46, "points": 472284, "last_played": 1794408502},
            ],
        })
    );
    assert!(!res.text().contains(FIXTURE_PUUID) && !res.text().contains("puuid"));
    assert_eq!(fake.total(), 4);
    assert_eq!(fake.hits_with("/riot/account/v1/accounts/by-riot-id/some%20player/euw"), 1);
    assert_eq!(fake.hits_with("/lol/league/v4/entries/by-puuid/"), 1);
    assert_eq!(fake.hits_with("/lol/champion-mastery/v4/champion-masteries/by-puuid/"), 1);
    assert!(fake.state.hits.lock().unwrap().iter().any(|h| h.ends_with("/top?count=5")));
    assert_eq!(fake.hits_with("/lol/summoner/v4/summoners/by-puuid/"), 1);
    assert!(lookup_file(&app).is_file());
}

#[tokio::test]
async fn the_player_may_be_given_in_either_encoding_and_any_case() {
    let (fake, app) = setup().await;
    assert_eq!(app.get(URI).await.status, StatusCode::OK);
    for uri in ["/v1/lookup/euw1/some%2520player%2523euw", "/v1/lookup/EUW1/SOME%20PLAYER%23euw"] {
        assert_eq!(app.get(uri).await.status, StatusCode::OK, "{uri}");
    }
    assert_eq!(fake.total(), 4);
}

#[tokio::test]
async fn a_second_lookup_comes_from_the_cache() {
    let (fake, app) = setup().await;
    let first = app.get(URI).await.json();
    let second = app.get(URI).await.json();
    assert_eq!(first, second);
    assert_eq!(fake.total(), 4);
}

#[tokio::test]
async fn fresh_skips_the_cache_and_reuses_the_puuid() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    let res = app.get(&format!("{URI}?fresh=1")).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(fake.total(), 7);
    assert_eq!(fake.hits_with("/riot/account"), 1);
    assert!(res.json().get("stale").is_none());
}

#[tokio::test]
async fn an_expired_file_is_read_again() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    edit_file(&app, |f| f["fetched_at"] = json!(1_000));
    let res = app.get(URI).await;
    assert_eq!(fake.total(), 7);
    assert!(res.json()["fetched_at"].as_u64().unwrap() > 1_000_000);
    app.get(URI).await;
    assert_eq!(fake.total(), 7);
}

#[tokio::test]
async fn the_ttl_comes_from_the_config() {
    let fake = fake_riot().await;
    let app = TestApp::with(Some(fake.riot()), |c| c.lookup_ttl_hours = 0);
    app.get(URI).await;
    app.get(URI).await;
    assert_eq!(fake.total(), 7);
}

#[tokio::test]
async fn riot_failing_serves_the_older_file_as_stale() {
    let (fake, app) = setup().await;
    let first = app.get(URI).await.json();
    fake.set(|m| m.fail = Some(500));
    let res = app.get(&format!("{URI}?fresh=1")).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    let mut body = res.json();
    assert_eq!(body["stale"], true);
    body.as_object_mut().unwrap().remove("stale");
    assert_eq!(body, first);

    edit_file(&app, |f| f["fetched_at"] = json!(1_000));
    let res = app.get(URI).await;
    assert_eq!((res.status, res.json()["stale"].clone()), (StatusCode::OK, json!(true)));
    fake.set(|m| m.fail = None);
    let res = app.get(URI).await;
    assert!(res.json().get("stale").is_none());
}

#[tokio::test]
async fn a_stale_answer_is_not_stored_as_fresh() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    edit_file(&app, |f| f["fetched_at"] = json!(1_000));
    fake.set(|m| m.fail = Some(500));
    assert_eq!(app.get(URI).await.json()["stale"], true);
    let stored: Value = serde_json::from_slice(&std::fs::read(lookup_file(&app)).unwrap()).unwrap();
    assert_eq!(stored["fetched_at"], 1_000);
    assert!(stored.get("stale").is_none());
}

#[tokio::test]
async fn riot_failing_with_nothing_stored_is_a_bad_gateway() {
    let (fake, app) = setup().await;
    fake.set(|m| m.fail = Some(500));
    let res = app.get(URI).await;
    assert_eq!(res.status, StatusCode::BAD_GATEWAY);
    assert!(res.json()["error"].is_string());
    assert!(!lookup_file(&app).exists());
}

#[tokio::test]
async fn a_rejected_key_answers_503_and_the_key_is_not_in_the_body() {
    let (fake, app) = setup().await;
    fake.set(|m| m.fail = Some(403));
    let res = app.get(URI).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!res.text().contains(RIOT_KEY));
}

#[tokio::test]
async fn an_unknown_player_is_404_and_nothing_is_stored() {
    let (fake, app) = setup().await;
    let res = app.get("/v1/lookup/euw1/zzzz-no-such-player-zzzz%230000").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(fake.total(), 1);
    assert!(common::all_files(app.path()).is_empty());
    app.get("/v1/lookup/euw1/zzzz-no-such-player-zzzz%230000").await;
    assert_eq!(fake.total(), 2);
}

#[tokio::test]
async fn an_unknown_player_is_404_even_when_an_older_file_exists() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    edit_file(&app, |f| f["fetched_at"] = json!(1_000));
    let copy = app.path().join("players").join("zzzz-no-such-player-zzzz%230000").join("lookup");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::copy(lookup_file(&app), copy.join("euw1.json")).unwrap();
    fake.set(|m| m.puuid = OTHER_PUUID.to_owned());
    let res = app.get("/v1/lookup/euw1/zzzz-no-such-player-zzzz%230000").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn no_riot_key_answers_503() {
    let app = TestApp::new();
    let res = app.get(URI).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(res.json()["error"].as_str().unwrap().contains("key"));
}

#[tokio::test]
async fn bad_platform_and_bad_player_are_400() {
    let (fake, app) = setup().await;
    assert_eq!(app.get("/v1/lookup/mars1/Some%20Player%23EUW").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get("/v1/lookup/euw1/nohash").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get("/v1/lookup/euw1/..").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(fake.total(), 0);
}

#[tokio::test]
async fn another_keys_puuid_is_read_again() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    fake.set(|m| m.puuid = OTHER_PUUID.to_owned());
    let res = app.get(&format!("{URI}?fresh=1")).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(fake.hits_with("/riot/account"), 2);
    assert!(*fake.state.rejected.lock().unwrap() >= 1);
    let stored: Value = serde_json::from_slice(&std::fs::read(lookup_file(&app)).unwrap()).unwrap();
    assert_eq!(stored["puuid"], OTHER_PUUID);
    app.get(&format!("{URI}?fresh=1")).await;
    assert_eq!(fake.hits_with("/riot/account"), 2);
}

#[tokio::test]
async fn a_file_made_with_another_key_is_not_tried() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    edit_file(&app, |f| f["key_id"] = json!("0000000000000000"));
    let res = app.get(&format!("{URI}?fresh=1")).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(fake.hits_with("/riot/account"), 2);
    assert_eq!(*fake.state.rejected.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_broken_lookup_file_is_replaced() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    std::fs::write(lookup_file(&app), "not json").unwrap();
    assert_eq!(app.get(URI).await.status, StatusCode::OK);
    assert_eq!(fake.hits_with("/riot/account"), 2);
    assert!(serde_json::from_slice::<Value>(&std::fs::read(lookup_file(&app)).unwrap()).is_ok());
}

#[tokio::test]
async fn the_three_puuid_requests_run_together() {
    let (fake, app) = setup().await;
    fake.set(|m| m.delay = Duration::from_millis(400));
    let started = Instant::now();
    assert_eq!(app.get(URI).await.status, StatusCode::OK);
    let took = started.elapsed();
    assert!(took >= Duration::from_millis(400) && took < Duration::from_millis(1000), "{took:?}");
}

#[tokio::test]
async fn each_platform_has_its_own_file() {
    let (fake, app) = setup().await;
    app.get(URI).await;
    assert_eq!(app.get("/v1/lookup/eun1/Some%20Player%23EUW").await.json()["platform"], "eun1");
    assert_eq!(fake.total(), 8);
    let dir = lookup_file(&app).parent().unwrap().to_owned();
    assert!(dir.join("euw1.json").is_file() && dir.join("eun1.json").is_file());
}

#[tokio::test]
async fn the_lookup_file_is_not_in_the_players_file_list() {
    let (_fake, app) = setup().await;
    app.get(URI).await;
    assert_eq!(app.get("/v1/players/Some%20Player%23EUW/files").await.json(), json!([]));
    assert_eq!(app.get("/v1/status").await.json()["players"], 1);
}
