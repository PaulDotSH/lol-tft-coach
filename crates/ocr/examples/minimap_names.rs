//! Names the enemy blips in saved debug minimap frames, for tuning portrait matching.
//! `cargo run -p ocr --example minimap_names -- <debug session dir> <icon dir> Teemo,Jinx,Yasuo,Ezreal,Soraka`
//! The icon dir holds Data Dragon squares named `<Id>.png`.

use ocr::Frame;
use ocr::minimap::{Portrait, enemy_blips, name_blips, portrait};
use ocr::scoreboard::decode_png;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, icons, champs] = &args[..] else {
        eprintln!("usage: minimap_names <debug dir> <icon dir> <Id,Id,...>");
        std::process::exit(2);
    };
    let names: Vec<&str> = champs.split(',').collect();
    let portraits: Vec<(u32, Portrait)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32, portrait(&decode_png(&std::fs::read(format!("{icons}/{n}.png")).unwrap()).unwrap())))
        .collect();
    let refs: Vec<(u32, &Portrait)> = portraits.iter().map(|(k, p)| (*k, p)).collect();
    let mut frames: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with("-minimap.png")).collect();
    frames.sort();
    let mut counts = vec![0; names.len()];
    let (mut blips_total, mut named_total) = (0, 0);
    for f in &frames {
        let img = decode_png(&std::fs::read(f).unwrap()).unwrap();
        let frame = Frame { width: img.w, height: img.h, bgra: img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect() };
        let b = enemy_blips(&frame);
        let n = name_blips(&frame, &b, &refs);
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
    println!("{} frames, {blips_total} blips, {named_total} named", frames.len());
    for (n, c) in names.iter().zip(counts) {
        println!("  {n}: {c}");
    }
}
