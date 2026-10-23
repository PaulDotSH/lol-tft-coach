//! Self-coaching: death review, deaths holding gold, goal progress and trends.

mod common;

use coach::{GameInput, GameLog, Now, death_review, game_advice, goal_progress, summary, trends};
use common::*;
use model::*;

/// One in-game tick through the real coach into the log.
fn tick(log: &mut GameLog, sd: &StaticData, time: f32, gold: f32, players: &[Player], events: &[GameEvent]) {
    let snap = GameSnapshot { time, my_gold: gold, players: players.to_vec(), events: events.to_vec(), ..snap(vec![], vec![]) };
    let advice = game_advice(&GameInput { snap: &snap, sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    log.observe(&snap, &advice, sd);
}

fn with_assist(time: f32, killer: usize, assister: usize) -> GameEvent {
    GameEvent::Kill { time, killer: Some(killer), victim: 0, assisters: vec![assister] }
}

/// Malphite top (index 5), Zed mid (7).
fn lobby_malphite() -> Vec<Player> {
    let mut p = lobby(STANDARD);
    p[5] = player(MALPHITE, Team::Chaos, Some(Lane::Top));
    for x in &mut p {
        x.level = 1;
    }
    p
}

#[test]
fn death_review_spikes_and_gold() {
    let sd = sd();
    let mut p = lobby_malphite();
    let mut ev = vec![];
    let mut log = GameLog::default();
    tick(&mut log, &sd, 0.0, 500.0, &p, &ev);

    p[5].level = 5;
    tick(&mut log, &sd, 340.0, 900.0, &p, &ev);
    p[5].level = 6;
    tick(&mut log, &sd, 360.0, 1150.0, &p, &ev);
    tick(&mut log, &sd, 378.0, 1200.0, &p, &ev);
    ev.push(kill(380.0, 5, 0));
    tick(&mut log, &sd, 380.5, 1200.0, &p, &ev);

    p[5].level = 11;
    tick(&mut log, &sd, 700.0, 250.0, &p, &ev);
    tick(&mut log, &sd, 728.0, 300.0, &p, &ev);
    ev.push(with_assist(730.0, 7, 5));
    tick(&mut log, &sd, 731.0, 300.0, &p, &ev);

    tick(&mut log, &sd, 998.0, 1500.0, &p, &ev);
    ev.push(kill(1000.0, 7, 0));
    tick(&mut log, &sd, 1001.0, 1500.0, &p, &ev);
    // Re-listed events are not counted twice.
    tick(&mut log, &sd, 1003.0, 1500.0, &p, &ev);

    let r = death_review(&log, &sd);
    assert_eq!(r.deaths.len(), 3);
    let d = &r.deaths[0];
    assert_eq!((d.time, d.killer, d.unspent, d.spikes.clone()), (380.0, Some(MALPHITE), 1200, vec![(MALPHITE, 6)]));
    assert_eq!(d.owned, vec![RUBY]);
    let d = &r.deaths[1];
    assert_eq!((d.killer, d.assisters.clone(), d.spikes.clone()), (Some(ZED), vec![MALPHITE], vec![(MALPHITE, 11)]));
    assert_eq!(r.deaths[2].spikes, vec![], "Malphite's 11 was 300 s earlier");
    assert!(r.deaths[2].gold_gap.is_some());
    assert_eq!(
        r.notes[..2],
        ["Died 2× within 60 s of Malphite reaching level 6/11".to_string(), "Died 2× with 1000+ g unspent".to_string()]
    );
}

#[test]
fn killer_top_threat_and_buy_next() {
    let sd = sd();
    let mut p = lobby(STANDARD);
    p[7] = p[7].clone().kda(12, 1, 3).lvl(11).items(&[YOUMUUS, EDGE_OF_NIGHT]);
    let mut log = GameLog::default();
    let mut ev = vec![];
    tick(&mut log, &sd, 60.0, 400.0, &p, &ev);
    for t in [600.0, 700.0] {
        tick(&mut log, &sd, t - 1.0, 3500.0, &p, &ev);
        ev.push(kill(t, 7, 0));
    }
    tick(&mut log, &sd, 702.0, 3500.0, &p, &ev);
    let r = death_review(&log, &sd);
    // Each death is judged on the tick before it (599 s, 699 s).
    assert_eq!(r.deaths.len(), 2);
    assert!(r.deaths.iter().all(|d| d.killer_top_threat && d.could_buy && d.buy_next.is_some()));
    assert!(r.deaths[0].gold_gap.unwrap() < -4000);
    assert!(r.notes.contains(&"2 of 2 deaths were to the enemy on top of the threat board".to_string()), "{:?}", r.notes);
    assert!(r.notes.iter().any(|n| n.starts_with("You had") && n.ends_with("less item gold than your killer on average")));
}

#[test]
fn deaths_before_the_app_started_are_ignored() {
    let sd = sd();
    let p = lobby(STANDARD);
    let ev = killed_me(7, 3); // 300, 360, 420
    let mut log = GameLog::default();
    tick(&mut log, &sd, 500.0, 500.0, &p, &ev);
    tick(&mut log, &sd, 502.0, 500.0, &p, &ev);
    assert_eq!(log.deaths.len(), 0);
}

#[test]
fn cs_at_10_early_deaths_and_buys() {
    let sd = sd();
    let mut p = lobby(STANDARD);
    let mut log = GameLog::default();
    let ev = vec![];
    p[0] = p[0].clone().cs(60);
    tick(&mut log, &sd, 598.0, 3000.0, &p, &ev);
    let suggested = {
        let snap = GameSnapshot { time: 598.0, my_gold: 3000.0, players: p.clone(), ..snap(vec![], vec![]) };
        game_advice(&GameInput { snap: &snap, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() }).buy_next.unwrap().item
    };
    p[0] = p[0].clone().cs(66).items(&[RUBY, suggested, LONG_SWORD]);
    tick(&mut log, &sd, 600.5, 100.0, &p, &ev);
    assert_eq!((log.cs_10, log.buys, log.followed), (Some(66), 1, 1), "component not counted");

    p[0].deaths = 2;
    tick(&mut log, &sd, 838.0, 100.0, &p, &ev);
    let other = INFINITY_EDGE; // never suggested for Ahri
    p[0] = p[0].clone().items(&[RUBY, suggested, LONG_SWORD, other]);
    tick(&mut log, &sd, 841.0, 100.0, &p, &ev);
    assert_eq!(log.early_deaths, Some(2));
    assert_eq!((log.buys, log.followed), (2, 1));
}

fn goals(k: &[(GoalKind, f32)]) -> Vec<Goal> {
    k.iter().map(|&(kind, target)| Goal { kind, target }).collect()
}

fn now(time: f32, cs: u32, deaths: u32, done: bool) -> Now {
    Now { mode: Mode::Rift, time, cs, deaths, vision: 18.0, done }
}

#[test]
fn goal_progress_live_and_final() {
    let mut log = GameLog::default();
    let g = goals(&[(GoalKind::Cs10, 7.0), (GoalKind::EarlyDeaths, 1.0)]);
    // 5:00, 33 CS: pace 6.6/min; one death, still fine.
    let p = goal_progress(&g, &log, &now(300.0, 33, 1, false));
    assert_eq!(p[0], GoalProgress { kind: GoalKind::Cs10, target: 7.0, value: Some(6.6), met: None });
    assert_eq!(p[1], GoalProgress { kind: GoalKind::EarlyDeaths, target: 1.0, value: Some(1.0), met: None });
    // Two deaths before 14:00: missed already.
    assert_eq!(goal_progress(&g, &log, &now(500.0, 50, 2, false))[1].met, Some(false));

    log.cs_10 = Some(72);
    log.early_deaths = Some(1);
    let p = goal_progress(&g, &log, &now(900.0, 100, 4, false));
    assert_eq!((p[0].value, p[0].met), (Some(7.2), Some(true)));
    assert_eq!((p[1].value, p[1].met), (Some(1.0), Some(true)));

    let g = goals(&[(GoalKind::Unspent, 1000.0), (GoalKind::BuyNext, 60.0), (GoalKind::Vision, 1.0)]);
    let p = goal_progress(&g, &log, &now(900.0, 100, 0, false));
    assert_eq!((p[0].value, p[0].met), (Some(0.0), None));
    assert_eq!((p[1].value, p[1].met), (None, None), "nothing bought yet");
    assert_eq!((p[2].value, p[2].met), (Some(1.2), None));
    log.buys = 4;
    log.followed = 3;
    let p = goal_progress(&g, &log, &now(1800.0, 200, 0, true));
    assert_eq!((p[0].met, p[1].value, p[1].met, p[2].met), (Some(true), Some(75.0), Some(true), Some(false)));

    // Rift-only goals don't apply in ARAM.
    let aram = Now { mode: Mode::Aram, ..now(300.0, 10, 0, false) };
    let g = goals(&[(GoalKind::Cs10, 7.0), (GoalKind::Unspent, 1000.0)]);
    assert_eq!(goal_progress(&g, &log, &aram).iter().map(|p| p.kind).collect::<Vec<_>>(), [GoalKind::Unspent]);
}

#[test]
fn summary_for_trends() {
    let sd = sd();
    let mut p = lobby(STANDARD);
    let mut log = GameLog::default();
    let mut ev = vec![];
    tick(&mut log, &sd, 30.0, 500.0, &p, &ev);
    tick(&mut log, &sd, 499.0, 1300.0, &p, &ev);
    ev.push(kill(500.0, 7, 0));
    p[0].deaths = 1;
    tick(&mut log, &sd, 501.0, 1300.0, &p, &ev);
    let eog = EogStats {
        champ: AHRI,
        mode: Mode::Rift,
        win: true,
        duration_s: 780,
        kills: 3,
        deaths: 1,
        assists: 2,
        cs: 100,
        gold: 6000,
        damage_to_champs: 9000,
        damage_taken: 8000,
        vision: 9,
        items: vec![],
    };
    let s = summary(&log, &eog, 1_792_000_000);
    assert_eq!((s.deaths_before_14, s.unspent_at_death.clone(), s.cs_10, s.win), (Some(1), vec![1300], None, true));
}

// ---- trends ----

fn past(i: usize, mode: Mode, win: bool, cs_10: u32, early: usize) -> PastGame {
    PastGame {
        game_id: i as u64,
        created: 0,
        queue_id: 420,
        mode,
        champ: AHRI,
        lane: None,
        win,
        remake: false,
        duration_s: 1800,
        kills: 5,
        deaths: 5,
        assists: 5,
        cs: 200,
        gold: 10000,
        damage_to_champs: 30000,
        vision: 30,
        death_times: Some((0..early).map(|k| 100 + 100 * k as u32).chain([1500]).collect()),
        gold_diff_10: None,
        gold_diff_15: None,
        xp_diff_15: None,
        ours: None,
        theirs: None,
        first_dragon: None,
        first_tower: None,
        first_blood: None,
        cs_10: Some(cs_10),
        augments: vec![],
    }
}

fn sum(mode: Mode, win: bool, unspent: &[u32], buys: u32, followed: u32) -> GameSummary {
    GameSummary {
        ended: 0,
        mode,
        champ: AHRI,
        win,
        duration_s: 1500,
        cs_10: None,
        deaths_before_14: None,
        vision: 10,
        unspent_at_death: unspent.to_vec(),
        buys,
        followed,
    }
}

#[test]
fn trends_per_mode() {
    // Newest first: 5 recent games at 75 CS at 10, 6 older at 55.
    let mut games: Vec<PastGame> = (0..11).map(|i| past(i, Mode::Rift, i % 2 == 0, if i < 5 { 75 } else { 55 }, 1)).collect();
    games.push(PastGame { remake: true, ..past(99, Mode::Rift, false, 0, 9) });
    games.extend((20..24).map(|i| past(i, Mode::Aram, true, 0, 3)));
    let summaries = vec![
        sum(Mode::Rift, true, &[200], 4, 4),
        sum(Mode::Rift, false, &[1400, 300], 4, 1),
        sum(Mode::Rift, true, &[], 3, 3),
        sum(Mode::Rift, false, &[1200], 2, 1),
        sum(Mode::Aram, false, &[1800], 2, 0),
    ];
    let g = goals(&[(GoalKind::Cs10, 7.0), (GoalKind::Unspent, 1000.0)]);
    let t = trends(&games, &summaries, &g);

    let cs = t.trends.iter().find(|x| x.mode == Mode::Rift && x.kind == GoalKind::Cs10).unwrap();
    assert_eq!((cs.recent, cs.before, cs.better), (7.5, Some(5.5), Some(true)));
    assert_eq!(cs.series.len(), 11, "remake excluded");
    assert_eq!((cs.series[0], cs.series[10]), (5.5, 7.5), "oldest first");
    let early = t.trends.iter().find(|x| x.mode == Mode::Rift && x.kind == GoalKind::EarlyDeaths).unwrap();
    assert_eq!((early.recent, early.better), (1.0, None), "flat");
    let unspent = t.trends.iter().find(|x| x.kind == GoalKind::Unspent).unwrap();
    assert_eq!((unspent.mode, unspent.series.clone(), unspent.before), (Mode::Rift, vec![1200.0, 850.0, 200.0], None));
    // ARAM: no Rift-only metrics; too few summaries for others.
    assert!(t.trends.iter().filter(|x| x.mode == Mode::Aram).all(|x| !matches!(x.kind, GoalKind::Cs10 | GoalKind::EarlyDeaths | GoalKind::Vision)));

    assert_eq!(t.follow, vec![FollowSplit { mode: Mode::Rift, wins: Some(100.0), losses: Some(37.5), games: 4 }]);
    assert_eq!(t.goals[0], GoalStatus { goal: g[0].clone(), met: 5, of: 10 });
    assert_eq!(t.goals[1], GoalStatus { goal: g[1].clone(), met: 1, of: 4 });
}

#[test]
fn mayhem_augments_games_and_damage_only() {
    let mk = |i, augs: &[AugmentId], dmg| PastGame { augments: augs.to_vec(), damage_to_champs: dmg, ..past(i, Mode::Mayhem, i % 2 == 0, 0, 0) };
    let games = vec![mk(1, &[1030, 2132], 60000), mk(2, &[1030], 30000), mk(3, &[1389], 90000)];
    let t = trends(&games, &[], &[]);
    assert_eq!(
        t.augments,
        vec![
            AugmentUse { id: 1030, games: 2, damage_per_min: 1500.0 },
            AugmentUse { id: 1389, games: 1, damage_per_min: 3000.0 },
            AugmentUse { id: 2132, games: 1, damage_per_min: 2000.0 },
        ]
    );
}

#[test]
fn traded_deaths_stay_out_of_the_blame_notes() {
    let sd = sd();
    let mut p = lobby(STANDARD);
    p[7] = p[7].clone().kda(12, 1, 3).lvl(11).items(&[YOUMUUS, EDGE_OF_NIGHT]);
    let mut log = GameLog::default();
    let mut ev = vec![];
    tick(&mut log, &sd, 60.0, 300.0, &p, &ev);
    tick(&mut log, &sd, 599.0, 300.0, &p, &ev);
    ev.push(kill(600.0, 7, 0));
    // Two enemies die right after: I went down for a won fight.
    ev.extend([kill(604.0, 1, 7), kill(607.0, 2, 8)]);
    tick(&mut log, &sd, 612.0, 300.0, &p, &ev);
    tick(&mut log, &sd, 699.0, 300.0, &p, &ev);
    ev.push(kill(700.0, 7, 0));
    tick(&mut log, &sd, 712.0, 300.0, &p, &ev);

    let r = death_review(&log, &sd);
    assert_eq!(r.deaths.iter().map(|d| d.traded).collect::<Vec<_>>(), vec![true, false]);
    assert!(r.deaths.iter().all(|d| d.killer_top_threat), "both still to the top threat");
    assert_eq!(r.notes, vec!["1 of 2 deaths were trades: your team won the fight".to_string()]);
}

#[test]
fn a_long_teamfight_is_judged_whole() {
    let sd = sd();
    let p = lobby(STANDARD);
    let mut log = GameLog::default();
    let mut ev = vec![];
    tick(&mut log, &sd, 1000.0, 300.0, &p, &ev);
    // We win the opening (three kills 8 s apart), then I die 14 s after the first.
    ev.extend([kill(1400.0, 1, 7), kill(1408.0, 2, 8), kill(1412.0, 3, 9), kill(1414.0, 7, 0), kill(1416.0, 5, 1)]);
    tick(&mut log, &sd, 1420.0, 300.0, &p, &ev);
    // A pick 30 s later is a separate fight.
    ev.push(kill(1450.0, 6, 0));
    tick(&mut log, &sd, 1452.0, 300.0, &p, &ev);
    let r = death_review(&log, &sd);
    assert_eq!(r.deaths.iter().map(|d| d.traded).collect::<Vec<_>>(), vec![true, false], "3 for 2, then a lone pick");
}
