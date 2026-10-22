//! End-to-end smoke run without the UI or a running game: real Data Dragon,
//! lolalytics and u.gg data + the clients fixtures → coach advice, printed.
//! `cargo run -p lol-coach --example smoke`

use model::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-smoke-cache"))?;
    let t = Instant::now();
    let sd = data::ddragon::load(&http).await?;
    let patch = sd.version.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
    println!("static: v{} {} items {} champs ({:?})", sd.version, sd.items.len(), sd.champs.len(), t.elapsed());

    let fixture = |name: &str| -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::from_str(&std::fs::read_to_string(format!("{}/../fixtures/clients/{name}", env!("CARGO_MANIFEST_DIR")))?)?)
    };

    // In game (Rift fixture).
    let snap = clients::live::parse(&fixture("live_allgamedata.json")?, &sd).expect("live fixture parses");
    let me = &snap.players[snap.me];
    let t = Instant::now();
    let stats =
        data::lolalytics::champ_stats(&http, &sd.champs[&me.champ], me.position, snap.mode, "emerald_plus", "all", &patch).await?;
    println!("\nlolalytics {} {:?}: patch {} games {} ({:?})", sd.champ_name(me.champ), me.position, stats.patch, stats.games, t.elapsed());
    let t = Instant::now();
    let a = coach::game_advice(&coach::GameInput { snap: &snap, sd: &sd, stats: Some(&stats), augments: None, offered: &[], gold_history: &[], my_augments: &[] });
    println!("coach tick: {:?}", t.elapsed());
    print_game(&sd, &a);

    // Mayhem: same snapshot treated as Mayhem with a fake 3-card offer.
    let m = data::ugg::mayhem(&http, &sd.version).await?;
    let aug = data::ugg::champ_augments(&http, &m.patch, me.champ).await?;
    let offered = [aug.gold[5], aug.gold[0], aug.gold[12]];
    let mut ms = snap.clone();
    ms.mode = Mode::Mayhem;
    let aram = data::lolalytics::champ_stats(&http, &sd.champs[&me.champ], None, Mode::Mayhem, "all", "all", &patch).await?;
    // Pretend the first offer's top card (if curated) was taken.
    let took = [MyAugment { id: Some(aug.gold[0]), candidates: vec![aug.gold[0]], time: 300.0 }];
    let a = coach::game_advice(&coach::GameInput { snap: &ms, sd: &sd, stats: Some(&aram), augments: Some((&aug, &m)), offered: &offered, gold_history: &[], my_augments: &took });
    println!("\n== Mayhem (u.gg {}) ==", m.patch);
    print_game(&sd, &a);

    // Champ select (draft fixture) with real enemy stats + tier list.
    let cs = clients::lcu::parse_champ_select(&fixture("champ_select_draft.json")?, 420).expect("cs fixture parses");
    let lane = cs.my_lane.unwrap_or(Lane::Middle);
    let mine = match cs.my_pick.or(cs.my_hover) {
        Some(c) => Some(data::lolalytics::champ_stats(&http, &sd.champs[&c], Some(lane), Mode::Rift, "emerald_plus", "all", &patch).await?),
        None => None,
    };
    let mut enemies = vec![];
    for e in &cs.enemies {
        enemies.push(data::lolalytics::champ_stats(&http, &sd.champs[e], Some(lane), Mode::Rift, "emerald_plus", "all", &patch).await?);
    }
    let tiers = data::lolalytics::tier_list(&http, lane, "emerald_plus", "all", &patch).await?;
    // Draft: lane matchups + synergy for every revealed champion but me.
    let (al, en) = coach::draft_lineups(&cs, &mine.iter().chain(&enemies).collect::<Vec<_>>());
    let (mut lane_stats, mut synergy) = (vec![], vec![]);
    for &(c, l) in al.iter().chain(&en) {
        let (Some(l), false) = (l, Some(c) == cs.my_pick.or(cs.my_hover)) else { continue };
        lane_stats.push(data::lolalytics_team::lane_stats(&http, &sd.champs[&c], Some(l), "emerald_plus", "all", &patch).await?);
        synergy.push(data::lolalytics_team::team(&http, &sd.champs[&c], l, "emerald_plus", "all", &patch).await?);
    }
    let adv = coach::champ_select_advice(&coach::ChampSelectInput {
        cs: &cs,
        sd: &sd,
        mine: mine.as_ref(),
        enemies: &enemies,
        tier_list: &tiers,
        mayhem: None,
        draft: coach::DraftData { lane_stats: &lane_stats, synergy: &synergy },
    });
    println!("\n== Champ select {lane:?} vs {:?} ==", cs.enemies.iter().map(|e| sd.champ_name(*e)).collect::<Vec<_>>());
    for p in &adv.picks {
        println!("pick {:<12} {:5.1}  {}", sd.champ_name(p.champ), p.score, p.reason);
    }
    for p in &adv.bans {
        println!("ban  {:<12} {:5.1}  {}", sd.champ_name(p.champ), p.score, p.reason);
    }
    if let Some(b) = &adv.build {
        println!("start {:?}", b.start.iter().map(|i| sd.item_name(*i)).collect::<Vec<_>>());
        println!("core  {:?}", b.core.iter().map(|p| &p.name).collect::<Vec<_>>());
    }
    if let Some(d) = &adv.draft {
        for l in &d.lanes {
            let name = |c: Option<ChampKey>| c.map_or("-", |c| sd.champ_name(c));
            println!("lane {:<8} {:<10} vs {:<10} {:?} ({})", l.lane.as_str(), name(l.ally), name(l.enemy), l.ally_wr, l.games);
        }
        for (side, s) in [("allies", &d.allies), ("enemies", &d.enemies)] {
            println!(
                "{side}: {:+.1} pts  AD {:.0}% AP {:.0}% true {:.0}%  front {} engage {} cc {:.1}  gaps {:?}",
                s.total, s.physical * 100.0, s.magic * 100.0, s.true_dmg * 100.0, s.frontline, s.engage, s.cc, s.gaps
            );
            for f in &s.factors {
                println!("  {:<12} {:+5.1}  {}", f.label, f.points, f.detail);
            }
        }
    }
    Ok(())
}

fn print_game(sd: &StaticData, a: &GameAdvice) {
    println!("me: {} at {:.0}s, owned {:?}", sd.champ_name(a.my_champ), a.time, a.owned.iter().map(|i| sd.item_name(*i)).collect::<Vec<_>>());
    if let Some(p) = &a.buy_next {
        println!("BUY NEXT {} ({}g) — {} [now: {:?}]", p.name, p.cost, p.reason, p.component_now.map(|i| sd.item_name(i)));
    }
    for p in &a.alternatives {
        println!("  alt {} — {}", p.name, p.reason);
    }
    println!("path {:?}", a.path.iter().map(|p| &p.name).collect::<Vec<_>>());
    for t in &a.threats {
        println!(
            "threat {:<10} {:5.1} {}/{}/{} gold{:+} lvl{:+} killed-me {} {}{} {:?}",
            t.name, t.score, t.kills, t.deaths, t.assists, t.item_gold_diff, t.level_diff, t.kills_on_me, t.damage,
            if t.lane_opp { " LANE" } else { "" }, t.flags
        );
    }
    for m in &a.matchups {
        println!("matchup {:<10} {:.1}% ({}) {}", sd.champ_name(m.champ), m.vs_wr, m.games, m.label);
    }
    println!("self {:?}", a.me);
    println!("timers {:?}", a.timers);
    if let Some(d) = &a.defense {
        println!("defense: EHP phys {:?} magic {:?}, enemy dmg {:.0}% magic {:.0}% true, notes {:?}", d.ehp_physical, d.ehp_magic, d.enemy_magic * 100.0, d.enemy_true * 100.0, d.pen_notes);
        for e in &d.enemies {
            println!("  {:<10} lvl {:>2} armor {:5.1} mr {:5.1}", sd.champ_name(e.champ), e.level, e.armor, e.mr);
        }
    }
    for g in &a.gold_sinks {
        println!("gold sink {} {:?} sell {:?} {}g — {}", g.kind, g.item.map(|i| sd.item_name(i)), g.sell.map(|i| sd.item_name(i)), g.cost, g.reason);
    }
    if !a.my_augments.is_empty() {
        println!("my augments {:?}", a.my_augments);
    }
    if let Some(au) = &a.augments {
        for r in &au.offered {
            println!("augment {:<28} {:?} rank {:?}/{} {}", r.name, r.rarity, r.rank, r.of, r.synergy.as_deref().unwrap_or(""));
        }
    }
}
