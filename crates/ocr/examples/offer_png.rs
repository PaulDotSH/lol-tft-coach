//! Run offer OCR on a saved PNG: crop a fraction band, recognise, match names.
//! `cargo run -p ocr --release --example offer_png -- <png> <x0> <x1> <y0> <y1> [manifest.json]`
//! (fractions of the image; the manifest defaults to the u.gg fixture).

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |i: usize| a[i].parse::<f32>().expect("fraction");
    let (x0, x1, y0, y1) = (f(1), f(2), f(3), f(4));
    let manifest = a.get(5).cloned().unwrap_or_else(|| {
        format!("{}/../../fixtures/data/ugg-augment-manifest-16_21.json", env!("CARGO_MANIFEST_DIR"))
    });
    let names: std::collections::HashMap<u32, String> = serde_json::from_str::<std::collections::HashMap<String, String>>(
        &std::fs::read_to_string(manifest)?,
    )?
    .into_iter()
    .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
    .collect();

    let img = ocr::scoreboard::decode_png(&std::fs::read(&a[0])?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let (w, h) = (img.w as f32, img.h as f32);
    let (cx, cy, cw, ch) = ((x0 * w) as u32, (y0 * h) as u32, ((x1 - x0) * w) as u32, ((y1 - y0) * h) as u32);
    let mut bgra = Vec::with_capacity((cw * ch * 4) as usize);
    for y in cy..cy + ch {
        let row = ((y * img.w + cx) * 4) as usize;
        // RGBA → BGRA
        bgra.extend(img.px[row..row + (cw * 4) as usize].as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]));
    }
    let frame = ocr::Frame { width: cw, height: ch, bgra };
    let t = std::time::Instant::now();
    let lines = ocr::recognize(&frame)?;
    let ids = ocr::match_augments(&lines, &names);
    println!("{cw}x{ch} in {:?}", t.elapsed());
    println!("lines: {lines:?}");
    println!("matched: {:?}", ids.iter().map(|i| &names[i]).collect::<Vec<_>>());
    Ok(())
}
