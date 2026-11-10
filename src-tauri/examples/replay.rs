//! Re-run in-game advice on a saved `live-allgamedata.json` debug dump.
//! `cargo run -p lol-coach --release --example replay -- <dump.json>`

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("dump path");
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-smoke-cache"))?;
    let sd = data::ddragon::load(&http).await?;
    let patch = sd.version.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
    let snap = clients::live::parse(&serde_json::from_str(&std::fs::read_to_string(path)?)?, &sd).expect("dump parses");
    let me = &snap.players[snap.me];
    let stats =
        data::lolalytics::champ_stats(&http, &sd.champs[&me.champ], me.position, snap.mode, "emerald_plus", "all", &patch).await?;
    let a = coach::game_advice(&coach::GameInput {
        snap: &snap,
        sd: &sd,
        stats: Some(&stats),
        augments: None,
        offered: &[],
        gold_history: &[],
        my_augments: &[],
        enemy_augments: Default::default(), keep: None, matchup: None,
    });
    println!("{} at {:.0}s", sd.champ_name(me.champ), snap.time);
    if let Some(d) = &a.defense {
        println!("  enemy magic {:.0}%, my EHP physical {:.0} magic {:.0}", 100.0 * d.enemy_magic, d.ehp_physical.unwrap_or(0.0), d.ehp_magic.unwrap_or(0.0));
    }
    for p in a.buy_next.iter().chain(&a.alternatives) {
        println!("  option {:<22} {}", p.name, p.reason);
    }
    if let Some(u) = &a.support_upgrade {
        println!("  quest  {:<22} {}", u.name, u.reason);
    }
    for p in &a.path {
        println!("  path   {:<22} {:.3} {}", p.name, p.score, p.reason);
    }
    for t in &a.threats {
        println!("  threat {:<12} {:>3.0} {}", t.name, t.score, t.damage);
    }
    Ok(())
}
