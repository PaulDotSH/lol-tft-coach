//! The player's minimap and HUD settings from League's `PersistedSettings.json`
//! (`files[Game.cfg].sections[HUD].settings`, each `{ "name", "value" }`), so the minimap's
//! on-screen size and orientation, and the HUD scale the screen readers calibrate under,
//! are known without any clicks. Read-only.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Minimap size when the file can't be read: League's default slider position.
pub const DEFAULT_SCALE: f32 = 1.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapSettings {
    /// `MinimapScale`, 0..=2.
    pub scale: f32,
    /// "Flip minimap" option on.
    pub flipped: bool,
    /// HUD scale slider (`GlobalScale`, 0..=1); None when the file doesn't say.
    pub hud_scale: Option<f32>,
}

const DEFAULTS: MinimapSettings = MinimapSettings { scale: DEFAULT_SCALE, flipped: false, hud_scale: None };

/// None when `json` isn't valid JSON; missing settings take defaults (DEFAULT_SCALE, not flipped).
pub fn parse(json: &str) -> Option<MinimapSettings> {
    let v: Value = serde_json::from_str(json).ok()?;
    let scale = find(&v, &|k| k.eq_ignore_ascii_case("MinimapScale"))
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|s| *s >= 0.0)
        .unwrap_or(DEFAULT_SCALE)
        .clamp(0.0, 2.0);
    let flipped = find(&v, &|k| k.eq_ignore_ascii_case("FlipMiniMap")).is_some_and(|s| truthy(&s));
    let hud_scale = find(&v, &|k| k.eq_ignore_ascii_case("GlobalScale")).and_then(|s| s.trim().parse::<f32>().ok()).filter(|s| s.is_finite()).map(|s| s.clamp(0.0, 1.0));
    Some(MinimapSettings { scale, flipped, hud_scale })
}

/// Reads `<install_dir>\Config\PersistedSettings.json` first when given, then the fallback install paths; defaults when none can be read.
pub fn minimap_settings(install_dir: Option<&Path>) -> MinimapSettings {
    let first = install_dir.map(|d| d.join("Config").join("PersistedSettings.json"));
    first.into_iter().chain(candidates()).find_map(|p| std::fs::read_to_string(p).ok().and_then(|t| parse(&t))).unwrap_or(DEFAULTS)
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")
}

/// First value whose `name` (or `key`) matches `pred` in a `{name, value}` pair, anywhere in the tree.
fn find(v: &Value, pred: &dyn Fn(&str) -> bool) -> Option<String> {
    match v {
        Value::Object(map) => {
            if let (Some(Value::String(k)), Some(val)) = (map.get("name").or_else(|| map.get("key")), map.get("value"))
                && pred(k)
            {
                return Some(scalar(val));
            }
            map.values().find_map(|c| find(c, pred))
        }
        Value::Array(items) => items.iter().find_map(|i| find(i, pred)),
        _ => None,
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn candidates() -> Vec<PathBuf> {
    const REL: &str = r"Riot Games\League of Legends\Config\PersistedSettings.json";
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("LOL_COACH_LEAGUE_CFG") {
        out.push(PathBuf::from(p));
    }
    for root in [r"C:\", r"C:\Program Files\", r"C:\Program Files (x86)\", r"D:\", r"D:\Games\", r"E:\"] {
        out.push(PathBuf::from(root).join(REL));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        std::fs::read_to_string(format!("{}/../../fixtures/clients/persisted_settings.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn real_file_values() {
        assert_eq!(parse(&fixture()), Some(MinimapSettings { scale: 1.0, flipped: false, hud_scale: None }));
    }

    #[test]
    fn flip_truthy() {
        let flipped = fixture().replacen(r#""value": "0""#, r#""value": "1""#, 1);
        assert_ne!(flipped, fixture());
        assert_eq!(parse(&flipped).map(|s| s.flipped), Some(true));
    }

    #[test]
    fn legacy_key_value_shape() {
        let j = r#"{"sections":[{"settings":[{"key":"MinimapScale","value":"0.8"},{"key":"FlipMiniMap","value":"true"}]}]}"#;
        assert_eq!(parse(j), Some(MinimapSettings { scale: 0.8, flipped: true, hud_scale: None }));
    }

    #[test]
    fn spectator_scale_and_move_self_do_not_count() {
        let j = r#"[{"name":"MinimapScaleSpectator","value":"1.5"},{"name":"MinimapMoveSelf","value":"1"},{"name":"GlobalScaleReplay","value":"0.5"}]"#;
        assert_eq!(parse(j), Some(DEFAULTS));
    }

    #[test]
    fn hud_scale_from_global_scale() {
        let j = r#"{"files":[{"name":"Game.cfg","sections":[{"name":"HUD","settings":[{"name":"GlobalScaleReplay","value":"1.0000"},{"name":"GlobalScale","value":"0.1000"}]}]}]}"#;
        assert_eq!(parse(j).and_then(|s| s.hud_scale), Some(0.1));
        assert_eq!(parse(r#"[{"name":"GlobalScale","value":"7"}]"#).and_then(|s| s.hud_scale), Some(1.0));
    }

    #[test]
    fn invalid_and_empty() {
        assert_eq!(parse("not json"), None);
        assert_eq!(parse("{}"), Some(DEFAULTS));
    }
}
