//! Lane calls (level races, a threat's new item) and quiet tips (CS pace,
//! vision, gold held).

mod common;

use coach::{GameInput, LiveInput, Tracker, game_advice, live_extras};
use common::*;
use model::*;
use std::collections::HashMap;

const ZED_I: usize = 7;

fn advise(s: &GameSnapshot, stats: Option<&ChampStats>, tracker: &Tracker) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    live_extras(&mut a, &LiveInput { snap: s, sd: &sd, tracker, ults: &HashMap::new() });
    a
}

fn keys(a: &GameAdvice) -> Vec<&str> {
    a.calls.iter().map(|c| c.key.as_str()).collect()
}

#[test]
fn level_races_with_the_lane_opponent() {
    let mut p = lobby(STANDARD);
    p[0].level = 5;
    p[ZED_I].level = 6;
    let a = advise(&snap(p.clone(), vec![]), None, &Tracker::default());
    let c = a.calls.iter().find(|c| c.key == "lvl-opp-6").expect("call");
    assert_eq!(c.text, "Zed hit level 6 before you (6 vs 5): play safe until you do");

    (p[0].level, p[ZED_I].level) = (6, 5);
    assert!(keys(&advise(&snap(p.clone(), vec![]), None, &Tracker::default())).contains(&"lvl-me-6"));

    // Level 11 race; dead, nothing.
    (p[0].level, p[ZED_I].level) = (10, 11);
    assert!(keys(&advise(&snap(p.clone(), vec![]), None, &Tracker::default())).contains(&"lvl-opp-11"));
    p[0].dead = true;
    p[0].respawn_in = 20.0;
    assert!(keys(&advise(&snap(p, vec![]), None, &Tracker::default())).iter().all(|k| !k.starts_with("lvl-")));
}

#[test]
fn a_new_item_on_the_lane_opponent_is_called_once_tracked() {
    let sd = sd();
    let mut t = Tracker::default();
    let mut p = lobby(STANDARD);
    t.observe(&GameSnapshot { time: 800.0, ..snap(p.clone(), vec![]) }, &sd);
    p[ZED_I].items.push(YOUMUUS);
    let s = GameSnapshot { time: 850.0, ..snap(p, vec![]) };
    t.observe(&s, &sd);
    let a = advise(&s, None, &t);
    let c = a.calls.iter().find(|c| c.key == format!("item-{ZED}-{YOUMUUS}")).expect("item call");
    assert!(c.text.starts_with("Zed finished "), "{}", c.text);
    assert!(c.text.ends_with("respect the spike"), "{}", c.text);
}

#[test]
fn quiet_tips_for_cs_pace_vision_and_wards() {
    // 15:00, 90 CS against 7.4/min: 21 behind; no Control Ward held.
    let mut p = lobby(STANDARD);
    p[0].ward_score = 2.0;
    let a = advise(&snap(p.clone(), vec![]), Some(&ahri_stats()), &Tracker::default());
    let tips: Vec<(&str, &str)> = a.tips.iter().map(|t| (t.key.as_str(), t.text.as_str())).collect();
    assert!(tips.contains(&("cs", "CS 21 behind the average pace (6.0/min vs 7.4): last-hit every wave, catch side waves")), "{tips:?}");
    assert!(tips.iter().any(|t| t.0 == "vision-low"), "{tips:?}");
    assert!(tips.contains(&("control-ward", "Next back: buy a Control Ward (75 g)")), "{tips:?}");

    // Holding one: no reminder. Sona with a Stealth Ward: swap to Oracle Lens.
    p[0].items.push(2055);
    p[4].items.push(3340);
    let mut s = snap(p, vec![]);
    let a = advise(&s, Some(&ahri_stats()), &Tracker::default());
    assert!(a.tips.iter().all(|t| t.key != "control-ward"), "{:?}", a.tips);
    s.me = 4;
    assert!(advise(&s, None, &Tracker::default()).tips.iter().any(|t| t.key == "oracle"));
}

#[test]
fn gold_held_with_buy_next_affordable_is_a_back_tip() {
    let sd = sd();
    let p = lobby(STANDARD);
    let mut t = Tracker::default();
    let at = |time: f32, gold: f32| GameSnapshot { time, my_gold: gold, ..snap(p.clone(), vec![]) };
    for i in 0..=20 {
        t.observe(&at(800.0 + 5.0 * i as f32, 3500.0), &sd);
    }
    let a = advise(&at(900.0, 3500.0), None, &t);
    assert!(a.gold_eta.as_ref().is_some_and(|e| e.missing == 0), "{:?}", a.gold_eta);
    let tip = a.tips.iter().find(|t| t.key == "back").expect("back tip");
    assert!(tip.text.starts_with("You've held 3500 gold for 1:40"), "{}", tip.text);

    // Just earned it: no tip yet.
    let mut fresh = Tracker::default();
    fresh.observe(&at(880.0, 400.0), &sd);
    fresh.observe(&at(895.0, 3500.0), &sd);
    assert!(advise(&at(900.0, 3500.0), None, &fresh).tips.iter().all(|t| t.key != "back"));
}
