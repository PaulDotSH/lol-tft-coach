//! OCR one rectangle of a saved capture: `ocr_crop -- <png> x y w h scale [threshold] [tiles]`.

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a.get(i).and_then(|v| v.parse::<u32>().ok());
    let img = ocr::scoreboard::decode_png(&std::fs::read(&a[0])?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
    let f = ocr::Frame { width: img.w, height: img.h, bgra };
    let mut c = tft_vision::crop(&f, n(1).unwrap(), n(2).unwrap(), n(3).unwrap(), n(4).unwrap(), n(5).unwrap());
    if let Some(t) = n(6) {
        for p in c.bgra.chunks_mut(4) {
            let v = if p[0].max(p[1]).max(p[2]) >= t as u8 { 0 } else { 255 };
            (p[0], p[1], p[2]) = (v, v, v);
        }
    }
    if let Some(k) = n(7) {
        let row = (c.width * 4) as usize;
        let bgra = c.bgra.chunks(row).flat_map(|r| std::iter::repeat_n(r, k as usize).flatten().copied()).collect();
        c = ocr::Frame { width: c.width * k, height: c.height, bgra };
    }
    for b in ocr::recognize_boxes(&c)? {
        println!("{:>5.0} {:>5.0} {:>4.0}x{:<3.0} {:?}", b.x, b.y, b.w, b.h, b.text);
    }
    Ok(())
}
