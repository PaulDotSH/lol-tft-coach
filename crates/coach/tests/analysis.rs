//! Match-history review: deaths with places and trades, gold swings, takeaways,
//! and what a live recap adds.

mod common;

use coach::analyze_match;
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
    let a = analyze_match(&detail(), &sd(), None, None);
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
    let a = analyze_match(&detail(), &sd(), None, None);
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
    let a = analyze_match(&d, &sd(), None, None);
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
    };
    let a = analyze_match(&detail(), &sd(), None, Some(&recap));
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
    let mut a = analyze_match(&d, &sd(), None, None);
    assert!(!a.review.notes.iter().any(|n| n.contains("killed you")), "{:?}", a.review.notes);
    d.kills.push(KillEvent { time: 1300.0, killer: Some(OPP), victim: ME, assisters: vec![], pos: Some((7000, 7000)) });
    a = analyze_match(&d, &sd(), None, None);
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
    let a = analyze_match(&d, &sd(), None, None);
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
    let a = analyze_match(&d2, &sd(), None, None);
    assert_eq!(a.marks[0].presence, Some(Presence::Dead));
}

#[test]
fn focus_picks_early_deaths_as_a_goal() {
    let a = analyze_match(&detail(), &sd(), None, None);
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
    let mut a = analyze_match(&d, &sd(), None, None);
    coach::compare_past(&mut a, &past, &sd());
    let c = a.vs_past.as_ref().expect("comparison");
    assert_eq!((c.games, c.wins), (3, 0));
    let texts: Vec<&str> = a.takeaways.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"Deaths 3 vs your usual 0 on Ahri"), "{texts:?}");
    assert!(texts.contains(&"Your best CS / min in 4 Ahri games (8.0)"), "{texts:?}");

    // Two earlier games are not enough.
    let mut b = analyze_match(&d, &sd(), None, None);
    coach::compare_past(&mut b, &past[..2], &sd());
    assert!(b.vs_past.is_none());
}
