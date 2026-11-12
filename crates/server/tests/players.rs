mod common;

use axum::http::{Method, StatusCode};
use common::{TOKEN, TestApp, all_files};
use lol_coach_server::players::RiotId;
use std::time::{SystemTime, UNIX_EPOCH};

const HELLO_SHA: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
const PLAYER: &str = "Some%20Player%23EUW";
const PLAYER_ENCODED_KEY: &str = "some%2520player%2523euw";

fn key(raw: &str) -> String {
    RiotId::parse(raw).unwrap().key
}

fn files_uri(player: &str, rest: &str) -> String {
    format!("/v1/players/{player}/files{rest}")
}

#[test]
fn key_is_trimmed_lowercased_and_percent_encoded() {
    assert_eq!(key("Some Player#EUW"), "some%20player%23euw");
    assert_eq!(key("  SOME PLAYER#euw  "), "some%20player%23euw");
    assert_eq!(key("a#b"), "a%23b");
    assert_eq!(key("a.b-c_d#1"), "a.b-c_d%231");
    assert_eq!(key("Zoë#Ñ1"), "zo%C3%AB%23%C3%B11");
    assert_eq!(key("a/b\\c#..d"), "a%2Fb%5Cc%23..d");
    assert_eq!(key("a % b#c"), "a%20%25%20b%23c");
}

#[test]
fn key_never_is_a_dot_name_or_holds_a_separator() {
    for raw in ["../..#x", "..#..", ".#.", "a/../b#c", "a\\..\\b#c"] {
        let k = key(raw);
        assert!(k.contains("%23") && !k.contains('/') && !k.contains('\\') && !k.contains('#') && k != "." && k != "..", "{k}");
    }
}

#[test]
fn riot_id_parts_are_kept() {
    let id = RiotId::parse(" Some Player # EUW ").unwrap();
    assert_eq!((id.name.as_str(), id.tag.as_str(), id.key.as_str()), ("some player", "euw", "some%20player%23euw"));
}

#[test]
fn bad_riot_ids_are_refused() {
    let long_name = format!("{}#t", "x".repeat(63));
    let wide = format!("{}#t", "é".repeat(62));
    let refused = ["", "   ", "nohash", "#tag", "name#", " # ", "a#b#c", "#", long_name.as_str(), wide.as_str()];
    for raw in refused {
        assert!(RiotId::parse(raw).is_err(), "accepted {raw:?}");
    }
    let longest = format!("{}#t", "x".repeat(62));
    assert_eq!(longest.chars().count(), 64);
    assert!(RiotId::parse(&longest).is_ok());
}

#[test]
fn path_form_accepts_the_raw_and_the_encoded_riot_id() {
    for segment in ["Some Player#EUW", "some%20player%23euw", "SOME%20PLAYER%23EUW"] {
        assert_eq!(RiotId::from_path(segment).unwrap().key, "some%20player%23euw", "{segment}");
    }
    assert!(RiotId::from_path("%zz%23a").is_err());
    assert!(RiotId::from_path("nohash").is_err());
    assert!(RiotId::from_path("%ff%23a").is_err());
}

#[tokio::test]
async fn store_round_trip() {
    let app = TestApp::new();
    let uri = files_uri(PLAYER, "/history/game-1.json");
    let put = app.put(&uri, "hello").await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.text());
    let body = put.json();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(body["size"], 5);
    assert_eq!(body["sha256"], HELLO_SHA);
    assert!(body["modified"].as_u64().unwrap().abs_diff(now) < 60);

    let got = app.get(&uri).await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(got.body, b"hello");
    assert_eq!(got.headers["etag"], format!("\"{HELLO_SHA}\""));

    let list = app.get(&files_uri(PLAYER, "")).await;
    assert_eq!(list.status, StatusCode::OK);
    let list = list.json();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["kind"], "history");
    assert_eq!(list[0]["name"], "game-1.json");
    assert_eq!(list[0]["size"], 5);
    assert_eq!(list[0]["sha256"], HELLO_SHA);
    assert_eq!(list[0]["modified"], body["modified"]);

    assert_eq!(app.delete(&uri).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&uri).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.delete(&uri).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&files_uri(PLAYER, "")).await.json(), serde_json::json!([]));
}

#[tokio::test]
async fn files_land_under_the_key_directory_and_no_temp_files_stay() {
    let app = TestApp::new();
    assert_eq!(app.put(&files_uri(PLAYER, "/recaps/a.json"), "x").await.status, StatusCode::OK);
    assert_eq!(app.put(&files_uri(PLAYER, "/recaps/a.json"), "yy").await.status, StatusCode::OK);
    let files = all_files(app.path());
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0], app.path().join("players").join("some%20player%23euw").join("recaps").join("a.json"));
    assert_eq!(std::fs::read(&files[0]).unwrap(), b"yy");
}

#[tokio::test]
async fn the_same_player_by_any_spelling_is_one_player() {
    let app = TestApp::new();
    app.put(&files_uri(PLAYER, "/k/n"), "1").await;
    for spelling in [PLAYER_ENCODED_KEY, "SOME%20PLAYER%23euw", "some%20player%23EUW"] {
        let got = app.get(&files_uri(spelling, "/k/n")).await;
        assert_eq!(got.status, StatusCode::OK, "{spelling}");
    }
    app.put(&files_uri(PLAYER_ENCODED_KEY, "/k/n"), "2").await;
    assert_eq!(app.get(&files_uri(PLAYER, "/k/n")).await.body, b"2");
    assert_eq!(all_files(app.path()).len(), 1);
}

#[tokio::test]
async fn last_write_wins_and_players_are_apart() {
    let app = TestApp::new();
    app.put(&files_uri("a%23x", "/k/n"), "first").await;
    app.put(&files_uri("a%23x", "/k/n"), "second").await;
    app.put(&files_uri("b%23x", "/k/n"), "other").await;
    assert_eq!(app.get(&files_uri("a%23x", "/k/n")).await.body, b"second");
    assert_eq!(app.get(&files_uri("b%23x", "/k/n")).await.body, b"other");
}

#[tokio::test]
async fn list_filters_by_kind_and_unknown_player_is_empty() {
    let app = TestApp::new();
    for (kind, name) in [("history", "a.json"), ("history", "b.json"), ("recaps", "c.json")] {
        app.put(&files_uri(PLAYER, &format!("/{kind}/{name}")), name).await;
    }
    let all = app.get(&files_uri(PLAYER, "")).await.json();
    let names: Vec<_> = all.as_array().unwrap().iter().map(|f| format!("{}/{}", f["kind"].as_str().unwrap(), f["name"].as_str().unwrap())).collect();
    assert_eq!(names, ["history/a.json", "history/b.json", "recaps/c.json"]);
    let one = app.get(&files_uri(PLAYER, "?kind=recaps")).await.json();
    assert_eq!(one.as_array().unwrap().len(), 1);
    assert_eq!(app.get(&files_uri(PLAYER, "?kind=nothing")).await.json(), serde_json::json!([]));
    assert_eq!(app.get(&files_uri(PLAYER, "?kind=..")).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.get(&files_uri("nobody%23x", "")).await.json(), serde_json::json!([]));
}

#[tokio::test]
async fn too_large_a_file_is_refused() {
    let app = TestApp::new();
    let uri = files_uri(PLAYER, "/k/big");
    let over = app.put(&uri, vec![7u8; 1024 * 1024 + 1]).await;
    assert_eq!(over.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(over.json()["error"].as_str().unwrap().contains("1 MB"));
    assert!(all_files(app.path()).is_empty());
    let exact = app.put(&uri, vec![7u8; 1024 * 1024]).await;
    assert_eq!(exact.status, StatusCode::OK, "{}", exact.text());
    assert_eq!(app.get(&uri).await.body.len(), 1024 * 1024);
}

#[tokio::test]
async fn an_empty_file_is_stored() {
    let app = TestApp::new();
    let uri = files_uri(PLAYER, "/k/empty");
    assert_eq!(app.put(&uri, "").await.status, StatusCode::OK);
    assert_eq!(app.get(&uri).await.body, b"");
}

#[tokio::test]
async fn names_that_try_to_leave_the_directory_are_refused() {
    let app = TestApp::new();
    let long = "x".repeat(129);
    let bad = ["..", "%2e%2e", "%2E%2E", ".", "%2e", "a%2Fb", "a%2fb", "%2e%2e%2f%2e%2e", "a%5Cb", "..%5C..", ".hidden", "-lead", "a%00b", "a%20b", "%C3%A9", long.as_str()];
    let mut pairs: Vec<(&str, &str)> = bad.iter().flat_map(|&p| [(p, "ok"), ("ok", p)]).collect();
    pairs.extend([("lookup", "euw1.json"), ("LOOKUP", "euw1.json"), ("Lookup", "x")]);
    for (kind, name) in pairs {
        let uri = files_uri(PLAYER, &format!("/{kind}/{name}"));
        for method in [Method::PUT, Method::GET, Method::DELETE] {
            let res = app.send(method.clone(), &uri, Some(TOKEN), b"x".to_vec()).await;
            assert_eq!(res.status, StatusCode::BAD_REQUEST, "{method} {uri}: {}", res.text());
        }
    }
    assert!(all_files(app.path()).is_empty());
}

#[tokio::test]
async fn raw_dot_segments_in_the_path_reach_no_route() {
    let app = TestApp::new();
    for uri in ["/v1/players/x%23y/files/../../../escape/n", "/v1/players/x%23y/files/k/../n", "/v1/players/../files/k/n", "/v1/players/x%23y/files/k/n/extra"] {
        let res = app.put(uri, "x").await;
        assert!(matches!(res.status, StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST), "{uri}: {}", res.status);
    }
    assert!(all_files(app.path()).is_empty());
    assert!(!app.path().join("escape").exists());
}

#[tokio::test]
async fn bad_players_are_refused_everywhere() {
    let app = TestApp::new();
    let long = format!("{}%23t", "x".repeat(64));
    for player in ["..", "%2e%2e", "nohash", "%23tag", "name%23", "a%23b%23c", long.as_str(), "%zz"] {
        let uri = files_uri(player, "/k/n");
        for res in [app.put(&uri, "x").await, app.get(&uri).await, app.delete(&uri).await, app.get(&files_uri(player, "")).await] {
            assert_eq!(res.status, StatusCode::BAD_REQUEST, "{player}: {}", res.text());
        }
    }
    assert!(all_files(app.path()).is_empty());
}

#[tokio::test]
async fn the_lookup_directory_is_not_reachable_through_the_store() {
    let app = TestApp::new();
    let lookup = app.path().join("players").join("some%20player%23euw").join("lookup");
    std::fs::create_dir_all(&lookup).unwrap();
    std::fs::write(lookup.join("euw1.json"), "{}").unwrap();
    assert_eq!(app.get(&files_uri(PLAYER, "")).await.json(), serde_json::json!([]));
    assert_eq!(app.get(&files_uri(PLAYER, "/lookup/euw1.json")).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.put(&files_uri(PLAYER, "/lookup/euw1.json"), "x").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read(lookup.join("euw1.json")).unwrap(), b"{}");
}

#[tokio::test]
async fn files_with_foreign_names_on_disk_are_not_listed() {
    let app = TestApp::new();
    let dir = app.path().join("players").join("some%20player%23euw").join("k");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("good.json"), "g").unwrap();
    std::fs::write(dir.join(".tmp-1-1"), "t").unwrap();
    std::fs::write(dir.join("-odd"), "o").unwrap();
    let names: Vec<_> = app.get(&files_uri(PLAYER, "")).await.json().as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_owned()).collect();
    assert_eq!(names, ["good.json"]);
}
