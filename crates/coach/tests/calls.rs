//! Live calls: shutdown bounties, fight readiness, objective calls.

mod common;

use coach::{GameInput, LiveInput, Tracker, game_advice, live_extras};
use common::*;
use model::*;
use std::collections::HashMap;

fn advise(s: &GameSnapshot) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
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
    assert!(texts.contains(&"Dragon is up and 3 enemies are dead: take it"), "{texts:?}");
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
