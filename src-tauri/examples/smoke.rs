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
    let a = coach::game_advice(&coach::GameInput { snap: &snap, sd: &sd, stats: Some(&stats), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: None, situations: &Default::default() });
    println!("coach tick: {:?}", t.elapsed());
    println!("enemy team (magic, range, tank, cc): {:?}", coach::situations::enemy_measures(&snap, &sd));
    print_game(&sd, &a);

    // Coachless situations for Buy next and its alternatives (the app's saved sign-in), then the
    // tick again: with the shipped table, and with the hand-set test table for a standing.
    let auth = data::coachless::Auth::load(dirs::data_local_dir().unwrap_or_default().join("lol-coach").join("coachless.json"));
    match (me.position.or(stats.lane), auth.signed_in()) {
        (Some(lane), true) => {
            let built: Vec<ItemId> = me.items.iter().copied().filter(|i| sd.items.get(i).is_some_and(|i| i.completed && !i.boots)).collect();
            let q = data::coachless::Query { champ: me.champ, lane, vs: None, tier: "emerald_plus", region: "all", first: built.first().copied(), second: built.get(1).copied() };
            let mut sits = std::collections::HashMap::new();
            for (item, slot) in coach::situation_items(&a, &snap, &sd).into_iter().take(3) {
                match data::coachless::item_situations(&http, &auth, q, item, slot).await {
                    Ok(s) => {
                        let games: f64 = s.by.iter().map(|b| b.2).sum::<f64>() / 5.0;
                        let parts: Vec<String> = s.by.iter().map(|(w, d, g)| format!("{w:?} {:+.2} ({:.0})", d - s.overall, g)).collect();
                        println!("situations {} {slot:?}: overall {:+.2} over {games:.0} games | {}", sd.item_name(item), s.overall, parts.join(", "));
                        sits.insert(item, s);
                    }
                    Err(e) => println!("situations {}: {e:#}", sd.item_name(item)),
                }
            }
            let handset = coach::winprob::Model::parse(include_str!("../../fixtures/coach/winprob_handset.toml"))?;
            // Made-up behind state: every enemy holds their items twice.
            let mut behind = snap.clone();
            for p in behind.players.iter_mut().filter(|p| p.team != me.team) {
                p.items = p.items.repeat(2);
            }
            for (label, s, winprob) in [("shipped table", &snap, None), ("HAND-SET table", &snap, Some(&handset)), ("HAND-SET table, enemy items doubled", &behind, Some(&handset))] {
                let a = coach::game_advice(&coach::GameInput { snap: s, sd: &sd, stats: Some(&stats), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob, situations: &sits });
                println!("with Coachless situations, {label} (standing {}):", a.win.as_ref().map_or("none".into(), |w| format!("{:.0}%", w.base * 100.0)));
                for p in a.buy_next.iter().chain(&a.alternatives) {
                    println!("  {} {:.4} — {}", p.name, p.score, p.reason);
                }
                for (i, p) in a.path.iter().enumerate().skip(1).filter(|(_, p)| sits.contains_key(&p.item)) {
                    println!("  path {} {} {:.4} — {}", i + 1, p.name, p.score, p.reason);
                }
            }
        }
        _ => println!("situations: not signed in to Coachless (or no lane)"),
    }

    // Game plan for the fixture's ten champions, lanes from their positions.
    let t = Instant::now();
    let mut length = std::collections::HashMap::new();
    for p in &snap.players {
        match data::lolalytics::length_stats(&http, &sd.champs[&p.champ], p.position, snap.mode, "emerald_plus", "all", &patch).await {
            Ok(c) => {
                length.insert(p.champ, c);
            }
            Err(e) => println!("length {}: {e:#}", sd.champ_name(p.champ)),
        }
    }
    let side = |ally: bool| snap.players.iter().filter(|p| (p.team == me.team) == ally).map(|p| (p.champ, p.position)).collect::<Vec<_>>();
    let (allies, foes) = (side(true), side(false));
    let keys = |v: &[(ChampKey, Option<Lane>)]| v.iter().map(|x| x.0).collect::<Vec<_>>();
    let draft = coach::roster_draft(&sd, &keys(&allies), &keys(&foes));
    let notes = data::notes::lane_notes()?;
    let plan = coach::game_plan(&coach::GamePlanInput { allies: &allies, enemies: &foes, length: &length, draft: Some(&draft), matchup: None, memory: &[], notes: &notes, sd: &sd, mode: snap.mode });
    println!("
== Game plan ({:?}) ==", t.elapsed());
    for (c, (lane, b)) in &length {
        let rows: Vec<String> = b.iter().map(|w| format!("{}:{}/{}", w.from_min, w.games, w.wins)).collect();
        println!("curve {:<12} {:?} {}", sd.champ_name(*c), lane, rows.join(" "));
    }
    match &plan {
        Some(p) => {
            for ph in &p.phases {
                println!("phase {:<5} {:>2}-{:<4} edge {:+.2}{}", ph.name, ph.from_min, ph.to_min.map_or("".into(), |m| m.to_string()), ph.edge, if ph.even { " (even)" } else { "" });
            }
            for s in &p.scalers {
                println!("scaler {:<12} ally {} early {:+.2} late {:+.2}{}", sd.champ_name(s.champ), s.ally, s.early, s.late, if s.off_lane { " (main-lane data)" } else { "" });
            }
            for l in &p.lines {
                println!("{:<8} {}", l.kind, l.text);
            }
            println!("fold: {} | {} | {} of {} champions", p.fold, p.source, p.curves, p.champs);
        }
        None => println!("no plan (no curves)"),
    }

    // Mayhem: same snapshot treated as Mayhem with a fake 3-card offer.
    let m = data::ugg::mayhem(&http, &sd.version).await?;
    let aug = data::ugg::champ_augments(&http, &m.patch, me.champ).await?;
    let offered = [aug.gold[5], aug.gold[0], aug.gold[12]];
    let mut ms = snap.clone();
    ms.mode = Mode::Mayhem;
    let aram = data::lolalytics::champ_stats(&http, &sd.champs[&me.champ], None, Mode::Mayhem, "all", "all", &patch).await?;
    // Pretend the first offer's top card (if curated) was taken.
    let took = [MyAugment { id: Some(aug.gold[0]), candidates: vec![aug.gold[0]], time: 300.0 }];
    let a = coach::game_advice(&coach::GameInput { snap: &ms, sd: &sd, stats: Some(&aram), augments: Some((&aug, &m)), offered: &offered, gold_history: &[], my_augments: &took, enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: None, situations: &Default::default() });
    println!("\n== Mayhem (u.gg {}) ==", m.patch);
    print_game(&sd, &a);

    let mut ars = snap.clone();
    ars.mode = Mode::Aram;
    let aram_stats = data::lolalytics::champ_stats(&http, &sd.champs[&me.champ], None, Mode::Aram, "all", "all", &patch).await?;
    let a = coach::game_advice(&coach::GameInput { snap: &ars, sd: &sd, stats: Some(&aram_stats), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: None, situations: &Default::default() });
    println!("\n== ARAM ==");
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
    // ARAM / Mayhem bench: tier or ARAM win rate + team fit, my team only.
    let aram_tiers = data::lolalytics_aram::aram_tier_list(&http, "emerald_plus", "all", &patch).await?;
    let mut acs = clients::lcu::parse_champ_select(&fixture("champ_select_aram.json")?, 450).expect("aram fixture parses");
    for mode in [Mode::Aram, Mode::Mayhem] {
        acs.mode = mode;
        let a = coach::champ_select_advice(&coach::ChampSelectInput {
            cs: &acs,
            sd: &sd,
            mine: None,
            enemies: &[],
            tier_list: &aram_tiers,
            mayhem: Some(&m),
            draft: Default::default(),
        });
        println!("
== {mode:?} bench, allies {:?} ==", acs.allies.iter().map(|c| sd.champ_name(*c)).collect::<Vec<_>>());
        for b in &a.bench {
            println!("bench {:<12} {:>5} {:?}  {}", sd.champ_name(b.champ), b.score.map_or("-".into(), |s| format!("{s:+.1}")), b.tier, b.reasons.join(" · "));
        }
        if let Some(t) = &a.team {
            println!("team: AD {:.0}% AP {:.0}% front {} engage {} peel {:.1} cc {:.1} gaps {:?}", t.physical * 100.0, t.magic * 100.0, t.frontline, t.engage, t.peel, t.cc, t.gaps);
        }
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
    match &a.win {
        Some(w) => println!(
            "win {:.0}%{} (standing {:.0}%) {} | {} | {}",
            w.p * 100.0,
            if w.rough { " ROUGH" } else { "" },
            w.base * 100.0,
            w.factors.iter().map(|f| format!("{} {:+.0}", f.label, f.points)).collect::<Vec<_>>().join(", "),
            w.stakes.iter().map(|s| format!("{} ±{:.0}", s.name, s.swing)).collect::<Vec<_>>().join(", "),
            w.source
        ),
        None => println!("win: no fit for this mode (winprob.toml has {} games)", coach::winprob::Model::shipped().games),
    }
    for w in &a.what_if {
        println!(
            "what-if {:<24} dmg {:+5.1}% (burst {:+5.1}% / 10 s {:+5.1}%, {} leads) EHP {:+5.1}% (phys {:+5.1}% magic {:+5.1}%){}{}",
            sd.item_name(w.item), w.damage * 100.0, w.burst * 100.0, w.sustained * 100.0, if w.burst_leads { "burst" } else { "10 s" },
            w.ehp * 100.0, w.ehp_physical * 100.0, w.ehp_magic * 100.0, if w.rough { " ROUGH" } else { "" }, w.note.as_deref().map_or(String::new(), |n| format!(" ({n})"))
        );
        for e in &w.per_enemy {
            println!("    vs {:<12} burst {:+5.1}%  10 s {:+5.1}%{}", sd.champ_name(e.champ), e.burst * 100.0, e.sustained * 100.0, if e.lane { "  LANE" } else { "" });
        }
    }
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
