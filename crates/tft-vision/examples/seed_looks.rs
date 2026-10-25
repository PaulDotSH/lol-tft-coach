//! Unit renders (tools/tft-render) composited into real planning frames and embedded as the app
//! embeds a unit: the render scaled to a height on the board, its feet on an empty spot, a bar
//! placed above it (not drawn: the crop starts below the bar), cropped by `units::crop`.
//! `cargo run -p tft-vision --release --example seed_looks -- <renders dir> <out dir> <capture dir>...`
//! Writes `<out>/embs.f32` (768 f32 per look, little endian) and `<out>/index.jsonl` (one line per
//! look: api, render, h, gap, dx, bg). Env: `TFT_MODEL` (required), `SEED_H` (unit heights in base
//! px, feet to top, default "100"), `SEED_GAP` (bar above the head, default "15"), `SEED_DX` (bar's
//! left end from the feet x − FULL_W/2, default "0"), `SEED_BG` (backgrounds per render, default 1),
//! `SEED_UNITS` (comma list of apis), `SEED_MATCH` (render file names containing all of these,
//! comma list, e.g. "_1.00_,flat"), `SEED_CROPS=<dir>` (also save the crops as PNG, for looking).

use std::io::Write;
use tft_model::SeenUnit;
use tft_vision::units;

/// A full bar's width (base px).
const BAR_W: f32 = 80.0;
/// Feet are placed in this part of my board (base px), on frames read as quiet planning.
const FEET: (f32, f32, f32, f32) = (700.0, 470.0, 1850.0, 820.0);
/// A spot is empty when no bar of the frame is this near the new bar (base px).
const CLEAR: (f32, f32) = (120.0, 150.0);
/// Backgrounds kept, and capture frames skipped between candidates.
const BACKGROUNDS: usize = 40;
const EVERY: usize = 5;

struct Render {
    api: String,
    file: String,
    w: usize,
    h: usize,
    /// Premultiplied RGBA.
    px: Vec<[f32; 4]>,
    feet: (f32, f32),
    /// Feet to mesh top (px) of this variant at size 1.0.
    base_h: f32,
}

fn env_list(k: &str, d: &str) -> Vec<f32> {
    std::env::var(k).unwrap_or(d.into()).split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

/// Area-average shrink by `s` (< 1), premultiplied.
fn shrink(r: &Render, s: f32) -> (usize, usize, Vec<[f32; 4]>) {
    let (w, h) = (((r.w as f32) * s).ceil() as usize, ((r.h as f32) * s).ceil() as usize);
    let mut out = vec![[0.0; 4]; w * h];
    for y in 0..h {
        let (sy0, sy1) = ((y as f32 / s) as usize, (((y + 1) as f32 / s) as usize).min(r.h).max((y as f32 / s) as usize + 1));
        for x in 0..w {
            let (sx0, sx1) = ((x as f32 / s) as usize, (((x + 1) as f32 / s) as usize).min(r.w).max((x as f32 / s) as usize + 1));
            let mut acc = [0.0f32; 4];
            let mut n = 0.0;
            for sy in sy0..sy1.min(r.h) {
                for sx in sx0..sx1.min(r.w) {
                    let p = r.px[sy * r.w + sx];
                    (0..4).for_each(|c| acc[c] += p[c]);
                    n += 1.0;
                }
            }
            if n > 0.0 {
                out[y * w + x] = acc.map(|v| v / n);
            }
        }
    }
    (w, h, out)
}

fn load_renders(dir: &std::path::Path, units: &[String], want: &[String]) -> anyhow::Result<Vec<Render>> {
    let mut out = vec![];
    let mut dirs: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        let Ok(meta) = std::fs::read_to_string(d.join("meta.json")) else { continue };
        let meta: serde_json::Value = serde_json::from_str(&meta)?;
        let api = meta["api"].as_str().unwrap_or_default().to_string();
        if !units.is_empty() && !units.contains(&api) {
            continue;
        }
        let feet = (meta["feet_px"][0].as_f64().unwrap_or(0.0) as f32, meta["feet_px"][1].as_f64().unwrap_or(0.0) as f32);
        let renders = meta["renders"].as_object().cloned().unwrap_or_default();
        // Each variant's height at size 1.0: the median over its turns.
        let mut heights: std::collections::HashMap<String, Vec<f32>> = Default::default();
        for r in renders.values() {
            if r["size"].as_f64() == Some(1.0) && !r["clipped"].as_bool().unwrap_or(false) {
                let top = r["mesh_top_px"].as_f64().unwrap_or(0.0) as f32;
                heights.entry(r["variant"].as_str().unwrap_or_default().into()).or_default().push(feet.1 - top);
            }
        }
        let mut names: Vec<&String> = renders.keys().collect();
        names.sort();
        for name in names {
            let r = &renders[name];
            if r["clipped"].as_bool().unwrap_or(false) || !want.iter().all(|w| name.contains(w.as_str())) {
                continue;
            }
            let Some(hs) = heights.get_mut(r["variant"].as_str().unwrap_or_default()) else { continue };
            hs.sort_by(f32::total_cmp);
            let base_h = hs[hs.len() / 2];
            let img = ocr::scoreboard::decode_png(&std::fs::read(d.join(name))?).ok_or_else(|| anyhow::anyhow!("bad png {name}"))?;
            let px = img.px.as_chunks::<4>().0.iter().map(|p| {
                let a = p[3] as f32 / 255.0;
                [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, a]
            });
            out.push(Render { api: api.clone(), file: format!("{api}/{name}"), w: img.w as usize, h: img.h as usize, px: px.collect(), feet, base_h });
        }
    }
    Ok(out)
}

struct Background {
    frame: ocr::Frame,
    bars: Vec<SeenUnit>,
}

fn backgrounds(dirs: &[String]) -> anyhow::Result<Vec<Background>> {
    let mut files: Vec<std::path::PathBuf> = vec![];
    for d in dirs {
        let mut v: Vec<_> = std::fs::read_dir(d)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "png")).collect();
        v.sort();
        files.extend(v);
    }
    let mut out = vec![];
    let step = (files.len() / (BACKGROUNDS * EVERY)).max(1) * EVERY;
    for p in files.iter().step_by(step) {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        if img.w != 2560 {
            continue;
        }
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let frame = ocr::Frame { width: img.w, height: img.h, bgra };
        let (bars, enemies) = units::read(&frame, 1.0);
        if enemies == 0 && bars.len() >= 3 && bars.iter().all(|b| b.full) {
            out.push(Background { frame, bars });
        }
        if out.len() >= BACKGROUNDS {
            break;
        }
    }
    Ok(out)
}

/// A tiny LCG: the same spots every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() >= 3, "seed_looks <renders dir> <out dir> <capture dir>...");
    let units: Vec<String> = std::env::var("SEED_UNITS").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
    let want: Vec<String> = std::env::var("SEED_MATCH").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
    let (hs, gaps, dxs) = (env_list("SEED_H", "100"), env_list("SEED_GAP", "15"), env_list("SEED_DX", "0"));
    let per = std::env::var("SEED_BG").ok().and_then(|v| v.parse().ok()).unwrap_or(1usize);
    let save = std::env::var("SEED_CROPS").ok();
    let renders = load_renders(std::path::Path::new(&args[0]), &units, &want)?;
    let mut bgs = backgrounds(&args[2..])?;
    eprintln!("{} renders, {} backgrounds", renders.len(), bgs.len());
    anyhow::ensure!(!bgs.is_empty(), "no quiet planning frames");
    let mut embedder = tft_embed::Embedder::load(std::path::Path::new(&std::env::var("TFT_MODEL")?))?;
    let out = std::path::PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out)?;
    if let Some(s) = &save {
        std::fs::create_dir_all(s)?;
    }
    let mut embs = std::io::BufWriter::new(std::fs::File::create(out.join("embs.f32"))?);
    let mut index = std::io::BufWriter::new(std::fs::File::create(out.join("index.jsonl"))?);
    let mut rng = Rng(18);
    let (mut batch, mut meta): (Vec<tft_embed::Rgb>, Vec<serde_json::Value>) = (vec![], vec![]);
    let mut flush = |batch: &mut Vec<tft_embed::Rgb>, meta: &mut Vec<serde_json::Value>| -> anyhow::Result<()> {
        for (e, m) in embedder.embed(batch)?.iter().zip(meta.iter()) {
            e.iter().try_for_each(|v| embs.write_all(&v.to_le_bytes()))?;
            writeln!(index, "{m}")?;
        }
        batch.clear();
        meta.clear();
        Ok(())
    };
    let mut n = 0;
    for r in &renders {
        for &h in &hs {
            let s = h / r.base_h;
            let (sw, sh, small) = shrink(r, s);
            let (fx, fy) = (r.feet.0 * s, r.feet.1 * s);
            for &gap in &gaps {
                for &dx in &dxs {
                    for _ in 0..per {
                        let k = (rng.next() * bgs.len() as f32) as usize % bgs.len();
                        let bg = &mut bgs[k];
                        // An empty spot: the new bar clear of every bar on the frame.
                        let mut spot = None;
                        for _ in 0..200 {
                            let (px, py) = (FEET.0 + rng.next() * (FEET.2 - FEET.0), FEET.1 + rng.next() * (FEET.3 - FEET.1));
                            let (bx, by) = (px - BAR_W / 2.0 + dx, py - h - gap);
                            if bg.bars.iter().all(|b| (b.x - bx).abs() > CLEAR.0 || (b.y - by).abs() > CLEAR.1) {
                                spot = Some((px, py, bx, by));
                                break;
                            }
                        }
                        let Some((px, py, bx, by)) = spot else { continue };
                        // Paste over the crop window only, then put the frame back.
                        let (x0, y0) = ((bx + units::CROP.0).max(0.0) as u32, (by + units::CROP.1).max(0.0) as u32);
                        let (x1, y1) = (((bx + units::CROP.2) as u32).min(bg.frame.width), ((by + units::CROP.3) as u32).min(bg.frame.height));
                        let saved: Vec<u8> = (y0..y1).flat_map(|y| {
                            let i = ((y * bg.frame.width + x0) * 4) as usize;
                            bg.frame.bgra[i..i + ((x1 - x0) * 4) as usize].to_vec()
                        }).collect();
                        let (ox, oy) = ((px - fx).round() as i32, (py - fy).round() as i32);
                        for y in y0..y1 {
                            for x in x0..x1 {
                                let (sx, sy) = (x as i32 - ox, y as i32 - oy);
                                if sx < 0 || sy < 0 || sx >= sw as i32 || sy >= sh as i32 {
                                    continue;
                                }
                                let p = small[sy as usize * sw + sx as usize];
                                let i = ((y * bg.frame.width + x) * 4) as usize;
                                for (c, ch) in [(0, 2), (1, 1), (2, 0)] {
                                    let v = p[c] + bg.frame.bgra[i + ch] as f32 * (1.0 - p[3]);
                                    bg.frame.bgra[i + ch] = v.round().clamp(0.0, 255.0) as u8;
                                }
                            }
                        }
                        let u = SeenUnit { x: bx, y: by, star: 1, bench: None, full: true, items: vec![], emb: vec![] };
                        let crop = units::crop(&bg.frame, 1.0, &u);
                        let row = ((x1 - x0) * 4) as usize;
                        for (k, y) in (y0..y1).enumerate() {
                            let i = ((y * bg.frame.width + x0) * 4) as usize;
                            bg.frame.bgra[i..i + row].copy_from_slice(&saved[k * row..(k + 1) * row]);
                        }
                        if let Some(s) = &save {
                            let name = format!("{}/{}_{n}.png", s, r.file.replace(['/', '\\'], "__").trim_end_matches(".png"));
                            let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&name)?), crop.w as u32, crop.h as u32);
                            e.set_color(png::ColorType::Rgb);
                            e.set_depth(png::BitDepth::Eight);
                            e.write_header()?.write_image_data(&crop.px.concat())?;
                        }
                        batch.push(crop);
                        meta.push(serde_json::json!({"api": r.api, "render": r.file, "h": h, "gap": gap, "dx": dx, "bg": k}));
                        n += 1;
                        if batch.len() == 32 {
                            flush(&mut batch, &mut meta)?;
                        }
                    }
                }
            }
        }
    }
    flush(&mut batch, &mut meta)?;
    eprintln!("{n} looks");
    Ok(())
}
