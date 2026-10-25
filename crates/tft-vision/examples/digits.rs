//! Cut trait-count digit masks from a saved capture: `<png> <row top>:<digit>...`.
//! Appends "digit w h bits" lines to stdout for `src/digits.txt`.

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let img = ocr::scoreboard::decode_png(&std::fs::read(&a[0])?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
    let f = ocr::Frame { width: img.w, height: img.h, bgra };
    for spec in &a[1..] {
        let (y, d) = spec.split_once(':').unwrap();
        let m = tft_vision::count_mask(&f, 1.0, y.parse()?);
        println!("{d} {} {} {}", m.w, m.h, m.bits.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>());
    }
    Ok(())
}
