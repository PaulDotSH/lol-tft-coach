//! Read a saved `tab-board` debug capture offline.
//! `cargo run -p ocr --release --example board_png -- <png> <client w> <client h> <icon dir>`
//! The icon dir holds the CommunityDragon small icons by file name plus `kiwi.json`
//! ({id: [icon path, rarity]}). Rows come out in API order (5 Order, 5 Chaos), no item matching.

use model::Team;
use ocr::scoreboard::{AugIcons, BoardPlayer, Geometry, decode_png, read_all};
use std::collections::HashMap;

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h): (u32, u32) = (a[1].parse()?, a[2].parse()?);
    let dir = std::path::Path::new(&a[3]);
    let kiwi: HashMap<String, (String, String)> = serde_json::from_str(&std::fs::read_to_string(dir.join("kiwi.json"))?)?;
    let mut ids: Vec<(u32, String)> = kiwi.iter().map(|(k, v)| (k.parse().unwrap(), v.0.clone())).collect();
    ids.sort();
    let icons = AugIcons::new(ids.iter().filter_map(|(id, p)| {
        let f = dir.join(p.rsplit('/').next()?);
        Some((*id, decode_png(&std::fs::read(f).ok()?)?))
    }));

    let img = decode_png(&std::fs::read(&a[0])?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
    let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    let frame = ocr::Frame { width: img.w, height: img.h, bgra };
    let g = Geometry::new(w, h);
    let r = g.board();
    let players: Vec<BoardPlayer> =
        (0..10).map(|i| BoardPlayer { team: if i < 5 { Team::Order } else { Team::Chaos }, items: &[] }).collect();
    let Some(rows) = read_all(&frame, (r.x, r.y), &g, &players, &icons, &HashMap::new()) else {
        println!("board not open");
        return Ok(());
    };
    for row in rows {
        let cells: Vec<String> = row
            .augments
            .iter()
            .map(|s| match s.group {
                Some(gr) => format!("{:?} {:.2} {:?}", icons.ids(gr), s.score, s.rarity),
                None => format!("UNKNOWN {:.2} {:?}", s.score, s.rarity),
            })
            .collect();
        println!("{}: {}", row.player, cells.join(" | "));
    }
    Ok(())
}
