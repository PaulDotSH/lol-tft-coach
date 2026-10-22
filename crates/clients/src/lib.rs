//! Riot's local APIs: the League Client (LCU) and the Live Client Data API.
//! Both use self-signed certs on 127.0.0.1; invalid certs are accepted only there.

pub mod lcu;
pub mod live;
