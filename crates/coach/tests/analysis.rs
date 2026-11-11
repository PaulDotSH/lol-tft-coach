//! Match-history review: deaths with places and trades, gold swings, takeaways,
//! and what a live recap adds.

mod common;

use coach::analyze_match;
use coach::winprob::Model;
use common::*;
use model::*;

const ME: usize = 0;
const OPP: usize = 7;

fn past_game() -> PastGame {
    PastGame {
        game_id: 1,
        created: 0,
        queue_id: 420,
        mode: Mode::Rift,
        champ: AHRI,
        lane: Some(Lane::Middle),
        win: false,
        remake: false,
        duration_s: 1500,
        kills: 2,
        deaths: 3,
        assists: 3,
        cs: 200,
        gold: 9000,
        damage_to_champs: 15000,
        vision: 20,
        death_times: None,
        gold_diff_10: Some(-400),
        gold_diff_15: Some(-900),
        xp_diff_15: None,
        ours: None,
        theirs: None,
        first_dragon: None,
        first_tower: None,
        first_blood: None,
        cs_10: None,
        augments: vec![],
        death_spots: vec![],
        enemies: vec![],
        opponent: None,
        killed_by: vec![],
        control_wards: None,
    }
}

fn line(champ: ChampKey, ally: bool, lane: Lane, kills: u32) -> MatchPlayer {
    MatchPlayer {
        champ,
        name: String::new(),
        ally,
        me: false,
        lane: Some(lane),
        level: 14,
        kills,
        deaths: 3,
        assists: 3,
        cs: 150,
        gold: 9000,
        damage: 15000,
        taken: 20000,
        vision: 20,
        items: vec![],
        wards_placed: 10,
        wards_killed: 2,
        control_wards: 1,
    }
}

/// Blue side, me Ahri mid against Zed. My team leads by 3k at 10:00, then loses
/// 4k by 12:00 (a lost fight and Baron) and the game.
fn detail() -> MatchDetail {
    use Lane::*;
    let mut players = vec![
        line(AHRI, true, Middle, 2),
        line(GAREN, true, Top, 2),
        line(LEE_SIN, true, Jungle, 2),
        line(JINX, true, Bottom, 2),
        line(LEONA, true, Support, 2),
        line(DARIUS, false, Top, 3),
        line(JARVAN, false, Jungle, 3),
        line(ZED, false, Middle, 3),
        line(CAITLYN, false, Bottom, 3),
        line(THRESH, false, Support, 3),
    ];
    players[ME].me = true;
    let lead = |m: u32| match m {
        0..=5 => 0,
        6..=10 => (m - 5) as i32 * 600,
        11 => 1000,
        _ => -1000,
    };
    let frames = (0..=25)
        .map(|m| Frame {
            time: m as f32 * 60.0,
            players: (0..10)
                .map(|i| FrameStat {
                    total_gold: 500 + m * 400 + if i < 5 { (lead(m) / 5 + 200) as u32 } else { 200 },
                    current_gold: if i == ME { 1200 } else { 0 },
                    ..Default::default()
                })
                .collect(),
        })
        .collect();
    let kill = |time: f32, killer: usize, victim: usize, pos: (i32, i32)| KillEvent { time, killer: Some(killer), victim, assisters: vec![], pos: Some(pos) };
    let kills = vec![
        // Twice caught in their jungle, then a death in a fight my team won.
        kill(300.0, OPP, ME, (10000, 6500)),
        kill(420.0, OPP, ME, (10500, 7000)),
        kill(650.0, OPP, ME, (5000, 9800)),
        kill(652.0, 1, 7, (5200, 9700)),
        kill(655.0, 2, 6, (5100, 9600)),
        // The swing: three more of my team die.
        kill(700.0, 5, 1, (9000, 9000)),
        kill(710.0, 5, 2, (9000, 9000)),
        kill(715.0, 6, 3, (9000, 9000)),
    ];
    let marks = vec![TimelineMark {
        time: 690.0,
        ours: false,
        kind: "Baron".into(),
        detail: None,
        pos: Some((5000, 10400)),
        mine: false,
        presence: None,
    }];
    MatchDetail { game: past_game(), me: ME, players, frames, kills, marks }
}

#[test]
fn deaths_get_places_trades_and_a_place_note() {
    let a = analyze_match(&detail(), &sd(), None, None, Model::shipped());
    let places: Vec<(Option<&str>, bool)> = a.review.deaths.iter().map(|n| (n.place.as_deref(), n.traded)).collect();
    assert_eq!(places, [(Some("enemy jungle"), false), (Some("enemy jungle"), false), (Some("river"), true)]);
    // Gold held comes from a frame at most 40 s old; 650 s is 50 s after the 600 s frame.
    let held: Vec<u32> = a.review.deaths.iter().map(|n| n.unspent).collect();
    assert_eq!(held, [1200, 1200, 0]);
    assert!(a.review.notes.contains(&"Died 2× in the enemy jungle (trades aside)".to_string()), "{:?}", a.review.notes);
    assert_eq!(a.opponent, Some(ZED));
    assert!(!a.live && a.recommended.is_empty());
    // My team first, each side in lane order.
    let order: Vec<ChampKey> = a.players.iter().map(|p| p.champ).collect();
    assert_eq!(order, [GAREN, LEE_SIN, AHRI, JINX, LEONA, DARIUS, JARVAN, ZED, CAITLYN, THRESH]);
    assert_eq!(a.kill_participation, Some(0.5));
}

#[test]
fn takeaways_lead_with_the_thrown_lead_and_the_swing() {
    let a = analyze_match(&detail(), &sd(), None, None, Model::shipped());
    let text: Vec<(bool, &str)> = a.takeaways.iter().map(|t| (t.good, t.text.as_str())).collect();
    assert_eq!(
        text,
        [
            (false, "Your team led by 3.0k at 10:00 and lost"),
            (false, "Biggest swing: −4.0k gold from 10:00 to 12:00: you died, 4 of your team died, they took Baron"),
            (false, "Behind Zed by 900 gold at 15:00"),
        ]
    );

    // Won anyway: no swing lesson unless it put my team behind.
    let mut d = detail();
    d.game.win = true;
    for f in &mut d.frames {
        for (i, p) in f.players.iter_mut().enumerate() {
            if i < 5 && f.time >= 660.0 {
                p.total_gold += 800;
            }
        }
    }
    let a = analyze_match(&d, &sd(), None, None, Model::shipped());
    assert!(a.takeaways.iter().all(|t| !t.text.starts_with("Biggest swing")), "{:?}", a.takeaways);
}

#[test]
fn a_live_recap_fills_in_held_gold_and_buy_next() {
    let live = DeathNote {
        time: 301.5,
        killer: Some(ZED),
        assisters: vec![],
        gold_gap: Some(-300),
        killer_top_threat: true,
        unspent: 1450,
        buy_next: Some(LUDENS),
        could_buy: true,
        owned: vec![],
        spikes: vec![(ZED, 6)],
        traded: false,
        place: None,
        pos: None,
        sight: None,
    };
    let recap = Recap {
        stats: EogStats {
            champ: AHRI,
            mode: Mode::Rift,
            win: false,
            duration_s: 1500,
            kills: 2,
            deaths: 3,
            assists: 3,
            cs: 200,
            gold: 9000,
            damage_to_champs: 15000,
            damage_taken: 20000,
            vision: 20,
            items: vec![],
        },
        benchmarks: Benchmarks { cs_per_min: Some(6.5), gold_per_min: None, damage_per_min: None, vision_per_min: None },
        recommended: vec![LUDENS, ZHONYAS],
        notes: vec![],
        review: Some(DeathReview { deaths: vec![live], notes: vec![] }),
        summary: None,
        goals: vec![],
        win_curve: vec![],
    };
    let a = analyze_match(&detail(), &sd(), None, Some(&recap), Model::shipped());
    assert!(a.live);
    assert_eq!(a.recommended, [LUDENS, ZHONYAS]);
    let n = &a.review.deaths[0];
    assert_eq!((n.time, n.unspent, n.buy_next, n.place.as_deref()), (301.5, 1450, Some(LUDENS), Some("enemy jungle")));
    assert_eq!(a.review.deaths[1].buy_next, None);
    // The live benchmarks apply: 200 CS over 25 min is 8/min, well above 6.5.
    assert!(a.takeaways.iter().any(|t| t.good && t.text.starts_with("CS 8.0/min")), "{:?}", a.takeaways);
}

#[test]
fn one_enemy_doing_the_killing_gets_a_note() {
    let mut d = detail();
    let mut a = analyze_match(&d, &sd(), None, None, Model::shipped());
    assert!(!a.review.notes.iter().any(|n| n.contains("killed you")), "{:?}", a.review.notes);
    d.kills.push(KillEvent { time: 1300.0, killer: Some(OPP), victim: ME, assisters: vec![], pos: Some((7000, 7000)) });
    a = analyze_match(&d, &sd(), None, None, Model::shipped());
    assert!(a.review.notes.contains(&"Zed killed you 3× (trades aside)".to_string()), "{:?}", a.review.notes);
}

#[test]
fn fights_and_where_i_was_for_objectives() {
    // I sit in top lane all game; Baron (11:30) and a dragon (16:40) go to them far away.
    let mut d = detail();
    for f in &mut d.frames {
        f.players[ME].pos = Some((1500, 13000));
    }
    d.marks.push(TimelineMark {
        time: 1000.0,
        ours: false,
        kind: "Dragon".into(),
        detail: Some("Infernal".into()),
        pos: Some((9850, 4400)),
        mine: false,
        presence: None,
    });
    let a = analyze_match(&d, &sd(), None, None, Model::shipped());
    type Row<'a> = (f32, u32, u32, Presence, bool, Option<&'a str>);
    let fights: Vec<Row> =
        a.fights.iter().map(|f| (f.start, f.won, f.lost, f.presence, f.died_first, f.then.as_ref().map(|m| m.kind.as_str()))).collect();
    assert_eq!(
        fights,
        [
            (300.0, 0, 1, Presence::There, true, None),
            (420.0, 0, 1, Presence::There, true, None),
            (650.0, 2, 1, Presence::There, true, Some("Baron")),
            (700.0, 0, 3, Presence::Away, false, None),
        ]
    );
    let presence: Vec<Option<Presence>> = a.marks.iter().map(|m| m.presence).collect();
    assert_eq!(presence, [Some(Presence::Away), Some(Presence::Away)]);
    assert!(
        a.takeaways.iter().any(|t| t.text == "Away for 2 of 2 objectives they took (Baron 11:30, Dragon 16:40)"),
        "{:?}",
        a.takeaways
    );
    // Solo deaths are not "lost fights" to have died first in.
    assert!(a.takeaways.iter().all(|t| !t.text.starts_with("You died first")));

    // Dead when Baron went: not "away".
    let mut d2 = d.clone();
    d2.kills.push(KillEvent { time: 685.0, killer: Some(OPP), victim: ME, assisters: vec![], pos: Some((1500, 13000)) });
    let a = analyze_match(&d2, &sd(), None, None, Model::shipped());
    assert_eq!(a.marks[0].presence, Some(Presence::Dead));
}

#[test]
fn focus_picks_early_deaths_as_a_goal() {
    let a = analyze_match(&detail(), &sd(), None, None, Model::shipped());
    let f = a.focus.expect("focus");
    assert_eq!(f.text, "At most 1 death before 14:00");
    assert_eq!(f.goal, Some(Goal { kind: GoalKind::EarlyDeaths, target: 1.0 }));
    // Phase rows at 5, 10, 15 and 20 minutes, with the gold lead over Zed.
    let minutes: Vec<u32> = a.phases.iter().map(|r| r.minute).collect();
    assert_eq!(minutes, [5, 10, 15, 20]);
    assert!(a.phases.iter().all(|r| r.gold_lead.is_some()));
}

#[test]
fn this_game_against_my_earlier_games_on_the_champion() {
    let mut d = detail();
    d.game.created = 10_000;
    let earlier = |i: u64| PastGame { game_id: 100 + i, created: i, deaths: 0, cs: 150, ..past_game() };
    let past = vec![earlier(1), earlier(2), earlier(3), PastGame { champ: ZED, created: 4, ..past_game() }];
    let mut a = analyze_match(&d, &sd(), None, None, Model::shipped());
    coach::compare_past(&mut a, &past, &sd());
    let c = a.vs_past.as_ref().expect("comparison");
    assert_eq!((c.games, c.wins), (3, 0));
    let texts: Vec<&str> = a.takeaways.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"Deaths 3 vs your usual 0 on Ahri"), "{texts:?}");
    assert!(texts.contains(&"Your best CS / min in 4 Ahri games (8.0)"), "{texts:?}");

    // Two earlier games are not enough.
    let mut b = analyze_match(&d, &sd(), None, None, Model::shipped());
    coach::compare_past(&mut b, &past[..2], &sd());
    assert!(b.vs_past.is_none());
}

/// Me Leona support with Jinx vs Caitlyn + Thresh: even for 5 minutes, then my duo
/// gains 400 gold a minute; Jinx backs at 6:00; Caitlyn kills me in bot lane at 3:20.
fn bot_lane() -> MatchDetail {
    let mut d = detail();
    d.players.iter_mut().for_each(|p| p.me = false);
    d.me = 4;
    d.players[4].me = true;
    let total = |i: usize, m: u32| 500 + m * 300 + if (i == 3 || i == 4) && m >= 6 { (m - 5) * 200 } else { 0 };
    d.frames = (0..=25)
        .map(|m| Frame {
            time: m as f32 * 60.0,
            players: (0..10)
                .map(|i| FrameStat {
                    total_gold: total(i, m),
                    current_gold: total(i, m) - if i == 3 && m >= 6 { 2400 } else { 500 },
                    level: m / 2 + 1,
                    ..Default::default()
                })
                .collect(),
        })
        .collect();
    let kill = |time: f32, killer: usize, victim: usize, pos: (i32, i32)| KillEvent { time, killer: Some(killer), victim, assisters: vec![9], pos: Some(pos) };
    d.kills = vec![kill(200.0, 8, 4, (13000, 2000)), kill(300.0, 5, 1, (1000, 13000)), kill(900.0, 8, 3, (13000, 2000))];
    d
}

#[test]
fn laning_compares_both_duos_minute_by_minute() {
    let l = analyze_match(&bot_lane(), &sd(), None, None, Model::shipped()).laning.expect("laning");
    assert_eq!((l.allies.as_slice(), l.enemies.as_slice()), ([JINX, LEONA].as_slice(), [CAITLYN, THRESH].as_slice()));
    assert_eq!(l.minutes.len(), 14);
    assert_eq!(l.minutes.iter().map(|r| r.gold_diff).take(7).collect::<Vec<_>>(), [0, 0, 0, 0, 0, 400, 800]);
    assert_eq!((l.minutes[13].my_level, l.minutes[13].opp_level), (8, Some(8)));
    assert_eq!(l.verdict, "Won lane: +3,600 gold at 14:00, ahead from 6:00");
    assert_eq!(l.turned_at, Some(6));
    // Only kills on the lane's players before 14:00.
    let ev: Vec<_> =
        l.events.iter().map(|e| (e.time, e.killer, e.victim, e.ours, e.place.as_deref())).collect();
    assert_eq!(ev, [(200.0, Some(CAITLYN), LEONA, false, Some("bot lane"))]);
    assert_eq!(l.events[0].assists, [THRESH]);
    // Jinx's shop visit; nobody else spent anything.
    let backs: Vec<(ChampKey, bool, u32, u32)> = l.backs.iter().map(|b| (b.champ, b.ally, b.minute, b.spent)).collect();
    assert_eq!(backs, [(JINX, true, 6, 1900)]);
}

#[test]
fn laning_is_me_against_my_opponent_outside_bot_lane_and_absent_off_the_rift() {
    let a = analyze_match(&detail(), &sd(), None, None, Model::shipped());
    let l = a.laning.expect("laning");
    assert_eq!((l.allies.as_slice(), l.enemies.as_slice()), ([AHRI].as_slice(), [ZED].as_slice()));
    let mut aram = detail();
    aram.game.mode = Mode::Aram;
    assert!(analyze_match(&aram, &sd(), None, None, Model::shipped()).laning.is_none());
    let mut short = detail();
    short.game.duration_s = 540;
    assert!(analyze_match(&short, &sd(), None, None, Model::shipped()).laning.is_none());
}

fn live_recap(deaths: Vec<DeathNote>, vision_per_min: Option<f32>) -> Recap {
    Recap {
        stats: EogStats {
            champ: AHRI,
            mode: Mode::Rift,
            win: false,
            duration_s: 1500,
            kills: 2,
            deaths: 3,
            assists: 3,
            cs: 200,
            gold: 9000,
            damage_to_champs: 15000,
            damage_taken: 20000,
            vision: 20,
            items: vec![],
        },
        benchmarks: Benchmarks { cs_per_min: None, gold_per_min: None, damage_per_min: None, vision_per_min },
        recommended: vec![],
        notes: vec![],
        review: Some(DeathReview { deaths, notes: vec![] }),
        summary: None,
        goals: vec![],
        win_curve: vec![],
    }
}

fn seen_death(time: f32, seen: Vec<SeenBefore>) -> DeathNote {
    DeathNote {
        time,
        killer: Some(JARVAN),
        assisters: vec![],
        gold_gap: None,
        killer_top_threat: false,
        unspent: 0,
        buy_next: None,
        could_buy: false,
        owned: vec![],
        spikes: vec![],
        traded: false,
        place: None,
        pos: None,
        sight: Some(DeathSight { covered: 1.0, seen, warned: None, verdict: None, line: None }),
    }
}

fn jarvan(at: (i32, i32)) -> SeenBefore {
    SeenBefore { champ: Some(JARVAN), first: 6.0, last: 1.0, x: at.0, y: at.1 }
}

fn ganked() -> MatchDetail {
    let mut d = detail();
    d.kills[0].killer = Some(6);
    d.kills[1].killer = Some(6);
    d
}

const NEAR: (i32, i32) = (9500, 6800);
const FAR: (i32, i32) = (2500, 12500);

#[test]
fn fog_deaths_get_their_place_and_the_vision_takeaway() {
    let blip = SeenBefore { champ: None, first: 6.0, last: 1.0, x: FAR.0, y: FAR.1 };
    let recap = live_recap(vec![seen_death(301.0, vec![blip]), seen_death(421.0, vec![])], Some(1.5));
    let a = analyze_match(&ganked(), &sd(), None, Some(&recap), Model::shipped());
    let line = |i: usize| a.review.deaths[i].sight.as_ref().and_then(|s| s.line.clone());
    assert_eq!(line(0).as_deref(), Some("Jarvan IV came out of fog in the enemy jungle"), "an unnamed blip across the map is not him");
    assert_eq!(line(1).as_deref(), Some("Jarvan IV came out of fog in the enemy jungle"));
    assert!(
        a.review.notes.contains(&"2 deaths came out of fog in the enemy jungle (Vision 0.80/min vs 1.50 average on Ahri)".to_string()),
        "{:?}",
        a.review.notes
    );
    assert!(!a.takeaways.iter().any(|t| t.text.starts_with("Vision")), "{:?}", a.takeaways);
}

#[test]
fn a_sighting_across_the_map_says_nothing() {
    let recap = live_recap(vec![seen_death(301.0, vec![jarvan(NEAR)]), seen_death(421.0, vec![jarvan(FAR)])], None);
    let a = analyze_match(&ganked(), &sd(), None, Some(&recap), Model::shipped());
    let kinds: Vec<Option<SightKind>> = a.review.deaths.iter().map(|d| d.sight.as_ref().and_then(|s| s.verdict)).collect();
    assert_eq!(kinds, [Some(SightKind::Visible), None, None]);
}

#[test]
fn deaths_seen_coming_are_the_focus() {
    let mut d = ganked();
    d.kills[0].time = 900.0;
    d.kills[1].time = 1000.0;
    d.kills.sort_by(|a, b| a.time.total_cmp(&b.time));
    let recap = live_recap(vec![seen_death(901.0, vec![jarvan(NEAR)]), seen_death(1001.0, vec![jarvan(NEAR)])], None);
    let a = analyze_match(&d, &sd(), None, Some(&recap), Model::shipped());
    let f = a.focus.unwrap();
    assert_eq!((f.text.as_str(), f.why.as_str(), f.goal), ("Look at the minimap before stepping up", "2 of 2 deaths were visible first.", None));
}

fn handset() -> Model {
    Model::parse(include_str!("../../../fixtures/coach/winprob_handset.toml")).unwrap()
}

fn fixture(name: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/../../fixtures/clients/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

/// `match_game.json`: me (participant 4) on blue, red wins, red's Baron at 25:30.
fn saved(model: &Model) -> MatchAnalysis {
    let d = clients::history::parse_match(&fixture("match_game.json"), Some("8f2c1d4e-me00-4a6b-9c1d-000000000000"), Some(&fixture("match_timeline.json"))).unwrap();
    analyze_match(&d, &sd(), None, None, model)
}

#[test]
fn the_win_curve_ends_on_the_winners_side() {
    let a = saved(&handset());
    assert!(!a.game.win);
    let win: Vec<f32> = a.gold.iter().map(|p| p.win.expect("every frame has one")).collect();
    assert!((win[0] - 0.5).abs() < 0.03, "even at 0:00, blue side's edge aside: {win:?}");
    assert!(*win.last().unwrap() < 0.3, "{win:?}");
    assert!(win.iter().all(|w| (0.0..=1.0).contains(w)));

    let w = |t: f32| a.gold.iter().find(|p| (p.time - t).abs() < 1.0).and_then(|p| p.win).unwrap();
    assert!(w(1560.0) < w(1500.0) - 0.1, "Baron given at 25:30");

    // The fixture's held gold jumps every minute, so gold spent swings the early curve.
    let s = &a.swings;
    assert_eq!(s.len(), 5, "{s:?}");
    assert!(s.windows(2).all(|x| x[0].delta.abs() >= x[1].delta.abs()), "largest first: {s:?}");
    assert!(s.iter().all(|w| w.kind != "other" || w.text == "Buys and levels, no fight or objective"), "{s:?}");
    let back = s.iter().find(|w| w.text == "Their Baron buff ran out").unwrap_or_else(|| panic!("{s:?}"));
    assert_eq!((back.time, back.kind.as_str()), (1710.0, "objective"));
    assert!((back.delta - 100.0 * (w(1740.0) - w(1680.0))).abs() < 1e-3 && back.delta > 10.0, "{back:?}");
}

#[test]
fn the_recorded_flex_game_curve_ends_with_the_winners() {
    let d = clients::history::parse_match(&fixture("match_game_flex.json"), Some("me-puuid"), Some(&fixture("match_timeline_flex.json"))).unwrap();
    let a = analyze_match(&d, &sd(), None, None, &handset());
    assert!(a.game.win);
    let last = a.gold.last().and_then(|p| p.win).unwrap();
    assert!(last > 0.9, "{last}");
    assert_eq!(a.swings.len(), 5);
    assert!(a.swings.windows(2).all(|x| x[0].delta.abs() >= x[1].delta.abs()));
    let kinds = ["fight", "death", "objective", "structure", "other"];
    assert!(a.swings.iter().all(|w| kinds.contains(&w.kind.as_str())), "{:?}", a.swings);
    // 8:00 to 9:00: a 1-for-1 at 8:01 (no side's gain), grubs, then a won fight that weighs most.
    let top = &a.swings[0];
    assert_eq!((top.kind.as_str(), top.text.as_str()), ("fight", "Grubs ×3 taken, fight won 2 for 0 in the river"));
    assert!((top.time - 536.741).abs() < 1e-3 && top.delta > 15.0, "{top:?}");
    let lost = a.swings.iter().find(|s| s.delta < 0.0).unwrap();
    assert_eq!(lost.text, "Fight lost 1 for 2 in bot lane, you died");
}

#[test]
fn swings_are_labelled_from_what_happened_in_the_span() {
    // Gold even all game: the curve moves on kills and Baron alone.
    let mut d = detail();
    for f in &mut d.frames {
        let m = (f.time / 60.0) as u32;
        f.players.iter_mut().for_each(|p| *p = FrameStat { total_gold: 500 + m * 400, current_gold: 0, ..Default::default() });
    }
    let a = analyze_match(&d, &sd(), None, None, &handset());
    let s: Vec<(f32, &str, &str, bool)> = a.swings.iter().map(|s| (s.time, s.kind.as_str(), s.text.as_str(), s.delta > 0.0)).collect();
    assert_eq!(
        s,
        [
            (690.0, "objective", "Baron given, fight lost 0 for 3 in mid lane", false),
            (650.0, "fight", "Fight won 2 for 1 in the river, you died", true),
            (300.0, "death", "You died in the enemy jungle", false),
            (420.0, "death", "You died in the enemy jungle", false),
        ]
    );
}

#[test]
fn live_samples_make_the_curve_when_the_app_watched_the_game() {
    let mut recap = live_recap(vec![], None);
    recap.win_curve = (0..=25).map(|m| (m as f32 * 60.0 + 4.0, if m < 12 { 0.55 } else { 0.30 })).collect();
    let a = analyze_match(&detail(), &sd(), None, Some(&recap), &handset());
    assert_eq!(a.gold.iter().map(|p| p.win).collect::<Vec<_>>(), (0..=25).map(|m| Some(if m < 12 { 0.55 } else { 0.30 })).collect::<Vec<_>>());
    assert_eq!(a.swings.len(), 1, "a flat curve has one swing: {:?}", a.swings);
    assert!((a.swings[0].delta + 25.0).abs() < 1e-3);
    // A frame with no sample near it has no point.
    recap.win_curve.retain(|s| s.0 < 600.0 || s.0 > 800.0);
    let a = analyze_match(&detail(), &sd(), None, Some(&recap), &handset());
    assert_eq!((a.gold[9].win, a.gold[10].win, a.gold[13].win, a.gold[14].win), (Some(0.55), None, None, Some(0.30)));
}

#[test]
fn no_fit_no_curve() {
    let a = saved(Model::shipped());
    assert!(a.gold.iter().all(|p| p.win.is_none()) && a.swings.is_empty());
    let rift_only = Model::parse("patch = \"t\"\ngames = 1\nknots = [0, 20]\n[rift]\ngold = [5.0, 6.0]").unwrap();
    let mut aram = detail();
    aram.game.mode = Mode::Aram;
    let a = analyze_match(&aram, &sd(), None, None, &rift_only);
    assert!(a.gold.iter().all(|p| p.win.is_none()) && a.swings.is_empty());
    // Live samples from a game played under an older fit are not shown without one now.
    let mut recap = live_recap(vec![], None);
    recap.win_curve = vec![(0.0, 0.5), (60.0, 0.6)];
    let a = analyze_match(&detail(), &sd(), None, Some(&recap), Model::shipped());
    assert!(a.gold.iter().all(|p| p.win.is_none()) && a.swings.is_empty());
}
