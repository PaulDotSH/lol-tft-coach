mod common;

use axum::http::{Method, Request, StatusCode, header};
use common::{TOKEN, TestApp};
use axum::body::Body;
use tower::ServiceExt;

const ROUTES: [(&str, &str); 8] = [
    ("GET", "/v1/status"),
    ("POST", "/v1/players/a%23b/claim"),
    ("GET", "/v1/players/a%23b/files"),
    ("GET", "/v1/players/a%23b/files/k/n"),
    ("PUT", "/v1/players/a%23b/files/k/n"),
    ("DELETE", "/v1/players/a%23b/files/k/n"),
    ("GET", "/v1/lookup/euw1/a%23b"),
    ("GET", "/v1/unknown"),
];

#[tokio::test]
async fn health_needs_no_token() {
    let app = TestApp::new();
    let res = app.send(Method::GET, "/v1/health", None, Vec::new()).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.json(), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn every_other_route_but_the_table_wants_a_token() {
    let app = TestApp::new();
    for (method, uri) in &ROUTES[..7] {
        let method = Method::from_bytes(method.as_bytes()).unwrap();
        let res = app.send(method.clone(), uri, None, b"x".to_vec()).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(res.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert!(res.json()["error"].is_string());
        let res = app.send(method.clone(), uri, Some("wrong-token-0123456789abcdef0123456789"), b"x".to_vec()).await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn the_table_needs_no_token() {
    let app = TestApp::new();
    let none = app.send(Method::GET, "/v1/winprob.toml", None, Vec::new()).await;
    assert_eq!(none.status, StatusCode::NOT_FOUND);
    write_table(&app, "16.9", "patch = \"16.9\"
");
    let res = app.send(Method::GET, "/v1/winprob.toml", None, Vec::new()).await;
    assert_eq!((res.status, res.text()), (StatusCode::OK, "patch = \"16.9\"
".to_owned()));
    let wrong = app.send(Method::GET, "/v1/winprob.toml", Some("wrong-token-0123456789abcdef0123456789"), Vec::new()).await;
    assert_eq!(wrong.status, StatusCode::OK);
}

#[tokio::test]
async fn token_forms() {
    let app = TestApp::new();
    let with_header = |value: Option<&str>| {
        let mut req = Request::builder().uri("/v1/status");
        if let Some(value) = value {
            req = req.header(header::AUTHORIZATION, value);
        }
        let router = app.router.clone();
        let req = req.body(Body::empty()).unwrap();
        async move { router.oneshot(req).await.unwrap().status() }
    };
    assert_eq!(with_header(Some(&format!("Bearer {TOKEN}"))).await, StatusCode::OK);
    assert_eq!(with_header(Some(&format!("bearer {TOKEN}"))).await, StatusCode::OK);
    assert_eq!(with_header(Some("Bearer second-token-0123456789abcdef012345")).await, StatusCode::OK);
    assert_eq!(with_header(None).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some(TOKEN)).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some(&format!("Basic {TOKEN}"))).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some("Bearer ")).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some("Bearer")).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some(&format!("Bearer {}", &TOKEN[..TOKEN.len() - 1]))).await, StatusCode::UNAUTHORIZED);
    assert_eq!(with_header(Some(&format!("Bearer {TOKEN}x"))).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn tokens_do_not_appear_in_error_bodies() {
    let app = TestApp::new();
    let sent = "wrong-token-0123456789abcdef0123456789";
    let res = app.send(Method::GET, "/v1/status", Some(sent), Vec::new()).await;
    let text = res.text();
    assert!(!text.contains(sent) && !text.contains(TOKEN) && !text.contains("second-token"));
}

#[tokio::test]
async fn unknown_route_is_a_json_404() {
    let app = TestApp::new();
    let res = app.get("/v1/unknown").await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.json()["error"], "not found");
}

#[tokio::test]
async fn status_counts_players_and_reports_the_key() {
    let app = TestApp::new();
    let empty = app.get("/v1/status").await;
    assert_eq!(empty.status, StatusCode::OK);
    let body = empty.json();
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!((body["players"].as_u64(), body["riot_key"].as_bool()), (Some(0), Some(false)));
    assert!(body["crawl"].is_null());
    app.put("/v1/players/a%23x/files/k/n", "1").await;
    app.put("/v1/players/a%23x/files/k/m", "1").await;
    app.put("/v1/players/b%23x/files/k/n", "1").await;
    assert_eq!(app.get("/v1/status").await.json()["players"], 2);

    let fake = common::fake_riot().await;
    let with_key = TestApp::with(Some(fake.riot()), |_| {});
    assert_eq!(with_key.get("/v1/status").await.json()["riot_key"], true);
}

fn write_table(app: &TestApp, patch: &str, body: &str) {
    let dir = app.path().join("winprob").join(patch).join("fit");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("winprob.toml"), body).unwrap();
}

#[tokio::test]
async fn winprob_table_none_then_the_newest_patch_by_number() {
    let app = TestApp::new();
    assert_eq!(app.get("/v1/winprob.toml").await.status, StatusCode::NOT_FOUND);
    std::fs::create_dir_all(app.path().join("winprob").join("16.12").join("raw")).unwrap();
    assert_eq!(app.get("/v1/winprob.toml").await.status, StatusCode::NOT_FOUND);

    write_table(&app, "16.9", "patch = \"16.9\"\n");
    let one = app.get("/v1/winprob.toml").await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.text(), "patch = \"16.9\"\n");
    assert!(one.headers[header::CONTENT_TYPE].to_str().unwrap().contains("toml"));

    write_table(&app, "16.10", "patch = \"16.10\"\n");
    write_table(&app, "15.24", "patch = \"15.24\"\n");
    write_table(&app, "latest", "patch = \"latest\"\n");
    assert_eq!(app.get("/v1/winprob.toml").await.text(), "patch = \"16.10\"\n");

    write_table(&app, "17.1", "patch = \"17.1\"\n");
    assert_eq!(app.get("/v1/winprob.toml").await.text(), "patch = \"17.1\"\n");
}

#[tokio::test]
async fn winprob_table_skips_a_newer_patch_without_a_fit() {
    let app = TestApp::new();
    write_table(&app, "16.9", "patch = \"16.9\"\n");
    std::fs::create_dir_all(app.path().join("winprob").join("16.11").join("fit")).unwrap();
    assert_eq!(app.get("/v1/winprob.toml").await.text(), "patch = \"16.9\"\n");
}

#[tokio::test]
async fn build_tables_need_no_token_and_come_from_the_newest_patch_that_has_one() {
    let app = TestApp::new();
    assert_eq!(app.send(Method::GET, "/v1/builds.json", None, Vec::new()).await.status, StatusCode::NOT_FOUND);
    let write = |patch: &str, body: &str| {
        let dir = app.path().join("winprob").join(patch);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("builds.json"), body).unwrap();
    };
    write("16.21", r#"{"patch":"16.21"}"#);
    write("16.9", r#"{"patch":"16.9"}"#);
    std::fs::create_dir_all(app.path().join("winprob").join("16.22").join("raw")).unwrap();
    let res = app.send(Method::GET, "/v1/builds.json", None, Vec::new()).await;
    assert_eq!((res.status, res.text()), (StatusCode::OK, r#"{"patch":"16.21"}"#.to_owned()), "the previous patch's table stays up until the new one has its own");
    assert!(res.headers[header::CONTENT_TYPE].to_str().unwrap().contains("json"));
    write("16.22", r#"{"patch":"16.22"}"#);
    assert_eq!(app.send(Method::GET, "/v1/builds.json", None, Vec::new()).await.text(), r#"{"patch":"16.22"}"#);
}
