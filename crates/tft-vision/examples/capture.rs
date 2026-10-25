//! Calibration captures: while the League window is up, save a full-window PNG every
//! few seconds (unchanged frames skipped). Never sends input or takes focus.
//! `cargo run -p tft-vision --release --example capture -- <out dir> [seconds]`

use ocr::scoreboard::Rect;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let every = Duration::from_secs(a.get(1).and_then(|s| s.parse().ok()).unwrap_or(6));
    std::fs::create_dir_all(&dir)?;
    let mut last = 0u64;
    loop {
        if let Some((w, h)) = ocr::league_client()
            && let Some(f) = ocr::capture_client(Rect { x: 0, y: 0, w: w as i32, h: h as i32 })
        {
            // Sample every 97th byte: cheap and enough to skip identical frames.
            let sig = f.bgra.iter().step_by(97).fold(0u64, |s, &b| s.wrapping_mul(31).wrapping_add(b as u64));
            if sig != last {
                last = sig;
                let ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                let rgba: Vec<u8> = f.bgra.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], 255]).collect();
                let p = dir.join(format!("{ms}.png"));
                let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&p)?), f.width, f.height);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header()?.write_image_data(&rgba)?;
                println!("{}", p.display());
            }
        }
        std::thread::sleep(every);
    }
}
