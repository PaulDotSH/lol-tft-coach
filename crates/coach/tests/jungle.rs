//! Jungler mode: clear pace, the lane board, the objective back tip, the laner tips left out, and the jungle review.

mod common;

use coach::{GameInput, LiveInput, MapView, Seen, Tracker, game_advice, jungle, live_extras};
use common::*;
use model::*;
use std::collections::HashMap;

const DARIUS_I: usize = 5;
const LEE_I: usize = 6;
const JINX_I: usize = 8;

/// Me = Ahri jungling (Smite) at index 0; Garen top, Jarvan mid, Ezreal and Sona bot; Lee Sin their jungler.
fn jungle_lobby() -> Vec<Player> {
    let mut p = lobby(STANDARD);
    p[0].position = Some(Lane::Jungle);
    p[0].smite = true;
    p[2].position = Some(Lane::Middle);
    p[LEE_I].smite = true;
    p
}

fn advise(s: &GameSnapshot, stats: Option<&ChampStats>, tracker: &Tracker) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: Some(&coach::winprob::Model::default()), situations: &Default::default() });
    live_extras(&mut a, &LiveInput { snap: s, sd: &sd, tracker, ults: &HashMap::new(), notes: &LaneNotes::default(), spells: &HashMap::new(), memory: &[] });
    a
}

/// My levels as tracked: (game time, level) reads, then the advice at `now` with my last level.
fn paced(reads: &[(f32, u32)], now: f32, swiftplay: bool) -> Option<String> {
    let sd = sd();
    let mut p = jungle_lobby();
    let mut t = Tracker::default();
    for &(time, level) in reads {
        p[0].level = level;
        t.observe(&GameSnapshot { time, ..snap(p.clone(), vec![]) }, &sd);
    }
    let s = GameSnapshot { time: now, swiftplay, ..snap(p, vec![]) };
    t.observe(&s, &sd);
    advise(&s, None, &t).jungle.unwrap().pace
}

#[test]
fn level_four_by_the_crabs_is_on_pace() {
    assert_eq!(paced(&[(60.0, 1), (100.0, 2), (140.0, 3), (202.0, 4)], 205.0, false).as_deref(), Some("Level 4 at 3:22: on pace"));
    assert_eq!(paced(&[(60.0, 1), (100.0, 2), (140.0, 3)], 150.0, false).as_deref(), Some("Level 3 at 2:20: on pace"));
}

#[test]
fn a_late_level_is_a_camp_behind() {
    assert_eq!(paced(&[(60.0, 1), (100.0, 2), (150.0, 3)], 210.0, false).as_deref(), Some("Level 3 at 3:30: one camp behind"));
    assert_eq!(paced(&[(60.0, 1), (100.0, 2)], 160.0, false).as_deref(), Some("Level 2 at 2:40: one camp behind"));
    assert_eq!(paced(&[(60.0, 1), (100.0, 2), (160.0, 3), (225.0, 4)], 230.0, false).as_deref(), Some("Level 4 at 3:45: one camp behind"));
}

#[test]
fn nothing_to_say_before_the_first_mark() {
    assert_eq!(paced(&[(60.0, 1), (100.0, 2)], 120.0, false), None);
}

#[test]
fn swiftplay_hides_the_level_table() {
    assert_eq!(paced(&[(60.0, 1), (100.0, 2), (150.0, 3)], 210.0, true), None);
}

#[test]
fn after_four_minutes_cs_against_the_benchmark() {
    let mut p = jungle_lobby();
    p[0].cs = 50;
    let s = GameSnapshot { time: 600.0, ..snap(p, vec![]) };
    let a = advise(&s, Some(&ahri_stats()), &Tracker::default());
    assert_eq!(a.jungle.unwrap().pace.as_deref(), Some("CS 24 behind the average pace (5.0/min vs 7.4): take your camps between ganks"));
    let mut p = jungle_lobby();
    p[0].cs = 75;
    let s = GameSnapshot { time: 600.0, ..snap(p, vec![]) };
    assert_eq!(advise(&s, Some(&ahri_stats()), &Tracker::default()).jungle.unwrap().pace.as_deref(), Some("CS on pace (7.5/min vs 7.4)"));
}

#[test]
fn no_benchmark_no_cs_line() {
    let s = GameSnapshot { time: 600.0, ..snap(jungle_lobby(), vec![]) };
    assert_eq!(advise(&s, None, &Tracker::default()).jungle.unwrap().pace, None);
}

#[test]
fn laners_get_no_jungle_advice_and_junglers_no_laner_tips() {
    let mut p = lobby(STANDARD);
    p[0].cs = 30;
    let s = GameSnapshot { time: 600.0, ..snap(p, vec![]) };
    let a = advise(&s, Some(&ahri_stats()), &Tracker::default());
    assert!(a.jungle.is_none());
    assert!(a.tips.iter().any(|t| t.key == "cs"));

    let mut p = jungle_lobby();
    p[0].cs = 30;
    let s = GameSnapshot { time: 600.0, ..snap(p, vec![]) };
    let a = advise(&s, Some(&ahri_stats()), &Tracker::default());
    assert!(a.jungle.is_some());
    assert!(!a.tips.iter().any(|t| t.key == "cs"), "{:?}", a.tips);
    assert!(a.matchup.is_empty(), "no laning panel");
}

#[test]
fn the_dragon_tip_says_whether_my_gold_buys_an_item() {
    let tip = |gold: f32| {
        let s = GameSnapshot { time: 230.0, my_gold: gold, ..snap(jungle_lobby(), vec![]) };
        advise(&s, Some(&ahri_stats()), &Tracker::default()).tips.into_iter().find(|t| t.key == "jg-objective").map(|t| t.text)
    };
    let rich = tip(5000.0).unwrap();
    assert!(rich.starts_with("Dragon in 1:10: back first: 5,000 g buys "), "{rich}");
    let poor = tip(300.0).unwrap();
    assert!(poor.starts_with("Dragon in 1:10: no full item yet, ") && poor.contains(" g short of "), "{poor}");
    let s = GameSnapshot { time: 100.0, my_gold: 5000.0, ..snap(jungle_lobby(), vec![]) };
    assert!(!advise(&s, Some(&ahri_stats()), &Tracker::default()).tips.iter().any(|t| t.key == "jg-objective"), "too early");
}

const NOW: f32 = 400.0;

fn board(p: Vec<Player>) -> jungle::Board {
    let sd = sd();
    let s = GameSnapshot { time: NOW, ..snap(p, vec![]) };
    let a = advise(&s, None, &Tracker::default());
    jungle::board(&s, &sd, &a.gold).unwrap()
}

fn seen(v: &[(ChampKey, f32, Lane, bool)]) -> MapView {
    MapView { time: NOW, seen: v.iter().map(|&(c, ago, side, pushed)| (c, Seen { time: NOW - ago, side, pushed })).collect() }
}

/// Every enemy laner seen just now, at home.
fn all_seen() -> Vec<(ChampKey, f32, Lane, bool)> {
    vec![(DARIUS, 0.0, Lane::Top, false), (ZED, 0.0, Lane::Middle, false), (JINX, 0.0, Lane::Bottom, false), (THRESH, 0.0, Lane::Bottom, false)]
}

fn row(v: &[LaneOpen], lane: Lane) -> &LaneOpen {
    v.iter().find(|r| r.lane == lane).unwrap()
}

fn score(v: &[LaneOpen], lane: Lane) -> f32 {
    row(v, lane).score
}

/// Each lane's score change from `base` to `with`.
fn moved(base: &[LaneOpen], with: &[LaneOpen]) -> [f32; 3] {
    [Lane::Top, Lane::Middle, Lane::Bottom].map(|l| score(with, l) - score(base, l))
}

#[test]
fn no_blips_gives_the_static_board() {
    let b = board(jungle_lobby());
    let v = jungle::lanes(&b, None);
    assert_eq!(v.iter().map(|r| r.lane).collect::<Vec<_>>().len(), 3);
    assert!(v.windows(2).all(|w| w[0].score >= w[1].score), "ranked");
    assert_eq!(row(&v, Lane::Bottom).enemy, [JINX, THRESH]);
    assert!(v.iter().all(|r| !r.reasons.iter().any(|s| s.contains("not seen") || s.contains("pushed"))), "{v:?}");
    let s = GameSnapshot { time: NOW, ..snap(jungle_lobby(), vec![]) };
    let j = advise(&s, None, &Tracker::default()).jungle.unwrap();
    assert!(!j.minimap);
    assert_eq!(j.lanes, v);
}

#[test]
fn a_pushed_laner_opens_their_lane() {
    let b = board(jungle_lobby());
    let base = jungle::lanes(&b, Some(&seen(&all_seen())));
    let mut s = all_seen();
    s[0].3 = true;
    let with = jungle::lanes(&b, Some(&seen(&s)));
    assert_eq!(moved(&base, &with), [coach::consts::JG_W_PUSHED, 0.0, 0.0]);
    assert!(row(&with, Lane::Top).reasons.contains(&"Darius is pushed up".to_string()), "{with:?}");
    assert_eq!(with[0].lane, Lane::Top);
}

#[test]
fn a_laner_out_of_sight_lowers_their_lane() {
    let b = board(jungle_lobby());
    let base = jungle::lanes(&b, Some(&seen(&all_seen())));
    let mut s = all_seen();
    s[1].1 = 22.0;
    let with = jungle::lanes(&b, Some(&seen(&s)));
    assert_eq!(moved(&base, &with), [0.0, coach::consts::JG_W_UNSEEN, 0.0]);
    let mut s = all_seen();
    s[0].3 = true;
    s[0].1 = 22.0;
    let gone = jungle::lanes(&b, Some(&seen(&s)));
    assert!(!row(&gone, Lane::Top).reasons.iter().any(|r| r.contains("pushed")), "an old sighting is not pushed now");
    let none = jungle::lanes(&b, Some(&seen(&all_seen()[1..])));
    assert_eq!(moved(&base, &none), [coach::consts::JG_W_UNSEEN, 0.0, 0.0]);
}

#[test]
fn setup_on_my_laner_opens_the_lane() {
    let base = jungle::lanes(&board(jungle_lobby()), None);
    let mut p = jungle_lobby();
    p[1].champ = LEONA;
    let with = jungle::lanes(&board(p), None);
    assert_eq!(moved(&base, &with), [coach::consts::JG_W_CC, 0.0, 0.0]);
}

#[test]
fn an_enemy_without_a_dash_opens_the_lane() {
    let mut p = jungle_lobby();
    p[DARIUS_I].champ = AATROX;
    let base = jungle::lanes(&board(p), None);
    let with = jungle::lanes(&board(jungle_lobby()), None);
    assert_eq!(moved(&base, &with), [coach::consts::JG_W_NO_DASH, 0.0, 0.0]);
    let mut p = jungle_lobby();
    p[DARIUS_I].champ = AATROX;
    p[DARIUS_I].level = 12;
    let floor = jungle::lanes(&board(p), None);
    assert!(row(&floor, Lane::Top).reasons == ["nothing special"], "{floor:?}");
}

#[test]
fn an_enemy_without_flash_opens_the_lane() {
    let mut p = jungle_lobby();
    p[JINX_I].spells = vec!["SummonerFlash".into(), "SummonerHeal".into()];
    let base = jungle::lanes(&board(p.clone()), None);
    p[JINX_I].spells = vec!["SummonerHaste".into(), "SummonerHeal".into()];
    let with = jungle::lanes(&board(p), None);
    assert_eq!(moved(&base, &with), [0.0, 0.0, coach::consts::JG_W_NO_FLASH]);
    assert!(row(&with, Lane::Bottom).reasons.iter().any(|r| r == "Jinx has no Flash"), "{with:?}");
}

#[test]
fn lane_gold_and_levels_move_the_lane() {
    let base = jungle::lanes(&board(jungle_lobby()), None);
    let mut p = jungle_lobby();
    p[DARIUS_I].items = vec![RUBY, RANDUINS];
    let behind = jungle::lanes(&board(p), None);
    assert_eq!(moved(&base, &behind)[1..], [0.0, 0.0]);
    assert!(score(&behind, Lane::Top) < score(&base, Lane::Top));
    let mut p = jungle_lobby();
    p[1].level = 11;
    let up = jungle::lanes(&board(p), None);
    assert_eq!(moved(&base, &up), [2.0 * coach::consts::JG_W_LEVEL, 0.0, 0.0]);
    assert!(row(&up, Lane::Top).reasons.iter().any(|r| r == "your side is 11, theirs is 9"), "{up:?}");
}

#[test]
fn the_enemy_jungler_seen_lowers_their_side_and_frees_the_others() {
    let b = board(jungle_lobby());
    let base = jungle::lanes(&b, Some(&seen(&all_seen())));
    let mut s = all_seen();
    s.push((LEE_SIN, 5.0, Lane::Top, false));
    let with = jungle::lanes(&b, Some(&seen(&s)));
    use coach::consts::{JG_W_JUNGLER_AWAY, JG_W_JUNGLER_HERE};
    assert_eq!(moved(&base, &with), [JG_W_JUNGLER_HERE, JG_W_JUNGLER_AWAY, JG_W_JUNGLER_AWAY]);
    s.last_mut().unwrap().1 = 60.0;
    assert_eq!(moved(&base, &jungle::lanes(&b, Some(&seen(&s)))), [0.0; 3], "an old sighting says nothing");
}

#[test]
fn a_pushed_laner_with_their_jungler_near_ranks_below_one_without() {
    let b = board(jungle_lobby());
    let mut s = all_seen();
    s[0].3 = true;
    s[2].3 = true;
    s.push((LEE_SIN, 3.0, Lane::Top, false));
    let v = jungle::lanes(&b, Some(&seen(&s)));
    assert!(score(&v, Lane::Bottom) > score(&v, Lane::Top), "{v:?}");
    assert!(row(&v, Lane::Bottom).reasons.iter().any(|r| r == "Jinx is pushed up" || r == "Lee Sin was just seen top side"), "{v:?}");
}

fn read(name: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/../../fixtures/clients/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

/// The recorded flex game as its blue-side jungler (Diana); Viego jungled for red.
fn jungle_game() -> MatchAnalysis {
    let d = clients::history::parse_match(&read("match_game_jungle.json"), Some("me-puuid"), Some(&read("match_timeline_flex.json"))).unwrap();
    coach::analyze_match(&d, &sd(), None, None, coach::winprob::Model::shipped())
}

#[test]
fn the_review_has_a_jungle_section_instead_of_laning() {
    let a = jungle_game();
    assert!(a.laning.is_none());
    let j = a.jungle.as_ref().unwrap();
    println!("{j:#?}");
    assert_eq!(j.enemy, Some(234));
    assert_eq!(j.first_clear.iter().map(|c| c.minute).collect::<Vec<_>>(), [3, 4]);
    assert_eq!(j.clear.iter().map(|c| (c.from, c.to)).collect::<Vec<_>>(), [(0, 5), (5, 10), (10, 15)]);
    assert_eq!(j.versus.iter().map(|r| r.minute).collect::<Vec<_>>(), [5, 10, 15]);
    assert!(j.versus.iter().all(|r| r.opp_cs.is_some() && r.gold_lead.is_some()));
    let epic = a.marks.iter().filter(|m| m.kind != "Tower" && m.kind != "Inhibitor").count();
    assert_eq!(j.objectives.len(), epic);
}

#[test]
fn a_laner_review_keeps_laning() {
    let d = clients::history::parse_match(&read("match_game_flex.json"), Some("me-puuid"), Some(&read("match_timeline_flex.json"))).unwrap();
    let a = coach::analyze_match(&d, &sd(), None, None, coach::winprob::Model::shipped());
    assert!(a.laning.is_some() && a.jungle.is_none());
}
