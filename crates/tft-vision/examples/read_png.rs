//! Run the reader on saved captures with the Set 18 fixture's names.
//! `cargo run -p tft-vision --release --example read_png -- <png>...`

fn main() -> anyhow::Result<()> {
    let set = tft_data::parse_set_data(&std::fs::read_to_string("fixtures/tft/cdragon_set18.json")?, tft_data::SET)?;
    let r = tft_vision::Reader::new(&set);
    for p in std::env::args().skip(1) {
        let img = ocr::scoreboard::decode_png(&std::fs::read(&p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let t = std::time::Instant::now();
        let s = r.read_frame(&ocr::Frame { width: img.w, height: img.h, bgra }, 0);
        println!("{} ({} ms): {}", p.rsplit(['/', '\\']).next().unwrap_or(&p), t.elapsed().as_millis(), serde_json::to_string(&s)?);
    }
    Ok(())
}
