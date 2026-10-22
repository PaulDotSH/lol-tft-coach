//! League Client API via the lockfile. Read-only.

use model::{ChampSelect, EogStats};

#[derive(Clone)]
pub struct Lcu {
    pub client: reqwest::Client,
    /// e.g. "https://127.0.0.1:2753"
    pub base: String,
    /// "Basic …" header value.
    pub auth: String,
}

/// Find a running client (lockfile in the default install dir, else via the
/// LeagueClientUx process command line). None when the client is closed.
pub fn discover() -> Option<Lcu> {
    todo!()
}

impl Lcu {
    /// `/lol-gameflow/v1/gameflow-phase`: "None", "Lobby", "ChampSelect", "InProgress", "EndOfGame", …
    pub async fn phase(&self) -> anyhow::Result<String> {
        todo!()
    }

    /// Queue id of the current gameflow session, if any.
    pub async fn queue_id(&self) -> anyhow::Result<Option<i64>> {
        todo!()
    }

    pub async fn champ_select(&self) -> anyhow::Result<Option<ChampSelect>> {
        todo!()
    }

    pub async fn eog_stats(&self) -> anyhow::Result<Option<EogStats>> {
        todo!()
    }
}

/// Pure parser for `/lol-champ-select/v1/session` (tests use fixtures).
pub fn parse_champ_select(json: &serde_json::Value, queue_id: i64) -> Option<ChampSelect> {
    let _ = (json, queue_id);
    todo!()
}

/// Pure parser for `/lol-end-of-game/v1/eog-stats-block`.
pub fn parse_eog(json: &serde_json::Value) -> Option<EogStats> {
    let _ = json;
    todo!()
}
