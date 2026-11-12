mod common;

use coach::{GameInput, GameLog, GankEnemies, GankTracker, SightLog, death_review, game_advice};
use common::*;
use model::*;

const BLUE_SUPPORT: &str = include_str!("../../../fixtures/coach/sight_blue_support.json");

#[derive(serde::Deserialize)]
struct Game {
    blue: bool,
    me: usize,
    players: Vec<(ChampKey, Team, Option<Lane>)>,
    kills: Vec<(f32, Option<usize>, usize, Vec<usize>)>,
    reads: Vec<(f32, Vec<(i32, i32, Option<ChampKey>)>)>,
}

fn roster(sd: &mut StaticData, names: &[(ChampKey, &str)]) {
    for &(key, name) in names {
        let c = Champion { key, name: name.into(), ..sd.champs[&ZED].clone() };
        sd.champs.insert(key, c);
    }
}

fn replay(json: &str, sd: &StaticData, until: f32) -> DeathReview {
    let g: Game = serde_json::from_str(json).unwrap();
    let players: Vec<Player> = g.players.iter().map(|&(c, t, l)| player(c, t, l)).collect();
    let events: Vec<GameEvent> = g.kills.iter().map(|(time, killer, victim, assisters)| GameEvent::Kill { time: *time, killer: *killer, victim: *victim, assisters: assisters.clone() }).collect();
    let mine = players[g.me].team;
    let foes: Vec<&Player> = players.iter().filter(|p| p.team != mine).collect();
    let enemies = GankEnemies {
        names: foes.iter().map(|p| (p.champ, sd.champ_name(p.champ).to_string())).collect(),
        jungler: foes.iter().find(|p| p.position == Some(Lane::Jungle)).map(|p| p.champ),
        lanes: foes.iter().filter_map(|p| Some((p.champ, p.position?))).collect(),
    };
    let lane = players[g.me].position;
    let (mut tracker, mut sights, mut log) = (GankTracker::default(), SightLog::default(), GameLog::default());
    let mut clock = g.reads.first().map_or(0.0, |r| r.0 - 1.0);
    let tick = |log: &mut GameLog, sights: &SightLog, time: f32| {
        let ev: Vec<GameEvent> = events.iter().filter(|e| matches!(e, GameEvent::Kill { time: t, .. } if *t <= time)).cloned().collect();
        let snap = GameSnapshot { time, me: g.me, players: players.clone(), events: ev, ..snap(vec![], vec![]) };
        let advice = game_advice(&GameInput { snap: &snap, sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: Some(&coach::winprob::Model::default()), situations: &Default::default() });
        log.observe(&snap, &advice, sd, Some(sights));
    };
    for (time, blips) in &g.reads {
        while clock <= *time {
            tick(&mut log, &sights, clock);
            clock += 2.0;
        }
        tracker.observe(blips, g.blue, *time);
        let calls = tracker.warnings(*time, lane, &enemies);
        let named: Vec<(i32, i32, Option<ChampKey>)> = blips.iter().zip(tracker.last_names()).map(|(&(x, y, _), c)| (x, y, c)).collect();
        sights.push(*time, &named, g.blue, &calls);
    }
    while clock <= until {
        tick(&mut log, &sights, clock);
        clock += 2.0;
    }
    death_review(&log, sd)
}

#[test]
fn support_deaths_to_wukong_were_seen_coming() {
    let mut sd = sd();
    roster(&mut sd, &[(875, "Sett"), (120, "Hecarim"), (777, "Yone"), (29, "Twitch"), (412, "Thresh"), (24, "Jax"), (62, "Wukong"), (112, "Viktor"), (15, "Sivir"), (26, "Zilean")]);
    let r = replay(BLUE_SUPPORT, &sd, 1010.0);
    for d in &r.deaths {
        println!("{:>4} {:?}", d.time as u32, d.sight);
    }
    let sight = |i: usize| r.deaths[i].sight.as_ref().unwrap();
    assert_eq!(r.deaths.len(), 3);
    assert_eq!(sight(0).verdict, Some(SightKind::Warned));
    assert!(sight(0).line.as_deref().unwrap().starts_with("Warned 7 s before: Wukong"), "{:?}", sight(0).line);
    assert_eq!(sight(0).seen[0].champ, Some(62));
    assert_eq!(sight(1).verdict, Some(SightKind::Visible));
    assert_eq!(sight(1).line.as_deref(), Some("Wukong was on the minimap for 11 s before"));
    assert!(r.deaths[2].sight.is_none(), "the minimap was not read then");
    assert!(r.notes.contains(&"2 of 2 deaths: the killer was on the minimap 4 s or more before".to_string()), "{:?}", r.notes);
}
