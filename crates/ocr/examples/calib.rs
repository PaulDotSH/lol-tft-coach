//! Run the screen locators over a debug session folder and print what each found.
//! `cargo run -p ocr --release --example calib -- <debug session dir> [manifest.json]`
//! Frames by kind: `ocr-offer` / `ocr-partial` (offer strip), `ocr-wide*` (wide band), `tab-board`
//! (scoreboard crop of a 16:9 window), `tab-wide-miss` (whole window), `minimap`.

use ocr::offer::{self, PITCH, STRIP, TITLE_TOP, WIDE};
use ocr::scoreboard::{Geometry, Rect, decode_png};
use ocr::{Frame, minimap};
use std::collections::HashMap;

fn frame(path: &std::path::Path) -> Option<Frame> {
    let img = decode_png(&std::fs::read(path).ok()?)?;
    Some(Frame { width: img.w, height: img.h, bgra: img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect() })
}

fn offer_row(f: &Frame, band: [f32; 4], names: &HashMap<u32, String>) -> String {
    let client = ((f.width as f32 / (band[2] - band[0])).round() as u32, (f.height as f32 / (band[3] - band[1])).round() as u32);
    let at = offer::rect(client.0, client.1, band);
    let boxes = match ocr::recognize_boxes(f) {
        Ok(b) => b,
        Err(e) => return format!("ocr failed: {e}"),
    };
    let cards = offer::locate(&boxes, names, Rect { w: f.width as i32, h: f.height as i32, ..at }, client);
    if cards.is_empty() {
        return format!("not found ({} lines)", boxes.len());
    }
    let top = cards.iter().map(|c| c.y).fold(f32::MAX, f32::min);
    let dx: Vec<String> = cards
        .iter()
        .map(|c| {
            let want = (0..3).map(|i| 0.5 + (i as f32 - 1.0) * PITCH).min_by(|a, b| (a - c.x).abs().total_cmp(&(b - c.x).abs())).unwrap();
            format!("{:+.1}", (c.x - want) * 100.0)
        })
        .collect();
    format!("{} cards, scale {:.2}, title top {:+.1} % h, x off [{}] % w", cards.len(), cards[0].found.scale, (top - TITLE_TOP) * 100.0, dx.join(" "))
}

fn board_row(f: &Frame, whole: bool) -> String {
    let (client, origin) = if whole {
        ((f.width, f.height), (0, 0))
    } else {
        let s = f.width as f32 / (1563.0 - 37.0);
        let h = (1200.0 * s).round() as u32;
        let client = ((h as f32 * 16.0 / 9.0).round() as u32, h);
        let b = Geometry::new(client.0, client.1).board();
        (client, (b.x, b.y))
    };
    let prior = Geometry::new(client.0, client.1);
    match Geometry::fit(f, origin, client, &prior) {
        Some((g, found)) => format!(
            "found at {:.2}x, centre {:+.0},{:+.0} px, {:.0} % rows ({}x{})",
            found.scale,
            g.cx - prior.cx,
            g.cy - prior.cy,
            found.score * 100.0,
            client.0,
            client.1
        ),
        None => format!("not found ({}x{})", client.0, client.1),
    }
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = a.first().ok_or_else(|| anyhow::anyhow!("usage: calib <debug session dir> [manifest.json]"))?;
    let manifest = a.get(1).cloned().unwrap_or_else(|| format!("{}/../../fixtures/data/ugg-augment-manifest-16_21.json", env!("CARGO_MANIFEST_DIR")));
    let names: HashMap<u32, String> = serde_json::from_str::<HashMap<String, String>>(&std::fs::read_to_string(manifest)?)?
        .into_iter()
        .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
        .collect();
    let mut paths: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "png")).collect();
    paths.sort();
    let (mut found, mut total) = (0, 0);
    for p in paths {
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let kind = name.split_once('-').map_or("", |k| k.1).trim_end_matches(".png");
        let Some(f) = frame(&p) else { continue };
        let row = match kind {
            "ocr-offer" | "ocr-partial" => offer_row(&f, STRIP, &names),
            k if k.starts_with("ocr-wide") => offer_row(&f, WIDE, &names),
            "tab-board" => board_row(&f, false),
            "tab-wide-miss" => board_row(&f, true),
            "minimap" => match minimap::find_square(&f) {
                Some(sq) => format!("map square at {:.0},{:.0} side {:.0}", sq.x, sq.y, sq.side),
                None => "not found (fallback place)".to_string(),
            },
            _ => continue,
        };
        total += 1;
        found += usize::from(!row.starts_with("not found") && !row.starts_with("ocr failed"));
        println!("{name:<40} {kind:<14} {row}");
    }
    println!("{found} of {total} frames located");
    Ok(())
}
