//! Live calls: shutdown bounties, fight readiness, objective calls.

mod common;

use coach::{GameInput, LiveInput, Tracker, game_advice, live_extras};
use common::*;
use model::*;
use std::collections::HashMap;

fn advise(s: &GameSnapshot) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None });
    live_extras(&mut a, &LiveInput { snap: s, sd: &sd, tracker: &Tracker::default(), ults: &HashMap::new() });
    a
}

#[test]
fn kill_streaks_and_death_streaks_set_bounties() {
    // Zed (7) kills Garen (1) three times; I kill Lee Sin, then Darius kills me.
    let ev = vec![kill(100.0, 7, 1), kill(200.0, 7, 1), kill(300.0, 0, 6), kill(400.0, 7, 1), kill(500.0, 5, 0)];
    let a = advise(&snap(lobby(STANDARD), ev));
    let lines: Vec<(ChampKey, bool, &str)> = a.bounties.iter().map(|b| (b.champ, b.ally, b.text.as_str())).collect();
    assert_eq!(lines, [(ZED, false, "3-kill streak: big shutdown"), (GAREN, true, "3 deaths in a row: worth little")]);
    assert!(a.calls.iter().all(|c| !c.key.starts_with("me-streak")));

    // My own streak is a call to play safe.
    let ev = vec![kill(100.0, 0, 5), kill(200.0, 0, 6), kill(300.0, 0, 7)];
    let a = advise(&snap(lobby(STANDARD), ev));
    assert_eq!(a.bounties[0].text, "You: 3-kill streak: big shutdown, don't die alone");
    assert!(a.calls.iter().any(|c| c.key == "me-streak-3"), "{:?}", a.calls);
}

#[test]
fn objective_calls_from_timers_gold_and_death_timers() {
    // 18:50: Baron in 1:10, I hold 1500 gold; dragon is up and three enemies are dead.
    let mut p = lobby(STANDARD);
    for e in &mut p[5..8] {
        e.dead = true;
        e.respawn_in = 30.0;
    }
    let s = GameSnapshot { time: 1130.0, my_gold: 1500.0, ..snap(p, vec![]) };
    let a = advise(&s);
    let texts: Vec<&str> = a.calls.iter().map(|c| c.text.as_str()).collect();
    assert!(texts.contains(&"Baron in 1:10: back now, spend 1500 gold, and be there"), "{texts:?}");
    assert_eq!(a.calls.iter().find(|c| c.key == "back-Baron-1200").expect("back call").say, "Baron in 70 seconds. Back and buy now.");
    assert!(texts.contains(&"Dragon is up: Darius (30 s), Lee Sin (30 s) and Zed (30 s) dead, their jungler can't smite. Take it"), "{texts:?}");
    let r = a.readiness.unwrap();
    assert_eq!(r.verdict, Verdict::Fight);
    assert!(r.reasons.iter().any(|x| x.starts_with("3 enemies dead")), "{:?}", r.reasons);

    // Dead myself: no back call; low gold: none either.
    let mut s2 = s.clone();
    s2.players[0].dead = true;
    s2.players[0].respawn_in = 20.0;
    assert!(advise(&s2).calls.iter().all(|c| !c.key.starts_with("back-")));
    let s3 = GameSnapshot { my_gold: 300.0, ..s.clone() };
    assert!(advise(&s3).calls.iter().all(|c| !c.key.starts_with("back-")));
}

#[test]
fn supports_and_junglers_ward_an_objective_before_it_spawns() {
    // 18:50: Baron in 1:10; Dragon is already up (no ward call for it).
    let mut p = lobby(STANDARD);
    p[0].position = Some(Lane::Support);
    let s = GameSnapshot { time: 1130.0, ..snap(p, vec![]) };
    let a = advise(&s);
    let ward: Vec<&Call> = a.calls.iter().filter(|c| c.key.starts_with("ward-")).collect();
    assert_eq!(ward.len(), 1, "{:?}", a.calls);
    assert_eq!(ward[0].key, "ward-Baron-1200");
    assert_eq!(ward[0].text, "Baron in 1:10: ward the pit and the river bushes, bring a Control Ward");
    assert_eq!(ward[0].say, "Baron in 70 seconds. Ward the river.");

    // Spoken once anywhere in the window, so it says the time left, not "one minute": a real game's
    // dragon (killed 10:45.6, back 15:45.6) was called "in one minute" at 14:35, 70 s out.
    let dragon = vec![GameEvent::Dragon { time: 645.6, team: Some(Team::Order), kind: "Air".into() }];
    let s6 = GameSnapshot { time: 875.2, events: dragon, ..s.clone() };
    let a6 = advise(&s6);
    let c = a6.calls.iter().find(|c| c.key == "ward-Dragon-945").expect("ward call");
    assert_eq!(c.say, "Dragon in 70 seconds. Ward the river.");
    let s7 = GameSnapshot { time: 1163.0, ..s.clone() };
    let a7 = advise(&s7);
    assert_eq!(a7.calls.iter().find(|c| c.key == "ward-Baron-1200").expect("ward call").say, "Baron in 35 seconds. Ward the river.");

    // Holding a Control Ward and the lens: sweep instead.
    let mut s2 = s.clone();
    s2.players[0].items = vec![2055, 3364];
    let a2 = advise(&s2);
    let c = a2.calls.iter().find(|c| c.key == "ward-Baron-1200").expect("ward call");
    assert!(c.text.ends_with(", sweep with Oracle"), "{}", c.text);

    // Mid laners and the dead get none; too early neither.
    let mut s3 = s.clone();
    s3.players[0].position = Some(Lane::Middle);
    assert!(advise(&s3).calls.iter().all(|c| !c.key.starts_with("ward-")));
    let mut s4 = s.clone();
    s4.players[0].dead = true;
    s4.players[0].respawn_in = 10.0;
    assert!(advise(&s4).calls.iter().all(|c| !c.key.starts_with("ward-")));
    let s5 = GameSnapshot { time: 1100.0, ..s.clone() };
    assert!(advise(&s5).calls.iter().all(|c| !c.key.starts_with("ward-")));
}

#[test]
fn readiness_turns_on_item_gold_and_waits_for_the_first_minutes() {
    let mut p = lobby(STANDARD);
    for e in &mut p[5..10] {
        e.items = vec![ZHONYAS];
    }
    let a = advise(&snap(p.clone(), vec![]));
    let r = a.readiness.unwrap();
    assert_eq!(r.verdict, Verdict::Avoid);
    assert!(r.reasons[0].starts_with("−") && r.reasons[0].ends_with("item gold"), "{:?}", r.reasons);
    let early = GameSnapshot { time: 120.0, ..snap(p, vec![]) };
    assert!(advise(&early).readiness.is_none());
}

#[test]
fn a_take_call_keeps_its_key_while_the_objective_is_up() {
    // Baron up at 22:00; Darius, Lee Sin and Zed died at 21:40-21:45. The key must not
    // follow the clock, or the call is spoken again every tick.
    let mut p = lobby(STANDARD);
    for e in &mut p[5..8] {
        e.dead = true;
        e.respawn_in = 30.0;
    }
    let ev = vec![kill(1300.0, 0, 5), kill(1302.0, 1, 6), kill(1305.0, 2, 7)];
    let take = |time: f32| {
        let a = advise(&GameSnapshot { time, ..snap(p.clone(), ev.clone()) });
        a.calls.iter().filter(|c| c.key.starts_with("take-Baron")).map(|c| c.key.clone()).collect::<Vec<_>>()
    };
    let (first, later) = (take(1320.0), take(1326.0));
    assert_eq!(first, ["take-Baron-1300"]);
    assert_eq!(later, first);
}

#[test]
fn a_dead_jungler_is_enough_for_dragon_and_respawning_enemies_are_named() {
    // Only Lee Sin, their jungler, is dead: take Dragon (no smite to contest it).
    let mut p = lobby(STANDARD);
    p[6].dead = true;
    p[6].respawn_in = 40.0;
    let a = advise(&GameSnapshot { time: 1130.0, ..snap(p.clone(), vec![]) });
    let c = a.calls.iter().find(|c| c.key.starts_with("take-Dragon")).expect("take Dragon");
    assert_eq!(c.text, "Dragon is up: Lee Sin (40 s) dead, their jungler can't smite. Take it");
    assert_eq!(c.say, "Their jungler is dead. Take Dragon.");

    // Darius and Zed long dead, Lee Sin back in 7 s: two counted, Lee Sin named.
    p[6].respawn_in = 7.0;
    for i in [5, 7] {
        p[i].dead = true;
        p[i].respawn_in = 30.0;
    }
    let a = advise(&GameSnapshot { time: 1130.0, ..snap(p, vec![]) });
    let c = a.calls.iter().find(|c| c.key.starts_with("take-Dragon")).expect("take Dragon");
    assert_eq!(c.text, "Dragon is up: Darius (30 s) and Zed (30 s) dead, Lee Sin back in 7 s. Take it");
}

#[test]
fn baron_needs_its_dead_to_stay_dead_longer() {
    // Darius 26 s and Zed 13 s: enough for Dragon, but Zed is back before Baron falls.
    let mut p = lobby(STANDARD);
    for (i, s) in [(5, 26.0), (7, 13.0)] {
        p[i].dead = true;
        p[i].respawn_in = s;
    }
    let keys = |p: &[Player], time: f32| advise(&GameSnapshot { time, ..snap(p.to_vec(), vec![]) }).calls.into_iter().map(|c| c.key).collect::<Vec<_>>();
    assert!(keys(&p, 1130.0).iter().any(|k| k.starts_with("take-Dragon")));
    assert!(!keys(&p, 1320.0).iter().any(|k| k.starts_with("take-Baron")));
    p[7].respawn_in = 22.0;
    assert!(keys(&p, 1320.0).iter().any(|k| k.starts_with("take-Baron")));
}
