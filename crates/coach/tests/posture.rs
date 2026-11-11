//! Posture: the stance table, its hysteresis and dwell, options from live facts, the stance-change
//! call, and ARAM without objectives.

mod common;

use coach::winprob::Model;
use coach::{GameInput, LiveInput, PostureMemo, Tracker, game_advice, live_extras, posture};
use common::*;
use model::*;
use std::collections::HashMap;

const HANDSET: &str = include_str!("../../../fixtures/coach/winprob_handset.toml");

fn advise(s: &GameSnapshot, winprob: Option<&Model>) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob, situations: &Default::default() });
    live_extras(&mut a, &LiveInput { snap: s, sd: &sd, tracker: &Tracker::default(), ults: &HashMap::new(), notes: &LaneNotes::default(), spells: &HashMap::new(), memory: &[] });
    a
}

/// A win probability as the model would hand it over: `base` standing, the outlook (now, later).
fn win(base: f32, outlook: Option<(f32, f32)>, stakes: &[(&str, f32)]) -> WinProb {
    WinProb {
        p: base,
        base,
        trend: None,
        factors: vec![WinFactor { key: "gold".into(), label: "item gold".into(), points: 100.0 * (base - 0.5) }],
        stakes: stakes.iter().map(|(n, s)| Stake { name: n.to_string(), swing: *s, note: None }).collect(),
        outlook: outlook.map(|(now, later)| Outlook { now, later, later_min: 25 }),
        rough: false,
        source: "test".into(),
    }
}

fn at(time: f32) -> GameSnapshot {
    GameSnapshot { time, ..snap(lobby(STANDARD), vec![]) }
}

fn read(memo: &mut PostureMemo, s: &GameSnapshot, w: &WinProb) -> Posture {
    let p = memo.decide(w, &advise(s, None), s, &sd());
    for line in std::iter::once(&p.headline).chain(&p.options) {
        assert!(!["Take ", "Push ", "Go ", "Group ", "Fight ", "End ", "Back "].iter().any(|v| line.starts_with(v)), "a read, not an order: {line}");
    }
    p
}

fn first(w: &WinProb) -> Posture {
    read(&mut PostureMemo::default(), &at(1140.0), w)
}

#[test]
fn ahead_and_time_does_not_favour_you_is_press() {
    for outlook in [None, Some((2.0, 1.0)), Some((1.0, 1.5))] {
        let p = first(&win(0.66, outlook, &[("Baron", 12.0)]));
        assert_eq!(p.stance, Stance::Press, "{outlook:?}");
        assert!(p.headline.contains("the lead is worth most now"), "{}", p.headline);
    }
}

#[test]
fn ahead_and_time_on_your_side_is_hold() {
    let p = first(&win(0.66, Some((0.0, 3.0)), &[("Baron", 12.0)]));
    assert_eq!(p.stance, Stance::Hold);
    assert!(p.headline.contains("time is on your side"), "{}", p.headline);
    assert!(p.why.iter().any(|w| w.contains("favour you")), "{:?}", p.why);
    assert!(p.why.iter().any(|w| w.starts_with("item gold +16")), "{:?}", p.why);
}

#[test]
fn between_forty_and_sixty_is_even_and_names_the_objective_that_decides() {
    for outlook in [None, Some((0.0, 3.0)), Some((0.0, -3.0))] {
        let p = first(&win(0.5, outlook, &[("Baron", 12.0), ("Dragon", 4.0), ("Tower", 3.0)]));
        assert_eq!(p.stance, Stance::Even);
        assert!(p.headline.contains("Baron") && p.headline.contains("±12"), "{}", p.headline);
        assert!(p.options[0].starts_with("Baron in 1:00, ±12 points"), "{:?}", p.options);
    }
    assert!(first(&win(0.5, None, &[])).headline.contains("the next objective decides"));
}

#[test]
fn behind_and_time_on_your_side_is_stall() {
    let p = first(&win(0.33, Some((0.0, 3.0)), &[("Baron", 12.0), ("Dragon", 4.0), ("Tower", 3.0)]));
    assert_eq!(p.stance, Stance::Stall);
    assert!(p.headline.contains("the game comes to you"), "{}", p.headline);
    assert!(p.options.iter().any(|o| o.starts_with("Least to lose: next tower, ±3 points")), "{:?}", p.options);
}

#[test]
fn behind_and_time_against_you_is_swing() {
    let p = first(&win(0.33, Some((0.0, -3.0)), &[("Baron", 12.0)]));
    assert_eq!(p.stance, Stance::Swing);
    assert!(p.headline.contains("time is against you"), "{}", p.headline);
    assert!(p.why.iter().any(|w| w.contains("favour them")), "{:?}", p.why);
    let nobody = first(&win(0.33, None, &[]));
    assert_eq!(nobody.stance, Stance::Swing);
    assert!(nobody.headline.contains("waiting does not close the gap"), "{}", nobody.headline);
}

#[test]
fn a_stance_flips_only_three_points_past_a_threshold() {
    use Stance::*;
    let mut m = PostureMemo::default();
    let mut st = |time: f32, base: f32| read(&mut m, &at(time), &win(base, None, &[])).stance;
    assert_eq!(st(600.0, 0.50), Even);
    assert_eq!(st(700.0, 0.62), Even, "two points past 60 % is not enough");
    assert_eq!(st(800.0, 0.64), Press);
    assert_eq!(st(900.0, 0.58), Press, "and back only three points under it");
    assert_eq!(st(1000.0, 0.56), Even);
    assert_eq!(st(1100.0, 0.38), Even);
    assert_eq!(st(1200.0, 0.36), Swing);
    assert_eq!(st(1300.0, 0.42), Swing);
    assert_eq!(st(1400.0, 0.44), Even);
    assert_eq!(first(&win(0.61, None, &[])).stance, Press, "the first read takes the thresholds as they are");
    assert_eq!(first(&win(0.39, None, &[])).stance, Swing);
}

#[test]
fn a_stance_holds_45_seconds_and_a_change_is_one_call() {
    let sd = sd();
    let run = |m: &mut PostureMemo, time: f32, base: f32| {
        let s = at(time);
        let mut a = advise(&s, None);
        a.win = Some(win(base, None, &[]));
        posture(&mut a, m, &s, &sd);
        let keys: Vec<String> = a.calls.iter().filter(|c| c.key.starts_with("stance-")).map(|c| c.key.clone()).collect();
        (a, keys)
    };
    let mut m = PostureMemo::default();
    let (a, keys) = run(&mut m, 600.0, 0.5);
    assert_eq!((a.posture.unwrap().stance, keys.len()), (Stance::Even, 0), "the first read is no change");
    let (a, keys) = run(&mut m, 620.0, 0.7);
    assert_eq!((a.posture.unwrap().stance, keys.len()), (Stance::Even, 0), "20 s in");
    let (a, keys) = run(&mut m, 645.0, 0.7);
    let p = a.posture.unwrap();
    assert_eq!((p.stance, keys.as_slice()), (Stance::Press, ["stance-press-1".to_string()].as_slice()));
    let call = a.calls.iter().find(|c| c.key == "stance-press-1").unwrap();
    assert_eq!(call.text, p.headline);
    assert!(!call.say.is_empty());
    assert_eq!((m.stance, m.since), (Some(Stance::Press), 645.0));
    assert_eq!(run(&mut m, 680.0, 0.7).1, ["stance-press-1"], "the key holds while the call stands");
    assert!(run(&mut m, 691.0, 0.7).1.is_empty(), "and the call goes 45 s on");
    assert_eq!(run(&mut m, 700.0, 0.5).1, ["stance-even-2"]);
    assert_eq!(run(&mut m, 750.0, 0.7).1, ["stance-press-3"], "a stance taken again is a new call");
    let (a, keys) = run(&mut m, 100.0, 0.5);
    assert_eq!((a.posture.unwrap().stance, keys.len()), (Stance::Even, 0), "the clock went back: a new game, no dwell, no call");

    // Whether it is spoken is the UI's `posture_calls`: the call is always there.
    let mut other = PostureMemo::default();
    run(&mut other, 600.0, 0.5);
    let (a, keys) = run(&mut other, 700.0, 0.7);
    assert_eq!((a.posture.unwrap().stance, keys.as_slice()), (Stance::Press, ["stance-press-1".to_string()].as_slice()));
}

#[test]
fn options_name_the_biggest_stake_with_its_timer_and_the_shutdowns() {
    // 19:00, Baron at 20:00; Zed took Garen three times; Lee Sin is dead for 25 s.
    let mut p = lobby(STANDARD);
    p[6].dead = true;
    p[6].respawn_in = 25.0;
    let s = GameSnapshot { time: 1140.0, ..snap(p, vec![kill(600.0, 7, 1), kill(800.0, 7, 1), kill(1000.0, 7, 1)]) };
    let a = advise(&s, None);
    let stakes = [("Baron", 14.0), ("Dragon", 5.0), ("Tower", 3.0), ("Your death", 4.0)];
    let swing = PostureMemo::default().decide(&win(0.33, Some((0.0, -3.0)), &stakes), &a, &s, &sd());
    assert_eq!(swing.stance, Stance::Swing);
    assert_eq!(swing.options, ["Zed is on a 3-kill streak: a big shutdown", "Dead on their side: Lee Sin (25 s)", "Baron in 1:00, ±14 points"]);
    let press = PostureMemo::default().decide(&win(0.66, None, &stakes), &a, &s, &sd());
    assert_eq!(press.options, ["Baron in 1:00, ±14 points", "Dead on their side: Lee Sin (25 s)", "Zed is on a 3-kill streak: a big shutdown"]);
    let hold = PostureMemo::default().decide(&win(0.66, Some((0.0, 3.0)), &stakes), &a, &s, &sd());
    assert!(hold.options.len() >= 2 && hold.options[1] == "Dying now costs 4 points", "{:?}", hold.options);
}

#[test]
fn no_win_probability_no_posture() {
    let (sd, s) = (sd(), at(1140.0));
    let mut a = advise(&s, None);
    assert_eq!(a.win, None, "the built-in table has no fit yet");
    let mut m = PostureMemo::default();
    posture(&mut a, &mut m, &s, &sd);
    assert_eq!(a.posture, None);
    assert!(a.calls.iter().all(|c| !c.key.starts_with("stance-")));
    assert_eq!(m.stance, None);
}

#[test]
fn aram_and_mayhem_postures_have_no_dragons_or_baron() {
    let (sd, m) = (sd(), Model::parse(HANDSET).unwrap());
    for mode in [Mode::Aram, Mode::Mayhem] {
        let s = GameSnapshot { mode, map: 12, ..snap(lobby(STANDARD), vec![GameEvent::Tower { time: 600.0, team: Team::Chaos, lane: Some(Lane::Middle), tier: 2 }]) };
        let mut a = advise(&s, Some(&m));
        let w = a.win.clone().unwrap();
        assert!(w.stakes.iter().all(|x| x.name == "Tower" || x.name == "Inhibitor"), "{:?}", w.stakes);
        posture(&mut a, &mut PostureMemo::default(), &s, &sd);
        let p = a.posture.clone().unwrap();
        assert!(p.options.iter().any(|o| o.contains("tower") || o.contains("inhibitor")), "{mode:?} {:?}", p.options);
        let text = format!("{} {:?} {:?}", p.headline, p.why, p.options);
        assert!(!["Dragon", "Baron", "Elder", "Herald", "Grubs"].iter().any(|n| text.contains(n)), "{mode:?}: {text}");

        // Even a stake named for a Rift objective is left out there.
        let even = PostureMemo::default().decide(&win(0.5, None, &[("Baron", 12.0), ("Tower", 5.0)]), &a, &s, &sd);
        assert!(even.headline.contains("next tower") && !even.options.iter().any(|o| o.contains("Baron")), "{even:?}");
    }
}
