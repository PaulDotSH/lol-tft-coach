//! OCR one region of a saved capture, enlarged: tune crop rectangles offline.
//! `cargo run -p tft-vision --release --example crop -- <png> <x> <y> <w> <h> <scale>`

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a[i].parse::<u32>().unwrap();
    let img = ocr::scoreboard::decode_png(&std::fs::read(&a[0])?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
    let frame = ocr::Frame { width: img.w, height: img.h, bgra };
    let mut f = tft_vision::crop(&frame, n(1), n(2), n(3), n(4), n(5));
    // Optional 6th arg: threshold → dark text on white.
    if let Some(t) = a.get(6).and_then(|t| t.parse::<u8>().ok()) {
        for p in f.bgra.chunks_mut(4) {
            let v = if p[0].max(p[1]).max(p[2]) >= t { 0 } else { 255 };
            (p[0], p[1], p[2]) = (v, v, v);
        }
    }
    // Optional 7th arg: tile the crop N times side by side (lone digits read badly).
    if let Some(k) = a.get(7).and_then(|t| t.parse::<u32>().ok()) {
        let (w, h) = (f.width, f.height);
        let mut bgra = Vec::with_capacity((w * k * h * 4) as usize);
        for y in 0..h {
            for _ in 0..k {
                bgra.extend_from_slice(&f.bgra[(y * w * 4) as usize..((y + 1) * w * 4) as usize]);
            }
        }
        f = ocr::Frame { width: w * k, height: h, bgra };
    }
    for b in ocr::recognize_boxes(&f)? {
        println!("{:>5.0} {:>5.0} {:>4.0}x{:<3.0} {}", b.x, b.y, b.w, b.h, b.text);
    }
    Ok(())
}
