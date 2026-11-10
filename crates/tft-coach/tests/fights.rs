//! Which player a fight was against: plates at any moment of it, the player list's HP after it,
//! dead players and ghost fights.

mod common;
use common::*;
use tft_coach::fights::{Fights, expected_drop};
use tft_model::*;

fn red(x: f32) -> SeenUnit {
    SeenUnit { x, y: 200.0, star: 1, bench: None, full: true, items: vec![], emb: vec![] }
}

fn green(x: f32) -> SeenUnit {
    SeenUnit { x, y: 700.0, star: 1, bench: None, full: true, items: vec![], emb: vec![] }
}

fn lobby(rows: &[(&str, u32)]) -> Vec<(String, u32)> {
    rows.iter().map(|(n, h)| (n.to_string(), *h)).collect()
}

fn planning(stage: (u32, u32), me: u32, rows: &[(&str, u32)]) -> TftSnapshot {
    TftSnapshot { hp: Some(me), lobby: lobby(rows), ..snap(stage, 30) }
}

fn fight(stage: (u32, u32), me: u32, rows: &[(&str, u32)], plate: Option<&str>, survivors: usize) -> TftSnapshot {
    TftSnapshot { hp: Some(me), lobby: lobby(rows), viewing: plate.map(str::to_string), foes: vec![red(900.0), red(1000.0)], enemies: 2, units: (0..survivors).map(|i| green(800.0 + 60.0 * i as f32)).collect(), ..snap(stage, 30) }
}

fn fights() -> Fights {
    Fights::default().single_reads()
}

const FOUR: [(&str, u32); 3] = [("Ann", 80), ("Bob", 70), ("Cid", 60)];

#[test]
fn a_plate_read_late_in_the_fight_links_the_whole_fight() {
    let mut f = fights();
    f.observe(&planning((3, 2), 90, &FOUR));
    f.observe(&fight((3, 2), 90, &FOUR, None, 3));
    f.observe(&fight((3, 2), 90, &FOUR, None, 3));
    assert_eq!(f.current().unwrap().opponent, None);
    f.observe(&fight((3, 2), 90, &FOUR, Some("Bob"), 2));
    assert_eq!(f.current().unwrap().opponent, Some(1));
    assert_eq!(f.current().unwrap().candidates, vec![1]);
    f.observe(&planning((3, 3), 90, &[("Ann", 80), ("Bob", 64), ("Cid", 55)]));
    assert_eq!(f.done()[0].opponent, Some(1));
    assert_eq!(f.players()[1], "Bob");
}

#[test]
fn two_plates_in_the_fight_link_nothing() {
    let mut f = fights();
    f.observe(&fight((3, 2), 90, &FOUR, Some("Ann"), 2));
    f.observe(&fight((3, 2), 90, &FOUR, Some("Bob"), 2));
    assert_eq!(f.current().unwrap().opponent, None);
    assert_eq!(f.current().unwrap().candidates, vec![0, 1, 2]);
}

#[test]
fn after_a_win_my_opponent_is_the_one_who_dropped_by_the_expected_amount() {
    let (lo, hi) = expected_drop(3, 2);
    assert!((lo..=hi).contains(&8) && hi < 30);
    let mut f = fights();
    let five = [("Ann", 80), ("Bob", 70), ("Cid", 60), ("Dee", 50), ("Eve", 40)];
    f.observe(&planning((3, 2), 90, &five));
    f.observe(&fight((3, 2), 90, &five, None, 2));
    f.observe(&planning((3, 3), 90, &[("Ann", 80), ("Bob", 62), ("Cid", 30), ("Dee", 50), ("Eve", 40)]));
    assert_eq!(f.done(), &[tft_coach::fights::Fight { round: (3, 2), candidates: vec![1], opponent: None }]);
}

#[test]
fn after_a_loss_my_opponent_is_one_whose_hp_held() {
    let mut f = fights();
    f.observe(&planning((4, 1), 70, &FOUR));
    f.observe(&fight((4, 1), 70, &FOUR, None, 0));
    f.observe(&planning((4, 2), 61, &[("Ann", 80), ("Bob", 59), ("Cid", 60)]));
    assert_eq!(f.done()[0].candidates, vec![0, 2]);
}

#[test]
fn a_plate_the_hp_rows_contradict_is_dropped() {
    let mut f = fights();
    f.observe(&planning((4, 1), 70, &FOUR));
    f.observe(&fight((4, 1), 70, &FOUR, Some("Bob"), 0));
    f.observe(&planning((4, 2), 61, &[("Ann", 80), ("Bob", 59), ("Cid", 60)]));
    assert_eq!((f.done()[0].opponent, f.done()[0].candidates.clone()), (None, vec![0, 2]));
}

#[test]
fn dead_players_are_never_candidates() {
    let mut f = fights();
    let rows = [("Ann", 80), ("Bob", 0), ("Cid", 60), ("Dee", 30)];
    f.observe(&planning((5, 1), 70, &rows));
    f.observe(&fight((5, 1), 70, &rows, None, 0));
    assert_eq!(f.current().unwrap().candidates, vec![0, 2, 3]);
    f.observe(&planning((5, 2), 60, &[("Ann", 80), ("Bob", 0), ("Cid", 60), ("Dee", 21)]));
    assert_eq!(f.done()[0].candidates, vec![0, 2]);
}

#[test]
fn a_ghost_fight_is_skipped() {
    let mut f = fights();
    let rows = [("Ann", 80), ("Bob", 70)];
    f.observe(&planning((5, 1), 70, &rows));
    f.observe(&fight((5, 1), 70, &rows, None, 3));
    f.observe(&planning((5, 2), 70, &rows));
    assert!(f.done().is_empty());
    f.observe(&fight((5, 2), 70, &rows, None, 3));
    f.observe(&planning((5, 3), 70, &[("Ann", 80), ("Bob", 58)]));
    assert_eq!(f.done()[0].round, (5, 2));
}

#[test]
fn pve_and_carousel_rounds_are_no_fights() {
    let mut f = fights();
    f.observe(&fight((2, 7), 90, &FOUR, Some("Ann"), 2));
    f.observe(&fight((3, 4), 90, &FOUR, Some("Ann"), 2));
    assert!(f.current().is_none());
}

#[test]
fn one_misread_hp_does_not_count_live() {
    let mut f = Fights::default();
    f.observe(&planning((3, 2), 90, &FOUR));
    f.observe(&planning((3, 2), 90, &FOUR));
    f.observe(&fight((3, 2), 90, &FOUR, None, 2));
    f.observe(&planning((3, 3), 90, &[("Ann", 8), ("Bob", 62), ("Cid", 60)]));
    f.observe(&planning((3, 3), 90, &[("Ann", 80), ("Bob", 62), ("Cid", 60)]));
    assert_eq!(f.done()[0].candidates, vec![1]);
}
