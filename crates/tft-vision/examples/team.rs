//! Read the team size ("N/M" on my board) of saved captures: `team -- <dir or png>...`.
//! Prints "index file N/M" (or "-") per capture (N 0 for a free slot whose N is hidden: no HP bars here); `TEAM_DEBUG=1` adds the glyphs found per style.

fn main() -> anyhow::Result<()> {
    let mut files: Vec<String> = vec![];
    for a in std::env::args().skip(1) {
        match std::fs::read_dir(&a) {
            Ok(d) => {
                let mut v: Vec<String> = d.filter_map(|e| Some(e.ok()?.path().to_str()?.to_string())).filter(|p| p.ends_with(".png")).collect();
                v.sort();
                files.extend(v);
            }
            Err(_) => files.push(a),
        }
    }
    for (i, p) in files.iter().enumerate() {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let f = ocr::Frame { width: img.w, height: img.h, bgra };
        let scale = f.width as f32 / 2560.0;
        let t = std::time::Instant::now();
        let r = tft_vision::team::read(&f, scale, 0);
        let ms = t.elapsed().as_millis();
        let file = p.rsplit(['/', '\\']).next().unwrap_or(p);
        println!("{i} {file} {} {ms}ms", r.map_or("-".into(), |(n, m)| format!("{n}/{m}")));
        if std::env::var("TEAM_DEBUG").is_ok() {
            for (k, g) in tft_vision::team::found(&f, scale).iter().enumerate() {
                println!("  {k}: {}", g.iter().map(|(c, s, x)| format!("{c}@{x}:{s:.2}")).collect::<Vec<_>>().join(" "));
            }
        }
    }
    Ok(())
}
