//! Print the TFT review and trends for a saved LCU TFT history.
//! `cargo run -p lol-coach --release --example tft_review -- <history.json> [puuid]`

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let p = std::env::args().nth(1).expect("usage: tft_review <history.json>");
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-replay-cache"))?;
    let set = tft_data::set_data(&http).await?;
    let me = std::env::args().nth(2);
    let mut games = tft_data::parse_history(&serde_json::from_str(&std::fs::read_to_string(p)?)?, me.as_deref());
    let meta = tft_data::meta(&http, games.first().map_or(1100, |g| g.queue), &set).await?;
    for g in &mut games {
        g.comp = g.comp.take().or_else(|| tft_coach::match_comp(g, &meta));
    }
    let r = tft_coach::review(&games[0], &meta, &set);
    println!("latest: place {} level {} comp {:?}", r.game.placement, r.game.level, r.comp.as_ref().map(|c| (&c.name, c.avg_place)));
    for t in &r.takeaways {
        println!("  - {t}");
    }
    for t in tft_coach::trends(&games, &meta) {
        println!("{:?}: {} games avg {:.2} top4 {:.0}%", t.mode, t.games, t.avg_place, t.top4 * 100.0);
        for c in t.comps.iter().take(4) {
            println!("   {} x{} avg {:.2}", c.name, c.games, c.avg_place);
        }
    }
    Ok(())
}
