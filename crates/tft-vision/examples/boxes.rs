//! Print the OCR boxes of a saved TFT capture, in client pixels.
//! `cargo run -p tft-vision --release --example boxes -- <png>`

fn main() -> anyhow::Result<()> {
    let p = std::env::args().nth(1).expect("usage: boxes <png>");
    let img = ocr::scoreboard::decode_png(&std::fs::read(&p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
    let frame = ocr::Frame { width: img.w, height: img.h, bgra };
    for b in ocr::recognize_boxes(&frame)? {
        println!("{:>5.0} {:>5.0} {:>4.0}x{:<3.0} {}", b.x, b.y, b.w, b.h, b.text);
    }
    Ok(())
}
