//! Next skill point.

mod common;

use coach::skills::next;
use coach::{GameInput, game_advice};
use common::*;
use model::*;

const THRESH: &str = "QEWQQRQEQEREEWW";

fn at(order: &str, have: [u32; 4], level: u32) -> (String, u32, u32) {
    let n = next(order, have, level).expect("a point");
    (n.key, n.unspent, n.at_level)
}

#[test]
fn on_the_order_the_next_level_is_shown() {
    assert_eq!(at(THRESH, [3, 1, 1, 0], 5), ("R".into(), 0, 6));
    assert_eq!(at(THRESH, [2, 1, 1, 0], 4), ("Q".into(), 0, 5));
    assert_eq!(at(THRESH, [0, 0, 0, 0], 1), ("Q".into(), 1, 1));
    assert_eq!(next(THRESH, [3, 1, 1, 0], 5).unwrap().priority, "QEW");
}

#[test]
fn one_point_behind_asks_for_it_now() {
    assert_eq!(at(THRESH, [2, 1, 1, 0], 5), ("Q".into(), 1, 5));
    assert_eq!(at(THRESH, [3, 1, 1, 0], 6), ("R".into(), 1, 6));
}

#[test]
fn levelled_out_of_order_asks_for_the_earliest_point_missing() {
    assert_eq!(at(THRESH, [1, 1, 2, 0], 4), ("Q".into(), 0, 5));
    assert_eq!(at(THRESH, [1, 2, 0, 0], 3), ("E".into(), 0, 4));
    assert_eq!(at(THRESH, [5, 0, 1, 0], 7), ("W".into(), 1, 3));
}

#[test]
fn two_unspent_points_name_the_first() {
    assert_eq!(at(THRESH, [1, 0, 0, 0], 3), ("E".into(), 2, 2));
}

#[test]
fn past_the_order_r_then_the_ability_with_points_left() {
    assert_eq!(at(THRESH, [5, 3, 5, 2], 15), ("R".into(), 0, 16));
    assert_eq!(at(THRESH, [5, 3, 5, 2], 16), ("R".into(), 1, 16));
    assert_eq!(at(THRESH, [5, 3, 5, 3], 16), ("W".into(), 0, 17));
    assert_eq!(at(THRESH, [5, 4, 5, 3], 18), ("W".into(), 1, 18));
    assert!(next(THRESH, [5, 5, 5, 3], 18).is_none(), "everything maxed");
    assert_eq!(at("QEWQQR", [3, 1, 1, 1], 7), ("Q".into(), 1, 7), "short order: R waits for 11");
}

#[test]
fn no_row_the_order_cannot_describe() {
    assert!(next(THRESH, [2, 1, 1, 1], 4).is_none(), "a free rank: more ranks than levels");
    assert!(next("", [1, 0, 0, 0], 1).is_none());
    assert!(next("QEWQQX", [1, 0, 0, 0], 1).is_none());
    assert!(next(THRESH, [0, 0, 0, 1], 6).is_none(), "no points to place");
    assert!(next(THRESH, [0, 0, 0, 0], 0).is_none());
}

fn ahri_snap(level: u32, have: Option<[u32; 4]>) -> GameSnapshot {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().lvl(level);
    GameSnapshot { my_abilities: have, ..snap(p, vec![]) }
}

fn vs_zed(games: u32) -> MatchupBuild {
    MatchupBuild {
        vs: ZED,
        games: 2_000,
        win_rate: 50.0,
        start: vec![],
        start_games: 0,
        start_wr: 0.0,
        core: vec![],
        runes: vec![],
        rune_page: None,
        summoners: vec![],
        skills: "QEW".into(),
        skill_order: "QEWQQRQEQEREEWW".into(),
        skill_order_games: games,
    }
}

fn skill(s: &GameSnapshot, stats: Option<&ChampStats>, matchup: Option<&MatchupBuild>) -> Option<SkillNext> {
    let sd = sd();
    game_advice(&GameInput { snap: s, sd: &sd, stats, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup }).skill
}

#[test]
fn matchup_order_with_enough_games_else_the_build_page() {
    let stats = ahri_stats();
    let s = ahri_snap(2, Some([1, 1, 0, 0]));
    let n = skill(&s, Some(&stats), Some(&vs_zed(1_200))).unwrap();
    assert_eq!((n.key.as_str(), n.priority.as_str(), n.source.as_str()), ("E", "QEW", "vs Zed, 1.2k games"));
    let n = skill(&s, Some(&stats), Some(&vs_zed(150))).unwrap();
    assert_eq!((n.key.as_str(), n.unspent, n.at_level, n.priority.as_str(), n.source.as_str()), ("E", 0, 3, "QWE", "lolalytics 16.21"));
    assert!(skill(&s, None, Some(&vs_zed(150))).is_none());
    assert!(skill(&ahri_snap(2, None), Some(&stats), None).is_none(), "no ranks from the API");
    let bare = ChampStats { skill_order: String::new(), ..stats };
    assert!(skill(&s, Some(&bare), None).is_none(), "no order on the page");
}
