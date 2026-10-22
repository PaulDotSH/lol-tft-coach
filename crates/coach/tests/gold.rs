//! Item-gold diff and its history.

mod common;

use coach::{GameInput, GoldHistory, HISTORY_MAX, SAMPLE_STEP, game_advice};
use common::*;
use model::*;

fn advise_with(snap: &GameSnapshot, stats: Option<&ChampStats>, history: &[GoldSample]) -> GameAdvice {
    let sd = sd();
    game_advice(&GameInput { snap, sd: &sd, stats, augments: None, offered: &[], gold_history: history })
}

fn advise(snap: &GameSnapshot) -> GameAdvice {
    advise_with(snap, None, &[])
}

/// Standard lobby; my team has a Ruby each (400), enemies a Ruby, Zed adds two items.
fn rich_zed() -> GameSnapshot {
    let mut p = lobby(STANDARD);
    p[7] = p[7].clone().kda(6, 1, 2).lvl(11).items(&[RUBY, YOUMUUS, LONG_SWORD]);
    p[0] = p[0].clone().lvl(8);
    snap(p, vec![])
}

fn sample(time: f32, team: i32) -> GoldSample {
    GoldSample { time, team, lane: None }
}

#[test]
fn team_totals_kills_and_levels() {
    let g = advise(&rich_zed()).gold;
    assert_eq!(g.team_item_gold, (5 * 400, 5 * 400 + 2800 + 350));
    assert_eq!(g.diff, -3150);
    assert_eq!(g.kills, (10, 8 + 6));
    // Levels: mine 8+9·4 = 44, theirs 9·4+11 = 47 → −0.6 average.
    assert!((g.level_diff + 0.6).abs() < 1e-4, "{}", g.level_diff);
}

#[test]
fn rift_lanes_pair_by_position_in_lane_order() {
    let g = advise(&rich_zed()).gold;
    let rows: Vec<_> = g.lanes.iter().map(|l| (l.lane, l.ally, l.enemy)).collect();
    assert_eq!(
        rows,
        vec![
            (Some(Lane::Top), GAREN, DARIUS),
            (Some(Lane::Jungle), JARVAN, LEE_SIN),
            (Some(Lane::Middle), AHRI, ZED),
            (Some(Lane::Bottom), EZREAL, JINX),
            (Some(Lane::Support), SONA, THRESH),
        ]
    );
    let mid = &g.lanes[2];
    assert!(mid.mine && g.lanes.iter().filter(|l| l.mine).count() == 1);
    assert_eq!((mid.gold_diff, mid.level_diff), (-3150, -3));
}

#[test]
fn my_lane_row_matches_self_stats() {
    let a = advise(&rich_zed());
    let mine = a.gold.lanes.iter().find(|l| l.mine).unwrap();
    assert_eq!(Some(mine.gold_diff), a.me.item_gold_vs_opp);
    assert_eq!(a.gold.history.last().unwrap().lane, a.me.item_gold_vs_opp);
}

#[test]
fn missing_positions_pair_in_scoreboard_order() {
    let mut s = rich_zed();
    for p in &mut s.players {
        p.position = None;
    }
    let g = advise(&s).gold;
    let rows: Vec<_> = g.lanes.iter().map(|l| (l.lane, l.ally, l.enemy)).collect();
    assert_eq!(
        rows,
        vec![(None, AHRI, DARIUS), (None, GAREN, LEE_SIN), (None, JARVAN, ZED), (None, EZREAL, JINX), (None, SONA, THRESH)]
    );
    assert_eq!(g.diff, -3150, "team totals don't depend on positions");
}

#[test]
fn missing_enemy_position_uses_my_lane_opponent_fallback() {
    // Zed has no position; lolalytics says Zed is a mid laner, so he is my lane opponent.
    let mut s = rich_zed();
    s.players[7].position = None;
    let a = advise_with(&s, Some(&ahri_stats()), &[]);
    let mine = a.gold.lanes.iter().find(|l| l.mine).unwrap();
    assert_eq!((mine.lane, mine.enemy), (Some(Lane::Middle), ZED));
    assert_eq!(a.gold.lanes.len(), 5);
    assert_eq!(a.gold.lanes.iter().map(|l| l.lane).collect::<Vec<_>>()[..3], [Some(Lane::Top), Some(Lane::Jungle), Some(Lane::Middle)]);
}

#[test]
fn partial_positions_known_lanes_first_rest_last() {
    let mut s = rich_zed();
    s.players[1].position = None; // Garen
    s.players[8].position = None; // Jinx
    let g = advise(&s).gold;
    let rows: Vec<_> = g.lanes.iter().map(|l| (l.lane, l.ally, l.enemy)).collect();
    assert_eq!(rows[..3], [(Some(Lane::Jungle), JARVAN, LEE_SIN), (Some(Lane::Middle), AHRI, ZED), (Some(Lane::Support), SONA, THRESH)]);
    // Leftovers: allies Garen, Ezreal vs enemies Darius, Jinx in scoreboard order.
    assert_eq!(rows[3..], [(None, GAREN, DARIUS), (None, EZREAL, JINX)]);
}

#[test]
fn aram_has_no_lanes_and_lists_players_by_team_then_gold() {
    let mut s = rich_zed();
    s.mode = Mode::Aram;
    s.map = 12;
    for p in &mut s.players {
        p.position = None;
    }
    s.players[3].items = vec![LUDENS];
    let g = advise(&s).gold;
    assert!(g.lanes.is_empty());
    assert_eq!(g.players.len(), 10);
    assert!(g.players[..5].iter().all(|p| p.ally) && g.players[5..].iter().all(|p| !p.ally));
    assert_eq!((g.players[0].champ, g.players[0].item_gold), (EZREAL, 2900));
    assert_eq!((g.players[5].champ, g.players[5].item_gold), (ZED, 3550));
    assert_eq!(g.players.iter().filter(|p| p.mine).map(|p| p.champ).collect::<Vec<_>>(), [AHRI]);
    assert_eq!(g.history.last().unwrap().lane, None);
}

#[test]
fn consumables_are_not_item_gold() {
    let mut sd = sd();
    let pot = Item {
        id: 2003,
        name: "Health Potion".into(),
        cost: 50,
        from: vec![],
        into: vec![],
        stats: ItemStats::default(),
        answers: Profile::default(),
        boots: false,
        completed: false,
        rift: true,
        aram: true,
        tags: vec!["Consumable".into()],
    };
    sd.items.insert(pot.id, pot);
    let mut s = rich_zed();
    s.players[0].items = vec![RUBY, 2003, 2003];
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[] });
    assert_eq!(a.gold.team_item_gold.0, 2000);
}

#[test]
fn history_is_passed_through_and_ends_now() {
    let s = rich_zed();
    let past = [sample(0.0, 0), sample(20.0, -100)];
    let g = advise_with(&s, None, &past).gold;
    assert_eq!(g.history.len(), 3);
    assert_eq!(g.history[..2], past);
    assert_eq!(g.history[2], GoldSample { time: 900.0, team: -3150, lane: Some(-3150) });
}

#[test]
fn history_samples_every_step() {
    let mut h = GoldHistory::default();
    for t in (0..=120).step_by(2) {
        h.push(sample(t as f32, t));
    }
    let times: Vec<f32> = h.samples.iter().map(|s| s.time).collect();
    assert_eq!(times, [0.0, 20.0, 40.0, 60.0, 80.0, 100.0, 120.0]);
}

#[test]
fn history_stays_bounded_and_covers_the_game() {
    let mut h = GoldHistory::default();
    for t in 0..=(3 * 3600) {
        h.push(sample(t as f32, 0));
        assert!(h.samples.len() <= HISTORY_MAX);
    }
    let s = &h.samples;
    assert_eq!(s[0].time, 0.0);
    assert!(s.last().unwrap().time > 3.0 * 3600.0 - 4.0 * 8.0 * SAMPLE_STEP, "{}", s.last().unwrap().time);
    // Evenly spaced after halving.
    let gaps: Vec<f32> = s.windows(2).map(|w| w[1].time - w[0].time).collect();
    assert!(gaps.iter().all(|&g| g == gaps[0]), "{gaps:?}");
}

#[test]
fn history_resets_when_the_clock_goes_back() {
    let mut h = GoldHistory::default();
    for t in [0.0, 20.0, 40.0, 60.0] {
        h.push(sample(t, 1));
    }
    h.push(sample(5.0, 2));
    assert_eq!(h.samples, [sample(5.0, 2)]);
}
