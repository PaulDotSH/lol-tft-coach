mod common;

use axum::http::{Method, StatusCode};
use common::{FIXTURE_PUUID, FakeRiot, Reply, TOKEN, TestApp, fake_riot, fixture};
use lol_coach_server::players::sha256_hex;
use serde_json::{Value, json};

const PLAYER: &str = "some%20player%23euw";
const PLAYER_DIR: &str = "some%20player%23euw";
const OTHER: &str = "other%23euw";
const KEY_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const KEY_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const END_MS: u64 = 1793791696398;
const HOUR_MS: u64 = 3_600_000;
const MATCH_1: &str = "EUW1_7000000001";
const MATCH_2: &str = "EUW1_7000000002";

fn participant(n: u32) -> String {
    format!("{n:02}{}", &FIXTURE_PUUID[2..])
}

fn good(key: &str, game_id: u64) -> Value {
    json!({ "platform": "euw1", "game_id": game_id, "champion_id": 14, "win": false, "duration_s": 1358, "account_key": key })
}

async fn rig() -> (FakeRiot, TestApp) {
    let fake = fake_riot().await;
    fake.set(|m| m.puuid = participant(1));
    fake.route(&format!("/lol/match/v5/matches/{MATCH_1}"), 200, fixture("match.json"));
    fake.route(&format!("/lol/match/v5/matches/{MATCH_2}"), 200, fixture("match.json").replace("7000000001", "7000000002"));
    let mut app = TestApp::with(Some(fake.riot()), |_| {});
    app.at(END_MS + HOUR_MS);
    (fake, app)
}

async fn claim_for(app: &TestApp, player: &str, body: &Value) -> Reply {
    app.send_as(Method::POST, &format!("/v1/players/{player}/claim"), Some(TOKEN), None, body.to_string().into_bytes()).await
}

async fn claim(app: &TestApp, body: &Value) -> Reply {
    claim_for(app, PLAYER, body).await
}

fn account(app: &TestApp) -> Value {
    serde_json::from_slice(&std::fs::read(app.path().join("players").join(PLAYER_DIR).join("account.json")).unwrap()).unwrap()
}

fn has_account(app: &TestApp) -> bool {
    app.path().join("players").join(PLAYER_DIR).join("account.json").exists()
}

fn files_uri(player: &str, rest: &str) -> String {
    format!("/v1/players/{player}/files{rest}")
}

async fn with_key(app: &TestApp, method: Method, uri: &str, key: Option<&str>, body: &str) -> Reply {
    app.send_as(method, uri, Some(TOKEN), key, body.as_bytes().to_vec()).await
}

#[tokio::test]
async fn a_good_claim_registers_the_hash_and_the_match() {
    let (fake, app) = rig().await;
    let res = claim(&app, &good(KEY_A, 7000000001)).await;
    assert_eq!((res.status, res.json()), (StatusCode::OK, json!({ "claimed": true })));
    assert_eq!(account(&app), json!({ "keys": [sha256_hex(KEY_A.as_bytes())], "matches": [MATCH_1] }));
    assert_eq!(fake.hits_with("/riot/account/v1/accounts/by-riot-id/some%20player/euw"), 1);
    assert_eq!(fake.hits_exact("/lol/match/v5/matches/EUW1_7000000001"), 1);
    assert!(!std::fs::read_to_string(app.path().join("players").join(PLAYER_DIR).join("account.json")).unwrap().contains(KEY_A));
    assert_eq!(with_key(&app, Method::PUT, &files_uri(PLAYER, "/k/n"), Some(KEY_A), "x").await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_same_key_again_is_ok_with_no_riot_request_and_no_match_used() {
    let (fake, app) = rig().await;
    claim(&app, &good(KEY_A, 7000000001)).await;
    let before = fake.total();
    for game in [7000000001, 7000000002, 5] {
        let res = claim(&app, &good(KEY_A, game)).await;
        assert_eq!((res.status, res.json()), (StatusCode::OK, json!({ "claimed": true })));
    }
    assert_eq!(fake.total(), before);
    assert_eq!(account(&app)["matches"], json!([MATCH_1]));
}

#[tokio::test]
async fn a_second_key_for_the_same_player_needs_another_match() {
    let (_, app) = rig().await;
    claim(&app, &good(KEY_A, 7000000001)).await;
    assert_eq!(claim(&app, &good(KEY_B, 7000000001)).await.status, StatusCode::CONFLICT);
    assert_eq!(account(&app)["keys"].as_array().unwrap().len(), 1);
    assert_eq!(claim(&app, &good(KEY_B, 7000000002)).await.status, StatusCode::OK);
    assert_eq!(account(&app), json!({ "keys": [sha256_hex(KEY_A.as_bytes()), sha256_hex(KEY_B.as_bytes())], "matches": [MATCH_1, MATCH_2] }));
    for key in [KEY_A, KEY_B] {
        assert_eq!(with_key(&app, Method::GET, &files_uri(PLAYER, ""), Some(key), "").await.status, StatusCode::OK);
    }
}

#[tokio::test]
async fn every_mismatch_is_the_same_403() {
    let (fake, app) = rig().await;
    let mut wrong_champion = good(KEY_A, 7000000001);
    wrong_champion["champion_id"] = json!(15);
    let mut wrong_result = good(KEY_A, 7000000001);
    wrong_result["win"] = json!(true);
    let mut too_short = good(KEY_A, 7000000001);
    too_short["duration_s"] = json!(1347);
    let mut too_long = good(KEY_A, 7000000001);
    too_long["duration_s"] = json!(1369);
    let mut bodies = Vec::new();
    for body in [wrong_champion, wrong_result, too_short, too_long] {
        let res = claim(&app, &body).await;
        assert_eq!(res.status, StatusCode::FORBIDDEN, "{body}");
        bodies.push(res.text());
    }
    fake.set(|m| m.puuid = participant(1).replace("01", "99"));
    let stranger = claim(&app, &good(KEY_A, 7000000001)).await;
    assert_eq!(stranger.status, StatusCode::FORBIDDEN);
    bodies.push(stranger.text());
    fake.set(|m| m.puuid = participant(1));
    let mut late = TestApp::with(Some(fake.riot()), |_| {});
    late.at(END_MS + 24 * HOUR_MS + 1);
    let old = claim(&late, &good(KEY_A, 7000000001)).await;
    assert_eq!(old.status, StatusCode::FORBIDDEN);
    bodies.push(old.text());
    assert!(bodies.windows(2).all(|pair| pair[0] == pair[1]), "{bodies:?}");
    assert!(bodies[0].contains("error"));
    assert!(!has_account(&app) && !has_account(&late));
}

#[tokio::test]
async fn the_edges_of_duration_and_age_are_accepted() {
    for duration in [1348, 1368] {
        let (_, app) = rig().await;
        let mut body = good(KEY_A, 7000000001);
        body["duration_s"] = json!(duration);
        assert_eq!(claim(&app, &body).await.status, StatusCode::OK, "{duration}");
    }
    let (_, mut app) = rig().await;
    app.at(END_MS + 24 * HOUR_MS);
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_age_limit_comes_from_the_config() {
    let fake = fake_riot().await;
    fake.set(|m| m.puuid = participant(1));
    fake.route("/lol/match/v5/matches/EUW1_7000000001", 200, fixture("match.json"));
    let mut app = TestApp::with(Some(fake.riot()), |c| c.claim_max_age_hours = 2);
    app.at(END_MS + 3 * HOUR_MS);
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::FORBIDDEN);
    app.at(END_MS + 2 * HOUR_MS);
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_system_clock_is_used_by_default_and_the_fixture_match_is_old() {
    let fake = fake_riot().await;
    fake.set(|m| m.puuid = participant(1));
    fake.route("/lol/match/v5/matches/EUW1_7000000001", 200, fixture("match.json"));
    let app = TestApp::with(Some(fake.riot()), |_| {});
    let now = app.state.now_ms();
    assert!(now > 1_700_000_000_000);
    let res = claim(&app, &good(KEY_A, 7000000001)).await;
    assert_eq!(res.status, if now > END_MS + 24 * HOUR_MS { StatusCode::FORBIDDEN } else { StatusCode::OK });
}

#[tokio::test]
async fn riot_without_the_match_is_202_and_nothing_is_stored() {
    let (_, app) = rig().await;
    let res = claim(&app, &good(KEY_A, 7000000099)).await;
    assert_eq!((res.status, res.json()), (StatusCode::ACCEPTED, json!({ "claimed": false, "retry": true })));
    assert!(!has_account(&app));
}

#[tokio::test]
async fn a_bad_claim_is_400_and_asks_riot_nothing() {
    let (fake, app) = rig().await;
    let long = format!("{KEY_A}a");
    for key in ["", "abc", &KEY_A[..63], long.as_str(), &"g".repeat(64), &" ".repeat(64)] {
        let res = claim(&app, &good(key, 7000000001)).await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{key:?}");
        assert!(!res.text().contains(&format!("\"{key}\"")) || key.is_empty());
    }
    let mut no_key = good(KEY_A, 7000000001);
    no_key.as_object_mut().unwrap().remove("account_key");
    assert_eq!(claim(&app, &no_key).await.status, StatusCode::BAD_REQUEST);
    let mut bad_platform = good(KEY_A, 7000000001);
    bad_platform["platform"] = json!("xx9");
    assert_eq!(claim(&app, &bad_platform).await.status, StatusCode::BAD_REQUEST);
    let garbage = app.send_as(Method::POST, &format!("/v1/players/{PLAYER}/claim"), Some(TOKEN), None, b"not json".to_vec()).await;
    assert_eq!(garbage.status, StatusCode::BAD_REQUEST);
    assert_eq!(claim_for(&app, "nohash", &good(KEY_A, 7000000001)).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(fake.total(), 0);
}

#[tokio::test]
async fn an_unknown_player_is_404() {
    let (_, app) = rig().await;
    let res = claim_for(&app, "zzzz-no-such-player-zzzz%23euw", &good(KEY_A, 7000000001)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn without_a_riot_key_a_new_claim_is_503_but_a_known_key_is_still_ok() {
    let app = TestApp::new();
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!has_account(&app));
    app.register(PLAYER, KEY_A);
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn riot_trouble_is_503_or_502() {
    let (fake, app) = rig().await;
    fake.route("/riot/account/v1/accounts/by-riot-id/*", 403, "{}");
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::SERVICE_UNAVAILABLE);
    fake.route("/riot/account/v1/accounts/by-riot-id/*", 418, "teapot");
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::BAD_GATEWAY);
    fake.state.routes.lock().unwrap().remove("/riot/account/v1/accounts/by-riot-id/*");
    fake.route("/lol/match/v5/matches/EUW1_7000000001", 401, "{}");
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::SERVICE_UNAVAILABLE);
    fake.route("/lol/match/v5/matches/EUW1_7000000001", 200, "not json");
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::BAD_GATEWAY);
    assert!(!has_account(&app));
}

#[tokio::test]
async fn the_claim_wants_the_bearer_token() {
    let (fake, app) = rig().await;
    let res = app.send_as(Method::POST, &format!("/v1/players/{PLAYER}/claim"), None, None, good(KEY_A, 7000000001).to_string().into_bytes()).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(fake.total(), 0);
}

#[tokio::test]
async fn racing_claims_never_share_a_match_or_lose_a_key() {
    let (_, app) = rig().await;
    let (body_a, body_b) = (good(KEY_A, 7000000001), good(KEY_B, 7000000001));
    let (a, b) = tokio::join!(claim(&app, &body_a), claim(&app, &body_b));
    let mut statuses = [a.status, b.status];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    assert_eq!(account(&app)["keys"].as_array().unwrap().len(), 1);
    assert_eq!(account(&app)["matches"], json!([MATCH_1]));

    let (_, app) = rig().await;
    let (body_a, body_b) = (good(KEY_A, 7000000001), good(KEY_B, 7000000002));
    let (a, b, c) = tokio::join!(claim(&app, &body_a), claim(&app, &body_b), claim(&app, &body_a));
    assert_eq!((a.status, b.status, c.status), (StatusCode::OK, StatusCode::OK, StatusCode::OK));
    let stored = account(&app);
    assert_eq!(stored["keys"].as_array().unwrap().len(), 2);
    assert_eq!(stored["matches"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn the_files_routes_want_the_players_account_key() {
    let (_, app) = rig().await;
    claim(&app, &good(KEY_A, 7000000001)).await;
    app.register(OTHER, KEY_B);
    let uri = files_uri(PLAYER, "/k/n");
    assert_eq!(with_key(&app, Method::PUT, &uri, Some(KEY_A), "mine").await.status, StatusCode::OK);
    let mut refusals = Vec::new();
    for key in [None, Some("wrong"), Some(KEY_B), Some(&"c".repeat(64)[..]), Some(""), Some(&KEY_A[..63])] {
        for (method, target) in [(Method::GET, files_uri(PLAYER, "")), (Method::GET, uri.clone()), (Method::PUT, uri.clone()), (Method::DELETE, uri.clone())] {
            let res = with_key(&app, method.clone(), &target, key, "evil").await;
            assert_eq!(res.status, StatusCode::FORBIDDEN, "{method} {target} {key:?}");
            refusals.push(res.text());
        }
    }
    let unclaimed = with_key(&app, Method::GET, &files_uri("nobody%23euw", ""), Some(KEY_A), "").await;
    assert_eq!(unclaimed.status, StatusCode::FORBIDDEN);
    refusals.push(unclaimed.text());
    assert_eq!(with_key(&app, Method::PUT, &files_uri("nobody%23euw", "/k/n"), Some(KEY_A), "x").await.status, StatusCode::FORBIDDEN);
    assert!(refusals.windows(2).all(|pair| pair[0] == pair[1]), "{refusals:?}");
    assert!(refusals[0].contains("error"));
    assert!(!app.path().join("players").join("nobody%23euw").exists());
    assert_eq!(with_key(&app, Method::GET, &uri, Some(KEY_A), "").await.body, b"mine");
    assert_eq!(with_key(&app, Method::GET, &files_uri(PLAYER, ""), Some(KEY_A), "").await.json().as_array().unwrap().len(), 1);
    assert_eq!(with_key(&app, Method::GET, &files_uri(OTHER, ""), Some(KEY_B), "").await.status, StatusCode::OK);
    assert_eq!(with_key(&app, Method::DELETE, &uri, Some(KEY_A), "").await.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn the_key_is_checked_before_a_body_is_read_and_after_the_token() {
    let app = TestApp::new();
    app.register(PLAYER, KEY_A);
    let big = vec![7u8; 1024 * 1024 + 1];
    let uri = files_uri(PLAYER, "/k/big");
    let res = app.send_as(Method::PUT, &uri, Some(TOKEN), Some(KEY_B), big.clone()).await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    let res = app.send_as(Method::PUT, &uri, Some(TOKEN), Some(KEY_A), big.clone()).await;
    assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
    let res = app.send_as(Method::PUT, &uri, None, Some(KEY_A), b"x".to_vec()).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    let res = app.send_as(Method::GET, &files_uri(PLAYER, ""), None, None, Vec::new()).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn lookups_status_and_the_claim_need_no_account_key() {
    let (_, app) = rig().await;
    assert_eq!(app.send_as(Method::GET, "/v1/status", Some(TOKEN), None, Vec::new()).await.status, StatusCode::OK);
    assert_eq!(app.send_as(Method::GET, "/v1/lookup/euw1/some%20player%23euw", Some(TOKEN), None, Vec::new()).await.status, StatusCode::OK);
    assert_eq!(claim(&app, &good(KEY_A, 7000000001)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn account_json_cannot_be_reached_through_the_files_routes() {
    let (_, app) = rig().await;
    claim(&app, &good(KEY_A, 7000000001)).await;
    let path = app.path().join("players").join(PLAYER_DIR).join("account.json");
    let before = std::fs::read(&path).unwrap();
    for kind in ["account.json", "ACCOUNT.JSON", "Account.Json"] {
        for target in [files_uri(PLAYER, &format!("/{kind}/n")), files_uri(PLAYER, &format!("/{kind}")), files_uri(PLAYER, &format!("/{kind}/account.json"))] {
            for method in [Method::GET, Method::PUT, Method::DELETE] {
                let res = with_key(&app, method.clone(), &target, Some(KEY_A), "{\"keys\":[]}").await;
                assert!(matches!(res.status, StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND), "{method} {target}: {}", res.status);
                assert!(!res.text().contains(&sha256_hex(KEY_A.as_bytes())));
            }
        }
    }
    let kind_uri = files_uri(PLAYER, "?kind=account.json");
    assert_eq!(with_key(&app, Method::GET, &kind_uri, Some(KEY_A), "").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(with_key(&app, Method::PUT, &files_uri(PLAYER, "/k/n"), Some(KEY_A), "x").await.status, StatusCode::OK);
    let listing = with_key(&app, Method::GET, &files_uri(PLAYER, ""), Some(KEY_A), "").await;
    assert!(!listing.text().contains("account"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn the_key_and_its_hash_appear_in_no_reply() {
    let (fake, app) = rig().await;
    let hash = sha256_hex(KEY_A.as_bytes());
    let mut replies = vec![claim(&app, &good(KEY_A, 7000000099)).await];
    replies.push(claim(&app, &good(KEY_A, 7000000001)).await);
    replies.push(claim(&app, &good(KEY_A, 7000000001)).await);
    replies.push(claim(&app, &good(KEY_B, 7000000001)).await);
    let mut wrong = good(KEY_B, 7000000002);
    wrong["win"] = json!(true);
    replies.push(claim(&app, &wrong).await);
    replies.push(with_key(&app, Method::GET, &files_uri(PLAYER, ""), Some(KEY_B), "").await);
    replies.push(with_key(&app, Method::GET, &files_uri(PLAYER, ""), Some(KEY_A), "").await);
    replies.push(with_key(&app, Method::GET, &files_uri(PLAYER, ""), None, "").await);
    replies.push(claim(&app, &good(&format!("{KEY_A}zz"), 7000000001)).await);
    fake.set(|m| m.fail = Some(500));
    replies.push(claim(&app, &good(KEY_B, 7000000002)).await);
    for reply in replies {
        let text = reply.text();
        assert!(!text.contains(KEY_A) && !text.contains(KEY_B) && !text.contains(&hash), "{text}");
        assert!(reply.headers.values().all(|v| !v.to_str().unwrap_or_default().contains(KEY_A)));
    }
    for hit in fake.hit_list() {
        assert!(!hit.contains(KEY_A) && !hit.contains(&hash));
    }
}
