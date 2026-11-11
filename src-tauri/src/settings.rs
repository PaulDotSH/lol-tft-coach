//! Settings (`%APPDATA%\lol-coach\settings.toml`) and app directories.

use model::Settings;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("lol-coach")
}

/// `%LOCALAPPDATA%\lol-coach\<sub>`, created on demand.
pub fn local_dir(sub: &str) -> PathBuf {
    let dir = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("lol-coach").join(sub);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn load() -> Settings {
    std::fs::read_to_string(config_dir().join("settings.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(s: &Settings) -> anyhow::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("settings.toml"), toml::to_string(s)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_is_opt_in() {
        assert!(!Settings::default().overlay);
        let old: Settings = toml::from_str("tier = \"all\"\nregion = \"all\"\nocr_enabled = true\nrecord_fixtures = false\n").unwrap();
        assert!(!old.overlay);
        assert!(old.audio && old.gank_warn && old.game_note);
        assert!(!old.tft_crops && !Settings::default().tft_crops);
    }

    #[test]
    fn win_probability_is_shown_and_posture_calls_are_silent_by_default() {
        let old: Settings = toml::from_str("tier = \"all\"\nregion = \"all\"\nocr_enabled = true\nrecord_fixtures = false\n").unwrap();
        assert!(old.win_prob && !old.posture_calls);
        assert!(Settings::default().win_prob && !Settings::default().posture_calls);
    }
}
