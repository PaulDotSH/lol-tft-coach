//! Shopping list, gold ETA, enemy spikes and death windows.

mod common;

use coach::{GameInput, LiveInput, Tracker, game_advice, gold_eta, live_extras, shopping};
use common::*;
use model::*;
use std::collections::HashMap;

const PATH: [ItemId; 2] = [ZHONYAS, SORCS];

fn me(items: &[ItemId]) -> Player {
    player(AHRI, Team::Order, Some(Lane::Middle)).items(items)
}

fn bought(l: &ShoppingList) -> Vec<(ItemId, u32, BuyKind)> {
    l.buys.iter().map(|b| (b.item, b.price, b.kind)).collect()
}

fn with_extras() -> StaticData {
    let mut sd = sd();
    let consumable = |id, name: &str, cost, rift, aram| Item {
        id,
        name: name.into(),
        cost,
        from: vec![],
        into: vec![],
        stats: ItemStats::default(),
        answers: Profile::default(),
        boots: false,
        completed: false,
        rift,
        aram,
        tags: vec!["Consumable".into()],
    };
    for it in [
        consumable(2055, "Control Ward", 75, true, false),
        consumable(2138, "Elixir of Iron", 500, true, true),
        consumable(2139, "Elixir of Sorcery", 500, true, true),
        consumable(2140, "Elixir of Wrath", 500, true, true),
    ] {
        sd.items.insert(it.id, it);
    }
    sd
}

// ---- Q1 shopping ----

#[test]
fn exact_gold_buys_the_full_item_with_nothing_left() {
    let l = shopping(&sd(), Mode::Aram, &me(&[LUDENS, RABADONS, BOOTS]), 3250.0, &PATH);
    assert_eq!(bought(&l), [(ZHONYAS, 3250, BuyKind::Full)]);
    assert_eq!((l.gold, l.leftover), (3250, 0));
}

#[test]
fn one_gold_short_buys_biggest_components_then_spends_leftover_on_the_next_item() {
    let l = shopping(&sd(), Mode::Aram, &me(&[LUDENS, RABADONS, BOOTS]), 3249.0, &PATH);
    // Seeker's (1000) before Blasting Wand (850); Sorcerer's costs 800 with Boots held.
    assert_eq!(
        bought(&l),
        [(SEEKERS, 1000, BuyKind::Component), (WAND, 850, BuyKind::Component), (SORCS, 800, BuyKind::Full)]
    );
    assert_eq!(l.buys[0].toward, Some(ZHONYAS));
    assert_eq!(l.leftover, 3249 - 1000 - 850 - 800);
}

#[test]
fn held_components_discount_the_full_item() {
    let l = shopping(&sd(), Mode::Aram, &me(&[SEEKERS, WAND]), 1400.0, &PATH);
    assert_eq!(bought(&l)[0], (ZHONYAS, 3250 - 1000 - 850, BuyKind::Full));
    assert_eq!(l.leftover, 0);
}

#[test]
fn full_inventory_cannot_take_components_but_can_finish_an_item() {
    let full = [LUDENS, RABADONS, VOID_STAFF, BANSHEES, BOOTS, TOME];
    // Tome sits in Seeker's: buying Seeker's frees its slot, a lone Wand does not fit.
    let l = shopping(&sd(), Mode::Aram, &me(&full), 900.0, &PATH);
    assert_eq!(bought(&l), [(SEEKERS, 600, BuyKind::Component)], "{l:?}");
    let l = shopping(&sd(), Mode::Aram, &me(&full), 500.0, &PATH);
    assert!(l.buys.is_empty(), "{l:?}");
}

#[test]
fn control_ward_on_the_rift_only_and_elixir_once_the_build_is_done() {
    let sd = with_extras();
    let l = shopping(&sd, Mode::Rift, &me(&[LUDENS, RABADONS, BOOTS]), 3330.0, &PATH);
    assert_eq!(bought(&l), [(ZHONYAS, 3250, BuyKind::Full), (2055, 75, BuyKind::Extra)]);
    assert_eq!(l.leftover, 5);
    let l = shopping(&sd, Mode::Aram, &me(&[LUDENS, RABADONS, BOOTS]), 3330.0, &PATH);
    assert_eq!(bought(&l), [(ZHONYAS, 3250, BuyKind::Full)]);

    let done = me(&[LUDENS, RABADONS, ZHONYAS, VOID_STAFF, BANSHEES, SORCS]).lvl(16);
    let l = shopping(&sd, Mode::Aram, &done, 1200.0, &[]);
    assert_eq!(bought(&l), [(2139, 500, BuyKind::Extra)], "AP champion drinks Sorcery");
    let l = shopping(&sd, Mode::Aram, &done.clone().lvl(8), 1200.0, &[]);
    assert!(l.buys.is_empty(), "elixirs need level 9");
}

#[test]
fn shopping_list_carries_death_timer_through_game_advice() {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().items(&[LUDENS, RABADONS, SORCS]);
    p[0].dead = true;
    p[0].respawn_in = 21.5;
    let mut s = snap(p, vec![]);
    s.my_gold = 3300.0;
    let sd = sd();
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: Some(&ahri_stats()), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: Some(&coach::winprob::Model::default()), situations: &Default::default() });
    let l = &a.shopping;
    assert!(l.dead && l.respawn_in == 21.5, "{l:?}");
    let first = &l.buys[0];
    assert_eq!((first.item, first.kind), (a.buy_next.as_ref().unwrap().item, BuyKind::Full), "{l:?}");
    assert_eq!(l.gold - l.buys.iter().map(|b| b.price).sum::<u32>(), l.leftover);
}

// ---- Q2 gold ETA ----

fn observe(t: &mut Tracker, sd: &StaticData, time: f32, gold: f32) {
    let mut s = snap(lobby(STANDARD), vec![]);
    (s.time, s.my_gold) = (time, gold);
    t.observe(&s, sd);
}

#[test]
fn income_ignores_spending_drops() {
    let sd = sd();
    let mut t = Tracker::default();
    // +20 g every 2 s, with a 1000 g purchase at 40 s.
    let mut gold = 2000.0;
    for k in 0..=30 {
        let time = 600.0 + 2.0 * k as f32;
        if k > 0 {
            gold += 20.0;
        }
        if k == 20 {
            gold -= 1000.0;
        }
        observe(&mut t, &sd, time, gold);
    }
    let r = t.income().expect("rate");
    assert!((r - 10.0).abs() < 1e-3, "{r}");

    let e = gold_eta(&sd, Mode::Rift, &me(&[LUDENS, RABADONS, BOOTS]), 2250.0, &PATH, Some(r));
    assert_eq!((e.item, e.missing), (Some(ZHONYAS), 1000));
    assert!((e.eta_s.unwrap() - 100.0).abs() < 0.01 && (e.per_min.unwrap() - 600.0).abs() < 0.1, "{e:?}");
}

#[test]
fn income_needs_enough_samples_and_resets_on_a_new_game() {
    let sd = sd();
    let mut t = Tracker::default();
    for k in 0..5 {
        observe(&mut t, &sd, 100.0 + 2.0 * k as f32, 500.0 + 30.0 * k as f32);
    }
    assert_eq!(t.income(), None, "8 s is too short");
    for k in 5..20 {
        observe(&mut t, &sd, 100.0 + 2.0 * k as f32, 500.0 + 30.0 * k as f32);
    }
    assert!((t.income().unwrap() - 15.0).abs() < 1e-3);
    observe(&mut t, &sd, 3.0, 500.0);
    assert_eq!(t.income(), None, "clock went back: new game");
}

#[test]
fn income_window_forgets_old_gains() {
    let sd = sd();
    let mut t = Tracker::default();
    let mut gold = 0.0;
    for k in 0..=150 {
        gold += if k <= 60 { 50.0 } else { 10.0 };
        observe(&mut t, &sd, 2.0 * k as f32, gold);
    }
    // Last 120 s are all +10 per 2 s.
    assert!((t.income().unwrap() - 5.0).abs() < 1e-3, "{:?}", t.income());
}

#[test]
fn back_plans_jump_to_the_gold_where_the_list_grows() {
    let e = gold_eta(&sd(), Mode::Rift, &me(&[LUDENS, RABADONS, BOOTS]), 900.0, &PATH, Some(10.0));
    let plans: Vec<_> = e.backs.iter().map(|b| (b.gold, b.items.clone(), b.in_s)).collect();
    assert_eq!(plans, [(1000, vec![SEEKERS], Some(10.0)), (1800, vec![SEEKERS, SORCS], Some(90.0))]);
    assert_eq!(e.missing, 2350);
}

#[test]
fn affordable_item_has_zero_eta_and_no_back_plans() {
    let e = gold_eta(&sd(), Mode::Rift, &me(&[]), 4000.0, &PATH, None);
    assert_eq!((e.missing, e.eta_s, e.per_min), (0, Some(0.0), None));
    assert!(e.backs.is_empty());
}

// ---- Q5 / V6 spikes, through live_extras ----

fn extras(s: &GameSnapshot, t: &Tracker, ults: &HashMap<ChampKey, UltCooldowns>) -> GameAdvice {
    let sd = sd();
    let mut a = game_advice(&GameInput { snap: s, sd: &sd, stats: None, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None, curves: &Default::default(), income: None, winprob: Some(&coach::winprob::Model::default()), situations: &Default::default() });
    live_extras(&mut a, &LiveInput { snap: s, sd: &sd, tracker: t, ults, notes: &LaneNotes::default(), spells: &HashMap::new(), memory: &[] });
    a
}

fn threat(a: &GameAdvice, c: ChampKey) -> &Threat {
    a.threats.iter().find(|t| t.champ == c).unwrap()
}

#[test]
fn r_rank_follows_visible_level_with_static_cooldowns() {
    let mut p = lobby([
        (MALPHITE, Lane::Top),
        (LEE_SIN, Lane::Jungle),
        (ZED, Lane::Middle),
        (JINX, Lane::Bottom),
        (THRESH, Lane::Support),
    ]);
    for (i, lvl) in [(5, 5), (6, 6), (7, 10), (8, 11), (9, 16)] {
        p[i].level = lvl;
    }
    let s = snap(p, vec![]);
    let ults = HashMap::from([
        (MALPHITE, UltCooldowns { max_rank: 3, cooldown: vec![130.0, 105.0, 80.0] }),
        (JINX, UltCooldowns { max_rank: 3, cooldown: vec![75.0, 65.0, 55.0] }),
        (LEE_SIN, UltCooldowns { max_rank: 3, cooldown: vec![110.0, 85.0, 60.0] }),
    ]);
    let a = extras(&s, &Tracker::default(), &ults);
    let u = |c| threat(&a, c).ult.clone();
    assert_eq!(u(MALPHITE), None, "level 5: no R yet");
    assert_eq!(u(LEE_SIN), Some(UltInfo { rank: 1, cooldowns: vec![110.0, 85.0, 60.0], spike: true }));
    assert_eq!(u(ZED), Some(UltInfo { rank: 1, cooldowns: vec![], spike: false }), "cooldowns not fetched yet");
    assert_eq!(u(JINX).map(|x| (x.rank, x.spike)), Some((2, true)));
    assert_eq!(u(THRESH).map(|x| (x.rank, x.spike)), Some((3, true)));
}

#[test]
fn four_rank_ults_start_at_one_and_odd_ults_are_skipped() {
    let four = UltCooldowns { max_rank: 4, cooldown: vec![6.0, 6.0, 6.0, 6.0] };
    let mut p = lobby(STANDARD);
    p[7].level = 1;
    let s = snap(p, vec![]);
    let a = extras(&s, &Tracker::default(), &HashMap::from([(ZED, four), (JINX, UltCooldowns { max_rank: 6, cooldown: vec![1.0; 6] })]));
    assert_eq!(threat(&a, ZED).ult.as_ref().map(|u| (u.rank, u.spike)), Some((1, true)));
    assert_eq!(threat(&a, JINX).ult, None);
}

#[test]
fn newly_finished_legendary_shows_for_a_while() {
    let sd = sd();
    let mut t = Tracker::default();
    let mut p = lobby(STANDARD);
    p[8] = p[8].clone().items(&[INFINITY_EDGE]); // Jinx: held when tracking began
    p[7] = p[7].clone().items(&[LONG_SWORD]);
    let mut s = snap(p, vec![]);
    t.observe(&s, &sd);
    s.time += 2.0;
    s.players[7].items = vec![YOUMUUS];
    t.observe(&s, &sd);
    let a = extras(&s, &t, &HashMap::new());
    assert_eq!(threat(&a, ZED).new_items, [YOUMUUS]);
    assert!(threat(&a, JINX).new_items.is_empty(), "already held at first sight");

    s.time += 120.0;
    t.observe(&s, &sd);
    assert!(extras(&s, &t, &HashMap::new()).threats.iter().all(|x| x.new_items.is_empty()));
}

#[test]
fn finished_legendaries_raise_threat_over_the_same_gold_in_parts() {
    let mut p = lobby(STANDARD);
    p[7] = p[7].clone().items(&[YOUMUUS]); // Zed: 2800 g, one legendary
    p[6] = p[6].clone().items(&[LONG_SWORD; 8]); // Lee Sin: 2800 g of parts
    for i in [6, 7] {
        p[i] = p[i].clone().kda(3, 3, 3).cs(90).lvl(10);
        p[i].position = Some(Lane::Top);
    }
    let a = extras(&snap(p, vec![]), &Tracker::default(), &HashMap::new());
    let (z, l) = (threat(&a, ZED), threat(&a, LEE_SIN));
    assert_eq!((z.legendaries, z.legendary_diff, l.legendaries), (1, 1, 0));
    assert!(z.score > l.score, "{} vs {}", z.score, l.score);
}

// ---- Q6 death windows ----

#[test]
fn death_windows_list_dead_players_soonest_back_first() {
    let mut p = lobby(STANDARD);
    for (i, t) in [(0, 30.0), (2, 12.0), (5, 25.0), (7, 18.0), (9, 9.5)] {
        p[i].dead = true;
        p[i].respawn_in = t;
    }
    let a = extras(&snap(p, vec![]), &Tracker::default(), &HashMap::new());
    let d = &a.deaths;
    let side = |v: &[Respawn]| v.iter().map(|r| (r.champ, r.in_s)).collect::<Vec<_>>();
    assert_eq!(side(&d.enemies), [(THRESH, 9.5), (ZED, 18.0), (DARIUS, 25.0)]);
    assert_eq!(side(&d.allies), [(JARVAN, 12.0)], "me excluded");

    let a = extras(&snap(lobby(STANDARD), vec![]), &Tracker::default(), &HashMap::new());
    assert!(a.deaths.enemies.is_empty() && a.deaths.allies.is_empty());
}
