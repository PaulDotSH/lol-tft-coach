//! Average a team-size glyph template over captures that show it at the same place:
//! `team_glyphs -- <style 0 dark|1 bright> <glyph> <x0> <x1> <png>...` (x at 2560 width).
//! Prints a "glyph style w h values" line for `src/team.txt`.

use tft_vision::team::{BAND, REGION, maps};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (style, glyph): (usize, &str) = (a[0].parse()?, &a[1]);
    let (x0, x1): (u32, u32) = (a[2].parse()?, a[3].parse()?);
    let (hx, w) = (((x0 - REGION.0) / 2) as usize, ((x1 - x0) / 2) as usize);
    let (y0, h) = (((BAND.0 - REGION.1) / 2) as usize, ((BAND.1 - BAND.0) / 2) as usize);
    let mut sum = vec![0f32; w * h];
    for p in &a[4..] {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let f = ocr::Frame { width: img.w, height: img.h, bgra };
        let m = &maps(&f, f.width as f32 / 2560.0)[style];
        for y in 0..h {
            for x in 0..w {
                sum[y * w + x] += m.v[(y0 + y) * m.w + hx + x];
            }
        }
    }
    let top = sum.iter().copied().fold(1e-6, f32::max);
    let v: String = sum.iter().map(|s| char::from(b'0' + (s / top * 9.0).round() as u8)).collect();
    println!("{glyph} {style} {w} {h} {v}");
    Ok(())
}
