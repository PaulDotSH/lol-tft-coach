//! Debug mode (`--debug` or `LOL_COACH_DEBUG=1`): verbose logs plus dumps of raw
//! API responses, advice and OCR frames into `%LOCALAPPDATA%\lol-coach\debug\<session>\`.
//! Each kind is written only when its content changed and at most every `every`,
//! with a cap on files per session, so a whole game stays small.

use crate::settings::local_dir;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_FILES: u32 = 4000;
const MAX_FRAMES: u32 = 300;

struct Session {
    dir: PathBuf,
    /// kind → (last write, content hash).
    last: Mutex<HashMap<String, (Instant, u64)>>,
    files: Mutex<(u32, u32)>,
}

static SESSION: OnceLock<Option<Session>> = OnceLock::new();

/// Decide once at startup; returns the session folder when debug mode is on.
pub fn init() -> Option<&'static Path> {
    SESSION
        .get_or_init(|| {
            let on = std::env::var("LOL_COACH_DEBUG").is_ok_and(|v| !v.is_empty() && v != "0")
                || std::env::args().any(|a| a == "--debug");
            on.then(|| {
                let dir = local_dir("debug").join(stamp());
                let _ = std::fs::create_dir_all(&dir);
                Session { dir, last: Mutex::default(), files: Mutex::default() }
            })
        })
        .as_ref()
        .map(|s| s.dir.as_path())
}

pub fn on() -> bool {
    dir().is_some()
}

pub fn dir() -> Option<&'static Path> {
    SESSION.get().and_then(|s| s.as_ref()).map(|s| s.dir.as_path())
}

/// Path for a new `kind` file, or None if unchanged, too soon, or over the cap.
fn slot(kind: &str, every: Duration, hash: u64, ext: &str, frame: bool) -> Option<PathBuf> {
    let s = SESSION.get()?.as_ref()?;
    {
        let mut last = s.last.lock().unwrap();
        if let Some((at, h)) = last.get(kind)
            && (*h == hash || at.elapsed() < every)
        {
            return None;
        }
        last.insert(kind.to_string(), (Instant::now(), hash));
    }
    let mut n = s.files.lock().unwrap();
    if n.0 >= MAX_FILES || (frame && n.1 >= MAX_FRAMES) {
        return None;
    }
    n.0 += 1;
    n.1 += frame as u32;
    Some(s.dir.join(format!("{}-{kind}.{ext}", millis())))
}

/// Pretty JSON of `v` as `<ms>-<kind>.json`.
pub fn dump(kind: &str, every: Duration, v: &impl serde::Serialize) {
    if !on() {
        return;
    }
    let Ok(json) = serde_json::to_string_pretty(v) else { return };
    if let Some(p) = slot(kind, every, hash(&json), "json", false) {
        let _ = std::fs::write(p, json);
    }
}

/// PNG of a captured frame as `<ms>-<kind>.png`.
pub fn frame(kind: &str, every: Duration, f: &ocr::Frame) {
    if !on() {
        return;
    }
    // Sample every 97th byte: cheap and enough to skip identical frames.
    let h = hash(&(f.width, f.height, f.bgra.iter().step_by(97).copied().collect::<Vec<u8>>()));
    let Some(p) = slot(kind, every, h, "png", true) else { return };
    let rgba: Vec<u8> = f.bgra.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], 255]).collect();
    let write = || -> anyhow::Result<()> {
        let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&p)?), f.width, f.height);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()?.write_image_data(&rgba)?;
        Ok(())
    };
    if let Err(e) = write() {
        tracing::warn!("debug frame {p:?}: {e:#}");
    }
}

/// Once per session: raw samples of the LCU endpoints whose schemas were built
/// from docs, so parsers can be checked against the real client.
pub async fn probe_lcu(lcu: clients::lcu::Lcu) {
    const PATHS: [&str; 4] = [
        "/lol-summoner/v1/current-summoner",
        "/lol-gameflow/v1/session",
        "/lol-end-of-game/v1/eog-stats-block",
        "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex=3",
    ];
    let mut first_game = None;
    for path in PATHS {
        match lcu.get(path).await {
            Ok(Some(v)) => {
                if path.contains("matches") {
                    first_game = v.pointer("/games/games/0/gameId").and_then(|g| g.as_u64());
                }
                dump(&format!("lcu-probe-{}", sanitize(path)), Duration::ZERO, &v);
            }
            Ok(None) => tracing::debug!("lcu probe {path}: 404"),
            Err(e) => tracing::warn!("lcu probe {path}: {e:#}"),
        }
    }
    if let Some(id) = first_game {
        for path in [format!("/lol-match-history/v1/games/{id}"), format!("/lol-match-history/v1/game-timelines/{id}")] {
            match lcu.get(&path).await {
                Ok(Some(v)) => dump(&format!("lcu-probe-{}", sanitize(&path)), Duration::ZERO, &v),
                Ok(None) => tracing::debug!("lcu probe {path}: 404"),
                Err(e) => tracing::warn!("lcu probe {path}: {e:#}"),
            }
        }
    }
    tracing::info!("lcu probe done");
}

fn sanitize(path: &str) -> String {
    path.split('?').next().unwrap_or(path).trim_matches('/').replace(['/', '{', '}'], "_")
}

fn hash(v: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

/// Local-time-free sortable session name (unix seconds).
fn stamp() -> String {
    format!("{}", millis() / 1000)
}
