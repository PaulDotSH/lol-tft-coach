//! Stage and gold glyph templates from labelled captures, for `src/glyphs.txt`.
//! `glyphs -- <list>`: each line "<png> s 2-1" or "<png> g 54". Prints each char's majority mask.

use std::collections::BTreeMap;
use tft_vision::glyphs::{Kind, glyphs};

fn main() -> anyhow::Result<()> {
    let list = std::fs::read_to_string(std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("list"))?)?;
    let mut sums: BTreeMap<(char, char), (Vec<u32>, u32)> = BTreeMap::new();
    for l in list.lines() {
        let p: Vec<&str> = l.split_whitespace().collect();
        let [png, kind, label] = p[..] else { continue };
        let img = ocr::scoreboard::decode_png(&std::fs::read(png)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let f = ocr::Frame { width: img.w, height: img.h, bgra };
        let (k, tag) = if kind == "s" { (Kind::Stage, 's') } else { (Kind::Gold, 'g') };
        let gs = glyphs(&f, f.width as f32 / 2560.0, k);
        let chars: Vec<char> = label.chars().filter(char::is_ascii_digit).collect();
        let picked: Vec<_> = match k {
            // The digits either side of the dash.
            Kind::Stage => match (1..gs.len().saturating_sub(1)).find(|&i| gs[i].h <= 6 && gs[i].w >= 5 && gs[i - 1].h >= 14 && gs[i + 1].h >= 14) {
                Some(i) => vec![&gs[i - 1], &gs[i + 1]],
                None => vec![],
            },
            Kind::Gold => gs.iter().filter(|g| g.h >= 14).collect(),
        };
        if picked.len() != chars.len() {
            eprintln!("skip {png} {label}: {} glyphs", picked.len());
            continue;
        }
        for (g, c) in picked.iter().zip(&chars) {
            let e = sums.entry((tag, *c)).or_insert((vec![0; g.mask.bits.len()], 0));
            for (s, b) in e.0.iter_mut().zip(&g.mask.bits) {
                *s += *b as u32;
            }
            e.1 += 1;
        }
    }
    for ((tag, c), (s, n)) in &sums {
        eprintln!("{tag} {c}: {n} samples");
        println!("{tag} {c} {}", s.iter().map(|&v| if v * 2 >= *n { '1' } else { '0' }).collect::<String>());
    }
    Ok(())
}
