//! Item reading against hand labels. `cargo run -p tft-vision --release --example items_eval -- <icons dir> <captures dir>
//! <bench labels> <unit labels>`. Icons are Community Dragon PNGs named as in their URL; labels are lines of
//! `<game dir>/<frame> <slot> <api|-|drag>` (bench) and `<frame> <bar x>,<bar y> <icon> <api|-|?>` (units).
//! With only `<icons dir> <game dir>`: every capture's bench and units' items, one line each.

use std::collections::HashMap;

fn frame(path: &str) -> anyhow::Result<ocr::Frame> {
    let img = ocr::scoreboard::decode_png(&std::fs::read(path)?).ok_or_else(|| anyhow::anyhow!("bad png {path}"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
    Ok(ocr::Frame { width: img.w, height: img.h, bgra })
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let set = tft_data::parse_set_data(&std::fs::read_to_string("fixtures/tft/cdragon_set18.json")?, tft_data::SET)?;
    let icons: Vec<(String, ocr::scoreboard::Rgba)> = set
        .items
        .values()
        .filter_map(|i| {
            let url = tft_data::icon_url(&i.icon);
            let b = std::fs::read(format!("{}/{}", a[0], url.rsplit('/').next()?)).ok()?;
            Some((i.api.clone(), ocr::scoreboard::decode_png(&b)?))
        })
        .collect();
    let t = std::time::Instant::now();
    let icons = std::sync::Arc::new(tft_vision::ItemIcons::new(icons));
    println!("{} templates in {} ms", icons.len(), t.elapsed().as_millis());
    if a.len() == 2 {
        let mut reader = tft_vision::Reader::new(&set);
        reader.set_items(Some(icons.clone()));
        let mut v: Vec<_> = std::fs::read_dir(&a[1])?.filter_map(|e| Some(e.ok()?.path())).collect();
        v.sort();
        for p in v {
            let f = frame(p.to_str().unwrap_or_default())?;
            let b = tft_vision::items::bench(&f, f.width as f32 / 2560.0, &icons);
            let b: Vec<&str> = b.iter().map(|s| s.as_deref().unwrap_or("-")).collect();
            let worn: Vec<String> = reader.read_frame(&f, 0).map_or(vec![], |s| s.units.iter().map(|u| format!("{:.0},{:.0}:{}", u.x, u.y, u.items.join("+"))).collect());
            println!("{} {} | {}", p.file_stem().and_then(|s| s.to_str()).unwrap_or_default(), b.join(" "), worn.join(" "));
        }
        return Ok(());
    }

    let (mut ok, mut wrong, mut missed, mut extra) = (0, 0, 0, 0);
    let mut frames: HashMap<String, ocr::Frame> = HashMap::new();
    let mut ms = vec![];
    for l in std::fs::read_to_string(&a[2])?.lines() {
        let p: Vec<&str> = l.split_whitespace().collect();
        let f = match frames.get(p[0]) {
            Some(f) => f,
            None => {
                let fr = frame(&format!("{}/20261025-{}.png", a[1], p[0]))?;
                let t = std::time::Instant::now();
                tft_vision::items::bench(&fr, fr.width as f32 / 2560.0, &icons);
                ms.push(t.elapsed().as_secs_f32() * 1000.0);
                frames.entry(p[0].to_string()).or_insert(fr)
            }
        };
        let got = tft_vision::items::bench(f, f.width as f32 / 2560.0, &icons)[p[1].parse::<usize>()?].clone();
        let want = (p[2] != "-").then_some(p[2]);
        match (want, got.as_deref()) {
            (Some(w), Some(g)) if w == g => ok += 1,
            (Some(w), Some(g)) => {
                wrong += 1;
                println!("bench {} {}: {g}, labelled {w}", p[0], p[1]);
            }
            (Some(w), None) => {
                missed += 1;
                println!("bench {} {}: nothing, labelled {w}", p[0], p[1]);
            }
            (None, Some(g)) => {
                extra += 1;
                println!("bench {} {}: {g}, labelled empty", p[0], p[1]);
            }
            (None, None) => {}
        }
    }
    let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    println!("bench: {ok} right, {wrong} wrong, {missed} missed, {extra} extra; {:.1} ms a read", avg(&ms));

    let mut reader = tft_vision::Reader::new(&set);
    reader.set_items(Some(icons.clone()));
    let (mut ok, mut wrong, mut missed, mut extra, mut unknown) = (0, 0, 0, 0, 0);
    let mut units: HashMap<String, Vec<tft_model::SeenUnit>> = HashMap::new();
    let mut ms = vec![];
    for l in std::fs::read_to_string(&a[3])?.lines() {
        let p: Vec<&str> = l.split_whitespace().collect();
        let game = if p[0] < "1792890900000" { "024840" } else { "031617" };
        let path = format!("{}/20261025-{game}/{}.png", a[1], p[0]);
        let f = frame(&path)?;
        if !units.contains_key(p[0]) {
            let snap = reader.read_frame(&f, 0).ok_or_else(|| anyhow::anyhow!("no HUD in {path}"))?;
            let t = std::time::Instant::now();
            let mut us = snap.units.clone();
            tft_vision::items::read(&f, f.width as f32 / 2560.0, &icons, &mut vec![], &mut us);
            ms.push(t.elapsed().as_secs_f32() * 1000.0);
            units.insert(p[0].to_string(), snap.units);
        }
        let (x, y) = p[1].split_once(',').unwrap();
        let (x, y): (f32, f32) = (x.parse()?, y.parse()?);
        let Some(u) = units[p[0]].iter().find(|u| u.x.round() == x && u.y.round() == y) else {
            println!("unit {} {}: not found", p[0], p[1]);
            continue;
        };
        let got = tft_vision::items::on_unit(&f, f.width as f32 / 2560.0, &icons, u)[p[2].parse::<usize>()?].clone();
        match (p[3], got.as_deref()) {
            ("-", None) => {}
            ("-", Some(g)) => {
                extra += 1;
                println!("unit {} {} {}: {g}, labelled empty", p[0], p[1], p[2]);
            }
            ("?", g) => {
                unknown += 1;
                println!("unit {} {} {}: {g:?}, labelled unknown", p[0], p[1], p[2]);
            }
            (w, Some(g)) if w == g => ok += 1,
            (w, Some(g)) => {
                wrong += 1;
                println!("unit {} {} {}: {g}, labelled {w}", p[0], p[1], p[2]);
            }
            (w, None) => {
                missed += 1;
                println!("unit {} {} {}: nothing, labelled {w}", p[0], p[1], p[2]);
            }
        }
    }
    println!("units: {ok} right, {wrong} wrong, {missed} missed, {extra} extra, {unknown} unknown; {:.1} ms a read", avg(&ms));
    Ok(())
}
