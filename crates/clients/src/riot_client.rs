//! Riot Client API via its lockfile. Read-only. The standalone TFT client has no local
//! API of its own: what it is doing shows in my chat presence, which the Riot Client serves.

use crate::lcu::{Lcu, parse_lockfile, processes};
use anyhow::Result;
use base64::Engine;
use serde_json::Value;

const PRODUCT_TFT: &str = "teamfighttactics";

/// What the standalone TFT client says I am doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TftPresence {
    /// The queue of the game or lobby, from `game_display_name_queueId_<id>`.
    pub queue: Option<i64>,
    /// The activity's progress: "InGame", …; empty in the menus.
    pub progress: String,
    /// My Riot ID game name.
    pub name: Option<String>,
}

impl TftPresence {
    pub fn in_game(&self) -> bool {
        self.progress == "InGame"
    }
}

/// The Riot Client's API: the same basic-auth scheme as the League client's.
#[derive(Clone)]
pub struct RiotClient(Lcu);

fn lockfile() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Riot Games\Riot Client\Config\lockfile"))
}

/// The running Riot Client from its lockfile; None when it is closed.
pub fn discover() -> Option<RiotClient> {
    let (pid, port, password) = parse_lockfile(&std::fs::read_to_string(lockfile()?).ok()?)?;
    let running = processes().is_none_or(|p| p.iter().any(|(id, _)| *id == pid));
    running.then(|| RiotClient(Lcu::new(port, &password)))
}

impl RiotClient {
    /// My TFT presence; None while the TFT client is closed.
    pub async fn tft(&self) -> Result<Option<TftPresence>> {
        let session = self.0.get("/chat/v1/session").await?.unwrap_or_default();
        let Some(puuid) = session["puuid"].as_str().filter(|p| !p.is_empty()) else { return Ok(None) };
        let presences = self.0.get("/chat/v4/presences").await?.unwrap_or_default();
        Ok(parse_tft_presence(&presences, puuid))
    }
}

/// My `teamfighttactics` entry of `/chat/v4/presences`, its `packedData` decoded.
pub(crate) fn parse_tft_presence(presences: &Value, puuid: &str) -> Option<TftPresence> {
    let me = presences["presences"].as_array()?.iter().find(|p| p["puuid"] == puuid && p["product"] == PRODUCT_TFT)?;
    let packed = me["packedData"].as_str().unwrap_or_default().trim_end_matches('=');
    let packed: Value = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(packed)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let activity = &packed["activity"];
    let queue = activity["mode"].as_str().and_then(|m| m.rsplit_once("queueId_")).and_then(|(_, q)| q.parse().ok()).filter(|&q| q > 0);
    Some(TftPresence {
        queue,
        progress: activity.pointer("/progress/value").and_then(Value::as_str).unwrap_or_default().to_owned(),
        name: me["game_name"].as_str().filter(|n| !n.is_empty()).map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ME: &str = "00000000-1111-2222-3333-444444444444";

    fn fixture() -> Value {
        let s = std::fs::read_to_string(format!("{}/../../fixtures/clients/rc_presences.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn tft_in_game() {
        let p = parse_tft_presence(&fixture(), ME).unwrap();
        assert_eq!(p, TftPresence { queue: Some(1230), progress: "InGame".into(), name: Some("Tester".into()) });
        assert!(p.in_game());
    }

    #[test]
    fn someone_else_or_no_tft() {
        assert_eq!(parse_tft_presence(&fixture(), "someone-else"), None);
        let mut f = fixture();
        f["presences"].as_array_mut().unwrap().retain(|p| p["product"] != PRODUCT_TFT);
        assert_eq!(parse_tft_presence(&f, ME), None);
    }

    #[test]
    fn tft_menu() {
        let p = json!({"presences": [{"puuid": ME, "product": PRODUCT_TFT, "game_name": "", "packedData": ""}]});
        let p = parse_tft_presence(&p, ME).unwrap();
        assert_eq!(p, TftPresence { queue: None, progress: String::new(), name: None });
        assert!(!p.in_game());
    }

    #[test]
    fn padded_packed_data() {
        let packed = base64::engine::general_purpose::STANDARD.encode(r#"{"activity":{"mode":"game_display_name_queueId_1100","progress":{"value":"InGame"}},"x":12}"#);
        assert!(packed.ends_with('='));
        let p = json!({"presences": [{"puuid": ME, "product": PRODUCT_TFT, "packedData": packed}]});
        assert_eq!(parse_tft_presence(&p, ME).unwrap().queue, Some(1100));
    }
}
