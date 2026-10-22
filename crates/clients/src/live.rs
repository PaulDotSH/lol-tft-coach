//! Live Client Data API (`https://127.0.0.1:2999/liveclientdata/allgamedata`).

use model::{GameSnapshot, StaticData};

pub enum LiveState {
    /// No game process / connection refused.
    Unavailable,
    /// Game running but data not ready yet.
    Loading,
    InGame(GameSnapshot),
}

#[derive(Clone)]
pub struct Live {
    pub client: reqwest::Client,
}

impl Live {
    pub fn new() -> Live {
        todo!()
    }

    /// Also returns the raw JSON, for the fixture recorder.
    pub async fn poll(&self, sd: &StaticData) -> (LiveState, Option<serde_json::Value>) {
        let _ = sd;
        todo!()
    }
}

/// Pure parser (tests use fixtures). Champion names resolve via `sd`.
pub fn parse(json: &serde_json::Value, sd: &StaticData) -> Option<GameSnapshot> {
    let _ = (json, sd);
    todo!()
}
