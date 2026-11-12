use lol_coach_server::riot::{Limiter, Priority, parse_limits};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

#[test]
fn limits_header_is_parsed() {
    assert_eq!(parse_limits("100:120,20:1"), vec![(100, 120), (20, 1)]);
    assert_eq!(parse_limits("20:1"), vec![(20, 1)]);
    assert_eq!(parse_limits(" 5:10 , x:1, 0:3, 7:0, 9 "), vec![(5, 10)]);
    assert!(parse_limits("").is_empty());
}

#[tokio::test(start_paused = true)]
async fn short_window_holds_back_the_twenty_first() {
    let limiter = Limiter::new();
    let start = Instant::now();
    for _ in 0..20 {
        limiter.acquire(Priority::Lookup).await;
    }
    assert_eq!(start.elapsed(), Duration::ZERO);
    limiter.acquire(Priority::Lookup).await;
    assert_eq!(start.elapsed(), secs(1.0));
}

#[tokio::test(start_paused = true)]
async fn long_window_holds_back_the_hundred_and_first() {
    let limiter = Limiter::new();
    let start = Instant::now();
    for _ in 0..100 {
        limiter.acquire(Priority::Lookup).await;
    }
    assert!(start.elapsed() < secs(10.0));
    limiter.acquire(Priority::Lookup).await;
    assert_eq!(start.elapsed(), secs(120.0));
}

#[tokio::test(start_paused = true)]
async fn windows_replaced_by_the_header_values() {
    let limiter = Limiter::new();
    limiter.set_windows(parse_limits("3:10,1:1"));
    assert_eq!(limiter.windows(), vec![(3, 10), (1, 1)]);
    let start = Instant::now();
    for expected in [0.0, 1.0, 2.0] {
        limiter.acquire(Priority::Lookup).await;
        assert_eq!(start.elapsed(), secs(expected));
    }
    limiter.acquire(Priority::Lookup).await;
    assert_eq!(start.elapsed(), secs(10.0));
}

#[tokio::test(start_paused = true)]
async fn an_empty_header_keeps_the_windows() {
    let limiter = Limiter::new();
    limiter.set_windows(parse_limits("garbage"));
    assert_eq!(limiter.windows(), vec![(20, 1), (100, 120)]);
}

#[tokio::test(start_paused = true)]
async fn retry_after_holds_the_host() {
    let limiter = Limiter::new();
    let start = Instant::now();
    limiter.hold(Duration::from_secs(5));
    limiter.hold(Duration::from_secs(2));
    limiter.acquire(Priority::Lookup).await;
    assert_eq!(start.elapsed(), secs(5.0));
    limiter.acquire(Priority::Crawl).await;
    assert_eq!(start.elapsed(), secs(5.0));
}

#[tokio::test(start_paused = true)]
async fn a_lookup_takes_the_slot_ahead_of_a_waiting_crawl() {
    let limiter = Arc::new(Limiter::new());
    limiter.set_windows(vec![(1, 10)]);
    let start = Instant::now();
    limiter.acquire(Priority::Lookup).await;
    let crawl = {
        let limiter = limiter.clone();
        tokio::spawn(async move {
            limiter.acquire(Priority::Crawl).await;
            start.elapsed()
        })
    };
    tokio::task::yield_now().await;
    let lookup = {
        let limiter = limiter.clone();
        tokio::spawn(async move {
            limiter.acquire(Priority::Lookup).await;
            start.elapsed()
        })
    };
    assert_eq!(lookup.await.unwrap(), secs(10.0));
    assert_eq!(crawl.await.unwrap(), secs(20.0));
}

#[tokio::test(start_paused = true)]
async fn a_crawl_waits_while_a_lookup_waits() {
    let limiter = Arc::new(Limiter::new());
    limiter.set_windows(vec![(1, 10)]);
    let start = Instant::now();
    limiter.acquire(Priority::Lookup).await;
    let lookup = {
        let limiter = limiter.clone();
        tokio::spawn(async move {
            limiter.acquire(Priority::Lookup).await;
            start.elapsed()
        })
    };
    tokio::task::yield_now().await;
    let crawl = {
        let limiter = limiter.clone();
        tokio::spawn(async move {
            limiter.acquire(Priority::Crawl).await;
            start.elapsed()
        })
    };
    let (lookup, crawl) = (lookup.await.unwrap(), crawl.await.unwrap());
    assert_eq!(lookup, secs(10.0));
    assert!(crawl > lookup);
}

#[tokio::test(start_paused = true)]
async fn crawl_requests_are_spaced_evenly_over_the_longest_window() {
    let limiter = Limiter::new();
    let start = Instant::now();
    for i in 0..4 {
        limiter.acquire(Priority::Crawl).await;
        assert_eq!(start.elapsed(), secs(1.2 * f64::from(i)));
    }
}

#[tokio::test(start_paused = true)]
async fn crawl_spacing_follows_the_limits_from_the_header() {
    let limiter = Limiter::new();
    limiter.set_windows(parse_limits("50:100,10:1"));
    let start = Instant::now();
    for i in 0..3 {
        limiter.acquire(Priority::Crawl).await;
        assert_eq!(start.elapsed(), secs(2.0 * f64::from(i)));
    }
}

#[tokio::test(start_paused = true)]
async fn lookups_are_not_spaced() {
    let limiter = Limiter::new();
    let start = Instant::now();
    limiter.acquire(Priority::Crawl).await;
    for _ in 0..5 {
        limiter.acquire(Priority::Lookup).await;
    }
    assert_eq!(start.elapsed(), Duration::ZERO);
}
