//! Eval crops for the replays (`TFT_CROPS=<dir>`): every unit crop used for naming, as embedded
//! (`units::crop`), saved as `<group>/<frame>_<x>_<y>.png` with its embedding beside it (`.f32`,
//! little-endian) and a line in `crops.jsonl`: `{file, frame, stage, who, quiet, group, x, y, star, bench, api, how}`
//! (`who`: the other player's name plate as read, if any; `quiet`: no red bars and every green bar full).
//! Groups: `mine` (my sure pieces the panel confirmed: what learning uses; a piece again only
//! when its embedding changed), `combat` (red bars of each fight's first read with every bar full,
//! not PvE), `scout` (another player's board with no fight on it, each unit with a full bar: quiet
//! planning, or after a fight, which the app doesn't read; a bar again only when its embedding changed). `api` is what the app named (mine: sure; others: looks or panel, else null).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use tft_model::{SeenUnit, TftSnapshot};

/// A bar of the read matches a piece or a named unit within this (base px).
const NEAR: f32 = 3.0;

pub struct Crops {
    dir: PathBuf,
    index: std::fs::File,
    /// Last embedding written per piece id / scouted bar place.
    last: BTreeMap<String, Vec<f32>>,
    /// Stage of the last fight written.
    fight: Option<(u32, u32)>,
    pub counts: BTreeMap<&'static str, u32>,
}

/// PvE rounds (minions, monsters): 1-2..1-4, then the 7th round of every later stage.
fn pve((stage, round): (u32, u32)) -> bool {
    (stage == 1 && round >= 2) || (stage >= 2 && round == 7)
}

/// Carousel rounds: 1-1, then the 4th round of every later stage.
fn carousel((stage, round): (u32, u32)) -> bool {
    (stage, round) == (1, 1) || (stage >= 2 && round == 4)
}

fn near(a: &SeenUnit, x: f32, y: f32) -> bool {
    (a.x - x).abs() <= NEAR && (a.y - y).abs() <= NEAR
}

impl Crops {
    /// From `TFT_CROPS`; None when unset.
    pub fn from_env() -> anyhow::Result<Option<Crops>> {
        std::env::var("TFT_CROPS").ok().map(|d| Crops::open(d.into())).transpose()
    }

    /// Crops into `dir` (`crops.jsonl` appended to: the replays may share a dir).
    pub fn open(dir: PathBuf) -> anyhow::Result<Crops> {
        for g in ["mine", "combat", "scout"] {
            std::fs::create_dir_all(dir.join(g))?;
        }
        let index = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crops.jsonl"))?;
        Ok(Crops { dir, index, last: BTreeMap::new(), fight: None, counts: BTreeMap::new() })
    }

    fn write(&mut self, f: &ocr::Frame, (frame, snap): (&str, &TftSnapshot), group: &'static str, u: &SeenUnit, api: Option<&str>, how: &str) -> anyhow::Result<()> {
        let scale = f.width as f32 / 2560.0;
        let c = tft_vision::units::crop(f, scale, u);
        let name = format!("{group}/{frame}_{:.0}_{:.0}", u.x, u.y);
        let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(self.dir.join(format!("{name}.png")))?), c.w as u32, c.h as u32);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()?.write_image_data(c.px.as_flattened())?;
        std::fs::write(self.dir.join(format!("{name}.f32")), u.emb.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())?;
        let j = serde_json::json!({"file": format!("{name}.png"), "frame": frame, "stage": snap.stage, "who": snap.viewing, "quiet": snap.enemies == 0 && snap.units.iter().all(|u| u.full), "group": group, "x": u.x, "y": u.y, "star": u.star, "bench": u.bench, "api": api, "how": how});
        writeln!(self.index, "{j}")?;
        *self.counts.entry(group).or_default() += 1;
        Ok(())
    }

    /// After `tracker.observe(snap)`: my confirmed pieces of a quiet read (`mine`).
    pub fn mine(&mut self, f: &ocr::Frame, frame: &str, snap: &TftSnapshot, tracker: &tft_coach::Tracker) -> anyhow::Result<()> {
        if snap.viewing_other || snap.enemies > 0 || snap.units.is_empty() || !snap.units.iter().all(|u| u.full) {
            return Ok(());
        }
        for q in tracker.board().pieces().iter().filter(|q| q.confirmed()) {
            let Some(u) = snap.units.iter().find(|u| u.bench == q.bench && (q.bench.is_some() || near(u, q.x, q.y))) else { continue };
            let key = format!("mine:{}", q.id);
            if u.emb.is_empty() || self.last.get(&key) == Some(&u.emb) {
                continue;
            }
            self.last.insert(key, u.emb.clone());
            let how = if q.named == tft_coach::board::Named::Bought { "$" } else { "=" };
            self.write(f, (frame, snap), "mine", u, q.api.as_deref(), how)?;
        }
        Ok(())
    }

    /// After `tracker.observe(snap)`: a fight's first full read (`combat`) or a scouted board (`scout`),
    /// named as the scout last read them.
    pub fn others(&mut self, f: &ocr::Frame, frame: &str, snap: &TftSnapshot, tracker: &tft_coach::Tracker) -> anyhow::Result<()> {
        let Some(stage) = snap.stage else { return Ok(()) };
        if carousel(stage) {
            return Ok(());
        }
        let read = tracker.scout().last_read();
        let named = |u: &SeenUnit| read.iter().find(|h| near(u, h.x, h.y)).map_or((None, "?"), |h| (h.api.clone(), h.how));
        let mine: Vec<(String, u32)> = tracker.panel().to_vec();
        let same_panel = !snap.traits.is_empty() && snap.traits.len() == mine.len() && snap.traits.iter().all(|t| mine.contains(t));
        if snap.viewing_other && snap.enemies == 0 {
            if same_panel {
                return Ok(());
            }
            for u in snap.units.iter().filter(|u| u.full) {
                let key = format!("scout:{:.0}:{:.0}", u.x, u.y);
                if u.emb.is_empty() || self.last.get(&key) == Some(&u.emb) {
                    continue;
                }
                self.last.insert(key, u.emb.clone());
                let (api, how) = named(u);
                self.write(f, (frame, snap), "scout", u, api.as_deref(), how)?;
            }
        } else if !snap.foes.is_empty() && !pve(stage) && self.fight != Some(stage) && snap.foes.iter().all(|u| u.full && !u.emb.is_empty()) {
            self.fight = Some(stage);
            for u in &snap.foes {
                let (api, how) = named(u);
                self.write(f, (frame, snap), "combat", u, api.as_deref(), how)?;
            }
        }
        Ok(())
    }
}
