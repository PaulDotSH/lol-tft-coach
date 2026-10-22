//! Riot's local APIs: the League Client (LCU) and the Live Client Data API.
//! Both use self-signed certs on 127.0.0.1; invalid certs are accepted only there.

pub mod lcu;
pub mod live;

use std::time::Duration;

/// HTTP client for Riot's loopback APIs. Invalid certs are accepted, which is
/// safe only because every request goes to a hard-coded `https://127.0.0.1`
/// base: no proxy, no redirects.
///
/// Windows retries a refused loopback connect for ~2 s; the short connect
/// timeout turns that into a quick `is_connect()` error instead.
pub(crate) fn local_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(500))
        .timeout(timeout)
        .build()
        .expect("reqwest client")
}

/// Positive integer id (champion / item); 0 and negatives mean "none".
pub(crate) fn id(v: &serde_json::Value) -> Option<u32> {
    v.as_u64().filter(|&n| n > 0).and_then(|n| u32::try_from(n).ok())
}
