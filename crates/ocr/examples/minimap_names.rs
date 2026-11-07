//! Names the enemy blips in saved debug minimap frames, for tuning ring detection and portrait matching.
//! `cargo run --release -p ocr --example minimap_names -- <debug session dir> <icon dir> Teemo,Jinx,Yasuo,Ezreal,Soraka`
//! The icon dir holds Data Dragon squares `<Id>.png` and, when present, HUD circles `<Id>_circle.png`.

use ocr::Frame;
use ocr::minimap::{Art, Portrait, enemy_blips, map_square, name_blips, portrait};
use ocr::scoreboard::decode_png;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, icons, champs] = &args[..] else {
        eprintln!("usage: minimap_names <debug dir> <icon dir> <Id,Id,...>");
        std::process::exit(2);
    };
    let names: Vec<&str> = champs.split(',').collect();
    let read = |p: String| std::fs::read(p).ok().and_then(|b| decode_png(&b));
    let portraits: Vec<(u32, Portrait)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let square = read(format!("{icons}/{n}.png")).expect("square icon");
            let circle = read(format!("{icons}/{n}_circle.png"));
            let mut art = vec![Art::Square(&square)];
            if let Some(c) = &circle {
                art.push(Art::Circle(c));
            }
            (i as u32, portrait(&art))
        })
        .collect();
    let refs: Vec<(u32, &Portrait)> = portraits.iter().map(|(k, p)| (*k, p)).collect();
    let mut frames: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with("-minimap.png")).collect();
    frames.sort();
    let mut counts = vec![0; names.len()];
    let (mut blips_total, mut named_total) = (0, 0);
    let mut spent = std::time::Duration::ZERO;
    for f in &frames {
        let img = decode_png(&std::fs::read(f).unwrap()).unwrap();
        let frame = Frame { width: img.w, height: img.h, bgra: img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect() };
        let t = Instant::now();
        let sq = map_square(&frame, false);
        let b = enemy_blips(&frame, sq);
        let n = name_blips(&frame, sq, &b, &refs);
        spent += t.elapsed();
        blips_total += b.len();
        let seen: Vec<String> = b
            .iter()
            .zip(&n)
            .map(|(&(x, y), k)| {
                if let Some(k) = k {
                    counts[*k as usize] += 1;
                    named_total += 1;
                }
                format!("({x:.2},{y:.2}) {}", k.map_or("?", |k| names[k as usize]))
            })
            .collect();
        println!("{} {}", f.file_name().unwrap().to_string_lossy(), seen.join(" | "));
    }
    println!("{} frames, {blips_total} blips, {named_total} named, {:.1} ms/frame", frames.len(), spent.as_secs_f64() * 1000.0 / frames.len().max(1) as f64);
    for (n, c) in names.iter().zip(counts) {
        println!("  {n}: {c}");
    }
}
