//! Review a saved match-history game the way the app does.
//! `cargo run -p lol-coach --release --example review -- <game.json> <timeline.json> <puuid> [out.json]`

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = |i: usize| std::env::args().nth(i).expect("usage: review <game.json> <timeline.json> <puuid>");
    let read = |p: String| -> anyhow::Result<serde_json::Value> { Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?) };
    let (game, timeline, puuid) = (read(arg(1))?, read(arg(2))?, arg(3));
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-smoke-cache"))?;
    let sd = data::ddragon::load(&http).await?;
    let patch = sd.version.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
    let d = clients::history::parse_match(&game, Some(&puuid), Some(&timeline)).expect("game parses");
    let g = &d.game;
    let stats = data::lolalytics::champ_stats(&http, &sd.champs[&g.champ], g.lane, g.mode, "emerald_plus", "all", &patch).await.ok();
    let a = coach::analyze_match(&d, &sd, stats.as_ref(), None);
    for p in &a.players {
        let lane = p.lane.map_or("-", |l| l.as_str());
        println!("{} {:<8} {:<12} {}/{}/{} {:>3} cs {:>6} dmg", if p.ally { "us  " } else { "them" }, lane, sd.champ_name(p.champ), p.kills, p.deaths, p.assists, p.cs, p.damage);
    }
    println!("opponent {:?}, gold 10/15 {:?}/{:?}", a.opponent.map(|c| sd.champ_name(c)), g.gold_diff_10, g.gold_diff_15);
    println!("kp {:?} dmg {:?} gold {:?} taken {:?}", a.kill_participation, a.damage_share, a.gold_share, a.taken_share);
    for p in a.gold.iter().step_by(5) {
        println!("  {:>5.0}s team {:>6} lane {:?}", p.time, p.team, p.lane);
    }
    for m in &a.marks {
        println!("  mark {:>5.0}s {} {} {:?}", m.time, if m.ours { "us  " } else { "them" }, m.kind, m.detail);
    }
    for n in &a.review.deaths {
        println!("  death {:>5.0}s by {:?} gap {:?} held {} traded {} at {:?}", n.time, n.killer.map(|c| sd.champ_name(c)), n.gold_gap, n.unspent, n.traded, n.place);
    }
    for n in &a.review.notes {
        println!("  note {n}");
    }
    println!("  recommended {:?}", a.recommended.iter().map(|i| sd.item_name(*i)).collect::<Vec<_>>());
    for t in &a.takeaways {
        println!("  {} {}", if t.good { "+" } else { "-" }, t.text);
    }
    if let Some(out) = std::env::args().nth(4) {
        std::fs::write(out, serde_json::to_string_pretty(&a)?)?;
    }
    Ok(())
}
