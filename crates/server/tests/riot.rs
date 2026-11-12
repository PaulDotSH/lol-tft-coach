mod common;

use common::{RIOT_KEY, fake_riot};
use lol_coach_server::riot::{Priority, Riot, RiotError};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(serde::Deserialize, Debug)]
struct Summoner {
    #[serde(rename = "summonerLevel")]
    level: u64,
}

const PATH: &str = "/lol/summoner/v4/summoners/by-puuid/PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP";

#[test]
fn urls_come_from_the_base_template() {
    let riot = Riot::new(RIOT_KEY).unwrap();
    assert_eq!(riot.url("euw1", "/x?y=1"), "https://euw1.api.riotgames.com/x?y=1");
    assert_eq!(riot.url("europe", "/x"), "https://europe.api.riotgames.com/x");
    let local = Riot::with_base(RIOT_KEY, "http://127.0.0.1:9").unwrap();
    assert_eq!(local.url("euw1", "/x"), "http://127.0.0.1:9/x");
}

#[test]
fn a_limiter_for_each_host_shared_by_all_callers() {
    let riot = Riot::new(RIOT_KEY).unwrap();
    assert!(Arc::ptr_eq(&riot.limiter("euw1"), &riot.limiter("euw1")));
    assert!(Arc::ptr_eq(&riot.limiter("europe"), &riot.clone().limiter("europe")));
    assert!(!Arc::ptr_eq(&riot.limiter("euw1"), &riot.limiter("europe")));
    assert!(!Arc::ptr_eq(&riot.limiter("euw1"), &riot.limiter("eun1")));
}

#[test]
fn the_key_id_is_not_the_key() {
    let a = Riot::new("RGAPI-aaaa").unwrap();
    let b = Riot::new("RGAPI-bbbb").unwrap();
    assert_ne!(a.key_id(), b.key_id());
    assert!(!a.key_id().contains("aaaa") && a.key_id().len() == 16);
    assert!(Riot::new("bad\nkey").is_err());
}

#[tokio::test]
async fn a_reply_is_read_and_the_key_is_sent_in_the_header() {
    let fake = fake_riot().await;
    let reply: Summoner = fake.riot().get("euw1", PATH, Priority::Lookup).await.unwrap();
    assert_eq!(reply.level, 971);
    assert_eq!(fake.state.tokens.lock().unwrap().as_slice(), [RIOT_KEY]);
}

#[tokio::test]
async fn limits_in_a_reply_header_replace_the_defaults() {
    let fake = fake_riot().await;
    fake.set(|m| m.rate_limit = Some("100:120,20:1".to_owned()));
    let riot = fake.riot();
    assert_eq!(riot.limiter("euw1").windows(), vec![(20, 1), (100, 120)]);
    fake.set(|m| m.rate_limit = Some("7:5,3:1".to_owned()));
    let _: Summoner = riot.get("euw1", PATH, Priority::Lookup).await.unwrap();
    assert_eq!(riot.limiter("euw1").windows(), vec![(7, 5), (3, 1)]);
    assert_eq!(riot.limiter("europe").windows(), vec![(20, 1), (100, 120)]);
}

#[tokio::test]
async fn a_429_holds_the_host_for_retry_after_and_is_tried_again() {
    let fake = fake_riot().await;
    fake.set(|m| m.limited_once = true);
    let started = Instant::now();
    let reply: Summoner = fake.riot().get("euw1", PATH, Priority::Lookup).await.unwrap();
    assert_eq!(reply.level, 971);
    assert!(started.elapsed() >= Duration::from_millis(950), "{:?}", started.elapsed());
    assert_eq!(fake.total(), 2);
}

#[tokio::test]
async fn a_server_error_is_tried_again_with_a_growing_wait() {
    let fake = fake_riot().await;
    fake.set(|m| m.fail_first = 2);
    let mut riot = fake.riot();
    riot.retry_base = Duration::from_millis(60);
    let started = Instant::now();
    let reply: Summoner = riot.get("euw1", PATH, Priority::Lookup).await.unwrap();
    assert_eq!(reply.level, 971);
    assert_eq!(fake.total(), 3);
    assert!(started.elapsed() >= Duration::from_millis(170), "{:?}", started.elapsed());
}

#[tokio::test]
async fn server_errors_that_do_not_stop_give_up() {
    let fake = fake_riot().await;
    fake.set(|m| m.fail = Some(503));
    let err = fake.riot().get::<Summoner>("euw1", PATH, Priority::Lookup).await.unwrap_err();
    assert!(matches!(err, RiotError::Unavailable(_)), "{err:?}");
    assert_eq!(fake.total(), 4);
}

#[tokio::test]
async fn a_rejected_key_is_its_own_error_and_is_not_retried() {
    let fake = fake_riot().await;
    for code in [401, 403] {
        fake.set(|m| m.fail = Some(code));
        let err = fake.riot().get::<Summoner>("euw1", PATH, Priority::Crawl).await.unwrap_err();
        assert!(matches!(err, RiotError::KeyRejected), "{err:?}");
        assert!(!err.to_string().contains(RIOT_KEY));
    }
    assert_eq!(fake.total(), 2);
}

#[tokio::test]
async fn not_found_and_other_statuses_are_told_apart() {
    let fake = fake_riot().await;
    let err = fake.riot().get::<Summoner>("europe", "/riot/account/v1/accounts/by-riot-id/zzzz-no-such-player-zzzz/0000", Priority::Lookup).await.unwrap_err();
    assert!(matches!(err, RiotError::NotFound), "{err:?}");
    fake.set(|m| m.puuid = "other".to_owned());
    let err = fake.riot().get::<Summoner>("euw1", PATH, Priority::Lookup).await.unwrap_err();
    assert!(matches!(&err, RiotError::Status(400, body) if body.contains("Exception decrypting")), "{err:?}");
}

#[tokio::test]
async fn a_dead_server_is_unavailable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let mut riot = Riot::with_base(RIOT_KEY, &base).unwrap();
    riot.retry_base = Duration::from_millis(1);
    let err = riot.get::<Summoner>("euw1", PATH, Priority::Lookup).await.unwrap_err();
    assert!(matches!(&err, RiotError::Unavailable(msg) if !msg.contains(RIOT_KEY)), "{err:?}");
}

#[tokio::test]
async fn a_crawl_request_goes_through_the_same_path() {
    let fake = fake_riot().await;
    let riot = fake.riot();
    let reply: Summoner = riot.get("euw1", PATH, Priority::Crawl).await.unwrap();
    assert_eq!(reply.level, 971);
    assert_eq!(fake.total(), 1);
}
