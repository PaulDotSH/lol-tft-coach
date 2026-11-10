//! Screen geometry calibration (`%LOCALAPPDATA%\lol-coach\calib.json`): where the screen readers
//! last found their elements, per game, client size and HUD scale. A stored value is the expected
//! position next session; locators keep refining it with an exponential average, so one bad frame
//! can't move it far. An entry last checked on another patch is unverified until the next find.
//! A missing or corrupt file starts from the defaults; deleting it is always safe.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Weight of a new measurement in the stored average.
pub const ALPHA: f32 = 0.2;
/// Changes are written at most this often.
const SAVE_EVERY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub s: f32,
    pub cx: f32,
    pub cy: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Patch of the last successful locate.
    #[serde(default)]
    pub patch: String,
    /// Mayhem offer title strip, fractions [x0, y0, x1, y1] of the client area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer_strip: Option<[f32; 4]>,
    /// Tab scoreboard: pixels per source unit and centre, client pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoreboard: Option<Board>,
    /// Unix seconds of the last locate.
    #[serde(default)]
    pub seen: u64,
}

/// game ("lol") → key → entry.
type Games = BTreeMap<String, BTreeMap<String, Entry>>;

#[derive(Default)]
pub struct Calib {
    games: Games,
    path: Option<PathBuf>,
    dirty: bool,
    saved: Option<Instant>,
}

/// Entry key: client size and HUD scale, e.g. "2560x1440@0.10" ("@?" when the HUD scale is unknown).
pub fn key(w: u32, h: u32, hud: Option<f32>) -> String {
    match hud {
        Some(s) => format!("{w}x{h}@{s:.2}"),
        None => format!("{w}x{h}@?"),
    }
}

fn ema<const N: usize>(old: Option<[f32; N]>, new: [f32; N], set: bool) -> [f32; N] {
    match old {
        Some(o) if !set => std::array::from_fn(|i| o[i] + ALPHA * (new[i] - o[i])),
        _ => new,
    }
}

impl Calib {
    /// The store at `path`; empty when the file is missing or unreadable.
    pub fn load(path: &Path) -> Calib {
        let games = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("calibration {}: {e}, starting from the defaults", path.display());
                Games::default()
            }),
            Err(_) => Games::default(),
        };
        Calib { games, path: Some(path.to_path_buf()), dirty: false, saved: None }
    }

    pub fn get(&self, game: &str, key: &str) -> Option<&Entry> {
        self.games.get(game)?.get(key)
    }

    /// The patch an entry was last checked on, when that isn't `patch` (unverified since then).
    pub fn unverified(&self, game: &str, key: &str, patch: &str) -> Option<String> {
        self.get(game, key).map(|e| e.patch.clone()).filter(|p| p != patch)
    }

    fn entry(&mut self, game: &str, key: &str, patch: &str, now: u64) -> &mut Entry {
        self.dirty = true;
        let e = self.games.entry(game.to_string()).or_default().entry(key.to_string()).or_default();
        e.patch = patch.to_string();
        e.seen = now;
        e
    }

    /// A located offer strip: averaged into the stored one, or replacing it when `set`.
    pub fn offer_strip(&mut self, game: &str, key: &str, patch: &str, now: u64, strip: [f32; 4], set: bool) -> [f32; 4] {
        let e = self.entry(game, key, patch, now);
        let v = ema(e.offer_strip, strip, set);
        e.offer_strip = Some(v);
        v
    }

    /// A fitted scoreboard: averaged into the stored one, or replacing it when `set`.
    pub fn scoreboard(&mut self, game: &str, key: &str, patch: &str, now: u64, b: Board, set: bool) -> Board {
        let e = self.entry(game, key, patch, now);
        let [s, cx, cy] = ema(e.scoreboard.map(|o| [o.s, o.cx, o.cy]), [b.s, b.cx, b.cy], set);
        e.scoreboard = Some(Board { s, cx, cy });
        e.scoreboard.unwrap()
    }

    /// Write changes, at most every `SAVE_EVERY` unless `now`.
    pub fn save(&mut self, now: bool) {
        if !self.dirty || (!now && self.saved.is_some_and(|t| t.elapsed() < SAVE_EVERY)) {
            return;
        }
        let Some(path) = &self.path else { return };
        self.saved = Some(Instant::now());
        self.dirty = false;
        match serde_json::to_string_pretty(&self.games) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("calibration {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!("calibration: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: &str = "2560x1440@0.10";

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("lol-coach-calib-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn keys_name_size_and_hud_scale() {
        assert_eq!(key(2560, 1440, Some(0.1)), K);
        assert_eq!(key(1920, 1080, None), "1920x1080@?");
    }

    #[test]
    fn unknown_key_has_no_entry() {
        let c = Calib::default();
        assert!(c.get("lol", K).is_none());
        assert_eq!(c.unverified("lol", K, "16.22"), None);
    }

    #[test]
    fn patch_change_marks_unverified_until_the_next_find() {
        let mut c = Calib::default();
        c.offer_strip("lol", K, "16.22", 1, [0.18, 0.365, 0.82, 0.462], false);
        assert_eq!(c.unverified("lol", K, "16.22"), None);
        assert_eq!(c.unverified("lol", K, "16.23").as_deref(), Some("16.22"));
        c.scoreboard("lol", K, "16.23", 2, Board { s: 1.2, cx: 1280.0, cy: 720.0 }, false);
        assert_eq!(c.unverified("lol", K, "16.23"), None);
        assert_eq!(c.unverified("lol", "1920x1080@?", "16.23"), None);
    }

    #[test]
    fn one_outlier_moves_the_value_by_one_averaging_step() {
        let mut c = Calib::default();
        let b = Board { s: 1.2, cx: 1280.0, cy: 720.0 };
        c.scoreboard("lol", K, "16.22", 1, b, false);
        let got = c.scoreboard("lol", K, "16.22", 2, Board { s: 2.0, cx: 0.0, cy: 720.0 }, false);
        assert!((got.s - (1.2 + ALPHA * 0.8)).abs() < 1e-5 && (got.cx - 1280.0 * (1.0 - ALPHA)).abs() < 1e-3, "{got:?}");
        assert_eq!(c.scoreboard("lol", K, "16.22", 3, b, true), b);
        let strip = c.offer_strip("lol", K, "16.22", 4, [0.2, 0.4, 0.8, 0.5], false);
        assert_eq!(strip, [0.2, 0.4, 0.8, 0.5]);
        let strip = c.offer_strip("lol", K, "16.22", 5, [0.2, 0.9, 0.8, 1.0], false);
        assert!((strip[1] - (0.4 + ALPHA * 0.5)).abs() < 1e-5);
    }

    #[test]
    fn saves_and_loads_and_ignores_a_corrupt_file() {
        let p = temp("roundtrip");
        let mut c = Calib::load(&p);
        c.offer_strip("lol", K, "16.22", 7, [0.18, 0.365, 0.82, 0.462], false);
        c.save(true);
        let back = Calib::load(&p);
        assert_eq!(back.get("lol", K), c.get("lol", K));
        assert_eq!(back.get("lol", K).unwrap().seen, 7);
        std::fs::write(&p, "{not json").unwrap();
        assert!(Calib::load(&p).get("lol", K).is_none());
        std::fs::write(&p, r#"{"lol": {"x": {"patch": "16.22", "offer_strip": "bad"}}}"#).unwrap();
        assert!(Calib::load(&p).get("lol", "x").is_none());
        let _ = std::fs::remove_file(&p);
        assert!(Calib::load(&temp("missing")).get("lol", K).is_none());
    }
}
