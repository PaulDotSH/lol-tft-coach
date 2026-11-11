//! Items and where the game stands: snowball items, the tempo term behind, Coachless situations.

mod common;

use coach::winprob::Model;
use coach::{GameInput, game_advice, situation_items};
use common::*;
use model::*;
use std::collections::HashMap;

const HANDSET: &str = include_str!("../../../fixtures/coach/winprob_handset.toml");
const MEJAIS: ItemId = 3041;

fn model() -> Model {
    Model::parse(HANDSET).unwrap()
}

fn sd_mejais() -> StaticData {
    let mut sd = sd();
    let mut m = sd.items[&LUDENS].clone();
    (m.id, m.name, m.cost, m.from, m.stats) = (MEJAIS, "Mejai's Soulstealer".into(), 1500, vec![], ItemStats { ap: 20.0, ..Default::default() });
    sd.items.insert(MEJAIS, m);
    sd
}

/// Ahri's first item: Rabadon's a little ahead of Luden's, Mejai's best of all where it may be offered.
fn stats() -> ChampStats {
    let mut s = ahri_stats();
    s.slots = vec![vec![row(RABADONS, 10_000, 52.5), row(LUDENS, 10_000, 52.0), row(MEJAIS, 10_000, 60.0)]];
    s
}

/// Everyone holds two Tomes and a Blasting Wand; `them` adds item gold to each enemy.
fn game(them: &[ItemId]) -> GameSnapshot {
    let mut p = lobby(STANDARD);
    for (i, x) in p.iter_mut().enumerate() {
        x.items = [TOME, TOME, WAND].iter().chain(if i >= 5 { them } else { &[] }).copied().collect();
    }
    snap(p, vec![])
}

fn behind() -> GameSnapshot {
    game(&[RUBY, RUBY])
}

fn advise(s: &GameSnapshot, sd: &StaticData, winprob: Option<&Model>, income: Option<f32>, sits: &HashMap<ItemId, ItemSituations>) -> GameAdvice {
    let st = stats();
    game_advice(&GameInput { snap: s, sd, stats: Some(&st), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income, winprob, situations: sits })
}

fn buy(a: &GameAdvice) -> ItemId {
    a.buy_next.as_ref().expect("buy next").item
}

fn ids(a: &GameAdvice) -> Vec<ItemId> {
    a.buy_next.iter().chain(&a.alternatives).chain(&a.path).map(|p| p.item).collect()
}

#[test]
fn the_states_these_tests_stand_on() {
    let (sd, m, none) = (sd(), model(), HashMap::new());
    let base = |s: &GameSnapshot| advise(s, &sd, Some(&m), None, &none).win.unwrap().base;
    assert!((0.42..0.58).contains(&base(&game(&[]))), "{}", base(&game(&[])));
    assert!(base(&behind()) <= 0.42, "{}", base(&behind()));
}

#[test]
fn behind_a_far_off_item_falls_below_a_near_one() {
    let (sd, m, none) = (sd(), model(), HashMap::new());
    // Luden's needs 750 more gold, Rabadon's 2,150: at 5 g/s, 2.5 against 7.2 minutes.
    assert_eq!(buy(&advise(&game(&[]), &sd, Some(&m), Some(5.0), &none)), RABADONS, "even: no tempo term");
    assert_eq!(buy(&advise(&behind(), &sd, Some(&m), None, &none)), RABADONS, "no income tracked yet");
    assert_eq!(buy(&advise(&behind(), &sd, None, Some(5.0), &none)), RABADONS, "no win probability");
    let a = advise(&behind(), &sd, Some(&m), Some(5.0), &none);
    assert_eq!(buy(&a), LUDENS, "{:?}", a.alternatives);
    let even = advise(&game(&[]), &sd, Some(&m), Some(5.0), &none);
    let lost = even.buy_next.unwrap().score - a.alternatives.iter().find(|p| p.item == RABADONS).unwrap().score;
    assert!(lost > 0.0 && lost < coach::consts::KEEP_MARGIN, "{lost}");
}

#[test]
fn snowball_items_need_the_game_ahead() {
    let (sd, m, none) = (sd_mejais(), model(), HashMap::new());
    let fed = |mut s: GameSnapshot| {
        s.players[0] = s.players[0].clone().kda(6, 0, 5);
        s
    };
    // Without a win probability: the KDA rule.
    assert!(!ids(&advise(&game(&[]), &sd, None, None, &none)).contains(&MEJAIS));
    assert_eq!(buy(&advise(&fed(game(&[])), &sd, None, None, &none)), MEJAIS);
    // With one: 6/0/5 in an even game is not ahead, nor behind.
    assert!(!ids(&advise(&fed(game(&[])), &sd, Some(&m), None, &none)).contains(&MEJAIS));
    assert!(!ids(&advise(&fed(behind()), &sd, Some(&m), None, &none)).contains(&MEJAIS));
    // Ahead: my team holds far more item gold.
    let mut ahead = game(&[]);
    for p in &mut ahead.players[..5] {
        p.items.extend([RUBY, RUBY, RUBY]);
    }
    let a = advise(&ahead, &sd, Some(&m), None, &none);
    assert!(a.win.as_ref().unwrap().base >= 0.58);
    assert_eq!(buy(&a), MEJAIS);
    // …but not while my own item gold is under my team's average.
    ahead.players[0].items = vec![TOME, TOME, WAND];
    assert!(!ids(&advise(&ahead, &sd, Some(&m), None, &none)).contains(&MEJAIS));
}

/// Luden's in every situation as good as overall, from behind `behind_wpa` points better over `games`.
fn ludens_behind(behind_wpa: f32, games: f64) -> HashMap<ItemId, ItemSituations> {
    let by = Situation::ALL.iter().map(|&s| if s == Situation::Behind { (s, 1.0 + behind_wpa, games) } else { (s, 1.0, 20_000.0) }).collect();
    HashMap::from([(LUDENS, ItemSituations { overall: 1.0, by })])
}

#[test]
fn an_item_better_from_behind_rises_when_behind_and_not_when_even() {
    let (sd, m) = (sd(), model());
    let sits = ludens_behind(2.0, 20_000.0);
    assert_eq!(buy(&advise(&behind(), &sd, Some(&m), None, &HashMap::new())), RABADONS);
    let a = advise(&behind(), &sd, Some(&m), None, &sits);
    let b = a.buy_next.clone().unwrap();
    assert_eq!(b.item, LUDENS);
    assert!(b.reason.contains("better from behind (Coachless)"), "{}", b.reason);
    let even = advise(&game(&[]), &sd, Some(&m), None, &sits);
    assert_eq!(buy(&even), RABADONS);
    assert!(!even.alternatives.iter().any(|p| p.reason.contains("Coachless")), "{:?}", even.alternatives);
    let (plain, unfit) = (advise(&behind(), &sd, None, None, &HashMap::new()), advise(&behind(), &sd, None, None, &sits));
    assert_eq!(unfit, plain, "no win probability: items as today, whatever Coachless says");
}

#[test]
fn too_few_games_in_a_situation_count_nothing() {
    let (sd, m) = (sd(), model());
    let a = advise(&behind(), &sd, Some(&m), None, &ludens_behind(2.0, 100.0));
    assert_eq!(buy(&a), RABADONS);
    assert!(a.alternatives.iter().all(|p| !p.reason.contains("Coachless")));
    let empty = HashMap::from([(LUDENS, ItemSituations { overall: 1.0, by: vec![] })]);
    let none = advise(&behind(), &sd, Some(&m), None, &HashMap::new());
    let with = advise(&behind(), &sd, Some(&m), None, &empty);
    assert_eq!(ids(&with), ids(&none), "no games is no data");
    assert_eq!(with.alternatives.iter().map(|p| p.score).collect::<Vec<_>>(), none.alternatives.iter().map(|p| p.score).collect::<Vec<_>>());
}

#[test]
fn situation_data_halves_the_answers_term() {
    // Zhonya's leads slot 3 against a fed Zed on its answers; with situation data (all even) it keeps half of them.
    let (sd, m) = (sd(), model());
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().items(&[LUDENS, RABADONS, SORCS]);
    p[7] = p[7].clone().kda(12, 1, 3).lvl(11).cs(130).items(&[YOUMUUS, EDGE_OF_NIGHT, LONG_SWORD]);
    let s = snap(p, killed_me(7, 3));
    let st = ahri_stats();
    let run = |sits: &HashMap<ItemId, ItemSituations>| {
        game_advice(&GameInput { snap: &s, sd: &sd, stats: Some(&st), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: Some(&m), situations: sits })
    };
    let flat = ItemSituations { overall: 0.5, by: Situation::ALL.iter().map(|&w| (w, 0.5, 20_000.0)).collect() };
    let (plain, halved) = (run(&HashMap::new()), run(&HashMap::from([(ZHONYAS, flat)])));
    let score = |a: &GameAdvice, id| a.buy_next.iter().chain(&a.alternatives).chain(&a.path).find(|p| p.item == id).map(|p| p.score);
    assert_eq!(plain.buy_next.as_ref().unwrap().item, ZHONYAS);
    assert!(score(&halved, ZHONYAS).is_none_or(|s| s < score(&plain, ZHONYAS).unwrap() - 0.01), "{:?} {:?}", score(&halved, ZHONYAS), score(&plain, ZHONYAS));
    assert_eq!(score(&halved, VOID_STAFF), score(&plain, VOID_STAFF), "an item without data keeps its score");
}

#[test]
fn situations_are_asked_for_buy_next_its_alternatives_and_the_next_two_path_items() {
    let sd = sd();
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().items(&[LUDENS, RABADONS, SORCS]);
    let s = snap(p, vec![]);
    let st = ahri_stats();
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: Some(&st), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: None, situations: &HashMap::new() });
    assert_eq!(a.alternatives.len(), 2);
    let mut expect: Vec<(ItemId, BuildSlot)> = vec![];
    let all = a.buy_next.iter().chain(&a.alternatives).map(|p| (p.item, BuildSlot::Legendary(2))).chain(a.path[1..3].iter().enumerate().map(|(i, p)| (p.item, BuildSlot::Legendary(3 + i))));
    for x in all {
        if !expect.iter().any(|e| e.0 == x.0) {
            expect.push(x);
        }
    }
    assert_eq!(situation_items(&a, &s, &sd), expect);
    let mut boots = lobby(STANDARD);
    boots[0] = boots[0].clone().items(&[LUDENS]);
    let s = snap(boots, vec![]);
    let a = advise(&s, &sd, None, None, &HashMap::new());
    assert_eq!(situation_items(&a, &s, &sd)[0], (a.buy_next.unwrap().item, BuildSlot::Boots));
}
