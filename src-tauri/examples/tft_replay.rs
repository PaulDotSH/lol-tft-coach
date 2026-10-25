//! Replay saved TFT captures through the screen reader and the coach.
//! `cargo run -p lol-coach --release --example tft_replay -- <queue> <png>...`

use tft_model::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let queue: i64 = a[0].parse()?;
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-replay-cache"))?;
    let set = tft_data::set_data(&http).await?;
    let meta = tft_data::meta(&http, queue, &set).await?;
    let hooks: Box<dyn tft_coach::ModeHooks> =
        if TftMode::of_queue(queue) == TftMode::Choncc { Box::new(tft_choncc::ChonccHooks) } else { Box::new(tft_coach::StandardHooks) };
    let mut reader = tft_vision::Reader::new(&set);
    let mut tracker = tft_coach::Tracker::default();
    let mut keep: Option<String> = None;
    let name = |api: &str| tft_coach::unit_name(&set, api).to_string();
    for p in &a[1..] {
        let img = ocr::scoreboard::decode_png(&std::fs::read(p)?).ok_or_else(|| anyhow::anyhow!("bad png"))?;
        let bgra = img.px.as_chunks::<4>().0.iter().flat_map(|c| [c[2], c[1], c[0], c[3]]).collect();
        let Some(snap) = reader.read_frame(&ocr::Frame { width: img.w, height: img.h, bgra }, 0) else { continue };
        tracker.observe(&snap, &set);
        let adv = tft_coach::advise(&tft_coach::CoachInput { snap: &snap, tracker: &tracker, set: &set, meta: &meta, hooks: hooks.as_ref(), keep: keep.as_deref() });
        keep = adv.comps.first().map(|c| c.comp.clone());
        let stage = snap.stage.map_or("?".into(), |(a, b)| format!("{a}-{b}"));
        let owned: Vec<String> = adv.owned.iter().map(|o| if o.star == 0 { format!("?{}", o.api) } else { format!("{}★{}", name(&o.api), o.star) }).collect();
        println!("== {stage} gold {:?} lvl {:?} hp {:?} | owned {}", snap.gold, snap.level, snap.hp, owned.join(", "));
        if std::env::var("FIT").is_ok() {
            println!("   panel {:?}", tracker.panel());
            let input = tft_coach::CoachInput { snap: &snap, tracker: &tracker, set: &set, meta: &meta, hooks: hooks.as_ref(), keep: None };
            for (c, s) in tft_coach::fit::rank(&input).iter().take(4) {
                println!("   {s:.3} {} {:?}", c.name, c.traits);
            }
        }
        for c in adv.comps.iter().take(2) {
            println!("   comp {} fit {:.2} avg {:.2} missing {}", c.name, c.fit, c.avg_place, c.missing.iter().map(|u| name(u)).collect::<Vec<_>>().join(", "));
        }
        let shop: Vec<String> = adv.shop.iter().map(|s| format!("{}{}", if s.buy { "BUY " } else { "" }, name(&s.unit))).collect();
        println!("   shop {}", shop.join(" | "));
        if let Some(e) = &adv.econ {
            println!("   econ {:?}: {}", e.action, e.text);
        }
        for i in &adv.items {
            println!("   item {} on {:?}{}", tft_coach::item_name(&set, &i.item), i.holder.as_deref().map(name), if i.now { " NOW" } else { "" });
        }
        for c in &adv.calls {
            println!("   CALL [{}] {}", c.key, c.text);
        }
        if let Some(ch) = &adv.choncc {
            println!("   choncc starter {:?} loot {:?}", ch.starter.as_deref().map(name), ch.loot);
        }
    }
    Ok(())
}
