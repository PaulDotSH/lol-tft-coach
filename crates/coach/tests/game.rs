//! In-game advice behaviour.

mod common;

use coach::{GameInput, game_advice};
use common::*;
use model::*;

fn advise(snap: &GameSnapshot, stats: Option<&ChampStats>) -> GameAdvice {
    let sd = sd();
    game_advice(&GameInput { snap, sd: &sd, stats, augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() })
}

/// Zed (index 7) mid, 12/1/3, two lethality items, killed me 3×.
fn fed_zed(my_items: &[ItemId]) -> GameSnapshot {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().items(my_items);
    p[7] = p[7].clone().kda(12, 1, 3).lvl(11).cs(130).items(&[YOUMUUS, EDGE_OF_NIGHT, LONG_SWORD]);
    snap(p, killed_me(7, 3))
}

fn all_ids(a: &GameAdvice) -> Vec<ItemId> {
    a.buy_next.iter().chain(&a.alternatives).chain(&a.path).map(|p| p.item).collect()
}

#[test]
fn fed_ap_assassin_who_killed_me_makes_stasis_buy_next_at_slot_3() {
    let s = fed_zed(&[LUDENS, RABADONS, SORCS]);
    let a = advise(&s, Some(&ahri_stats()));
    let b = a.buy_next.clone().expect("buy next");
    assert_eq!(b.item, ZHONYAS, "{a:#?}");
    assert!(b.reason.contains("Zed") && b.reason.contains("killed you 3×"), "{}", b.reason);
}

#[test]
fn without_the_fed_assassin_slot_3_follows_stats() {
    let mut s = fed_zed(&[LUDENS, RABADONS, SORCS]);
    s.players[7] = player(ZED, Team::Chaos, Some(Lane::Middle));
    s.events.clear();
    let a = advise(&s, Some(&ahri_stats()));
    assert_eq!(a.buy_next.clone().unwrap().item, VOID_STAFF);
}

#[test]
fn slot_one_follows_lolalytics_even_with_threats() {
    let a = advise(&fed_zed(&[]), Some(&ahri_stats()));
    let b = a.buy_next.clone().unwrap();
    assert_eq!(b.item, LUDENS);
    assert!(b.reason.starts_with("top pick for Ahri (55% over 20k games)"), "{}", b.reason);
}

#[test]
fn healer_team_puts_anti_heal_in_path_and_others_do_not() {
    let a = advise(&snap(lobby(HEALERS), vec![]), Some(&ahri_stats()));
    let ids: Vec<_> = a.path.iter().map(|p| p.item).collect();
    assert!(ids.contains(&MORELLO), "{ids:?}");
    let m = a.path.iter().find(|p| p.item == MORELLO).unwrap();
    assert!(m.reason.starts_with("Anti-heal"), "{}", m.reason);

    let a = advise(&snap(lobby(STANDARD), vec![]), Some(&ahri_stats()));
    assert!(!a.path.iter().any(|p| p.item == MORELLO), "{:?}", a.path);
}

#[test]
fn owned_items_and_second_boots_never_recommended() {
    let owned = [LUDENS, ZHONYAS, SORCS, TOME];
    let a = advise(&fed_zed(&owned), Some(&ahri_stats()));
    let ids = all_ids(&a);
    assert!(!ids.is_empty());
    for o in owned {
        assert!(!ids.contains(&o), "{o} recommended: {ids:?}");
    }
    let sd = sd();
    assert!(!ids.iter().any(|i| sd.items[i].boots), "boots after Sorcerer's: {ids:?}");
    assert_eq!(a.owned, owned.to_vec());
}

#[test]
fn path_fills_to_six_items_with_one_pair_of_boots_and_no_duplicates() {
    let sd = sd();
    for owned in [&[][..], &[LUDENS][..], &[LUDENS, SORCS, RABADONS][..]] {
        let a = advise(&fed_zed(owned), Some(&ahri_stats()));
        let finished = owned.iter().filter(|i| sd.items[i].completed).count();
        assert_eq!(a.path.len() + finished, 6, "{owned:?}");
        let ids: Vec<_> = a.path.iter().map(|p| p.item).collect();
        let mut d = ids.clone();
        d.sort();
        d.dedup();
        assert_eq!(d.len(), ids.len());
        let boots = ids.iter().chain(owned).filter(|i| sd.items[*i].boots && sd.items[*i].completed).count();
        assert_eq!(boots, 1, "{ids:?}");
        assert!(ids.iter().all(|i| sd.items[i].completed), "components in path {ids:?}");
        assert_eq!(a.buy_next.as_ref().map(|b| b.item), ids.first().copied());
    }
}

#[test]
fn alternatives_are_two_distinct_others() {
    let a = advise(&fed_zed(&[LUDENS, RABADONS, SORCS]), Some(&ahri_stats()));
    let b = a.buy_next.clone().unwrap();
    assert_eq!(a.alternatives.len(), 2);
    assert!(a.alternatives.iter().all(|x| x.item != b.item && x.score <= b.score));
}

#[test]
fn boots_after_first_item_mercs_vs_cc_and_ap() {
    let mut p = lobby([(MALPHITE, Lane::Top), (THRESH, Lane::Jungle), (LUX, Lane::Middle), (SYNDRA, Lane::Bottom), (LEONA, Lane::Support)]);
    p[0] = p[0].clone().items(&[LUDENS]);
    p[7] = p[7].clone().kda(8, 1, 4).lvl(11).items(&[LUDENS]);
    let a = advise(&snap(p, killed_me(7, 2)), Some(&ahri_stats()));
    let b = a.buy_next.clone().unwrap();
    assert_eq!(b.item, MERCS, "{a:#?}");
}

#[test]
fn boots_plated_vs_auto_attackers() {
    let mut p = lobby([(DARIUS, Lane::Top), (LEE_SIN, Lane::Jungle), (VAYNE, Lane::Middle), (JINX, Lane::Bottom), (CAITLYN, Lane::Support)]);
    p[0] = p[0].clone().items(&[LUDENS]);
    p[8] = p[8].clone().kda(8, 1, 4).lvl(11).items(&[INFINITY_EDGE, BERSERKERS]);
    let a = advise(&snap(p, killed_me(8, 2)), Some(&ahri_stats()));
    assert_eq!(a.buy_next.clone().unwrap().item, PLATED, "{a:#?}");
}

#[test]
fn default_boots_follow_stats_when_nothing_special() {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().items(&[LUDENS]);
    let a = advise(&snap(p, vec![]), Some(&ahri_stats()));
    assert_eq!(a.buy_next.clone().unwrap().item, SORCS);
}

#[test]
fn lolalytics_missing_still_gives_rule_based_advice() {
    let sd = sd();
    let a = advise(&fed_zed(&[]), None);
    let b = a.buy_next.clone().expect("advice without stats");
    assert_eq!(b.item, ZHONYAS, "{a:#?}");
    assert!(b.reason.contains("Zed"));
    assert_eq!(a.path.len(), 6);
    for p in &a.path {
        let it = &sd.items[&p.item];
        assert!(it.boots || it.stats.ap > 0.0, "non-AP item {} for Ahri", it.name);
    }
    assert!(a.matchups.is_empty());
    assert_eq!(a.threats.len(), 5);
}

#[test]
fn components_and_wrong_map_items_never_recommended() {
    let sd = sd();
    let a = advise(&snap(lobby(HEALERS), vec![]), None);
    for i in all_ids(&a) {
        assert!(sd.items[&i].completed);
        assert_ne!(i, GUARDIANS_ORB, "ARAM-only item on the Rift");
    }
    let mut s = fed_zed(&[]);
    s.mode = Mode::Aram;
    let a = advise(&s, None);
    assert_eq!(a.buy_next.clone().unwrap().item, GUARDIANS_ORB, "ARAM item available in ARAM");
}

#[test]
fn marksman_vs_healers_gets_mortal_reminder_not_executioners() {
    let mut p = lobby(HEALERS);
    p[0] = player(CAITLYN, Team::Order, Some(Lane::Bottom)).items(&[INFINITY_EDGE, BERSERKERS]);
    p[3] = player(AHRI, Team::Order, Some(Lane::Middle));
    let a = advise(&snap(p, vec![]), None);
    let ids = all_ids(&a);
    assert!(ids.contains(&MORTAL_REMINDER), "{ids:?}");
    assert!(!ids.contains(&EXECUTIONERS));
    assert!(!ids.contains(&ZHONYAS) && !ids.contains(&RANDUINS), "off-direction item {ids:?}");
}

#[test]
fn last_whisper_items_do_not_stack() {
    let mut p = lobby(HEALERS);
    p[0] = player(CAITLYN, Team::Order, Some(Lane::Bottom)).items(&[INFINITY_EDGE, LORD_DOMINIKS, BERSERKERS]);
    p[3] = player(AHRI, Team::Order, Some(Lane::Middle));
    let a = advise(&snap(p, vec![]), None);
    assert!(!all_ids(&a).contains(&MORTAL_REMINDER));
}

#[test]
fn component_now_when_gold_is_short() {
    let sd = sd();
    let mut s = fed_zed(&[]);
    s.my_gold = 1100.0;
    let b = advise(&s, Some(&ahri_stats())).buy_next.unwrap();
    assert_eq!(b.item, LUDENS);
    assert_eq!(b.component_now, Some(WAND));

    // Holding a Blasting Wand: 2050 left, 2100 gold → buy the item.
    let mut s = fed_zed(&[WAND]);
    s.my_gold = 2100.0;
    assert_eq!(advise(&s, Some(&ahri_stats())).buy_next.unwrap().component_now, None);

    // Holding a Blasting Wand, 500 gold → a Tome, not another wand.
    s.my_gold = 500.0;
    assert_eq!(advise(&s, Some(&ahri_stats())).buy_next.unwrap().component_now, Some(TOME));

    // Too poor for anything.
    s.my_gold = 100.0;
    assert_eq!(advise(&s, Some(&ahri_stats())).buy_next.unwrap().component_now, None);
    assert!(sd.items[&TOME].cost > 100);
}

#[test]
fn threat_board_ranks_fed_lane_opponent_first() {
    let a = advise(&fed_zed(&[]), Some(&ahri_stats()));
    let t = &a.threats[0];
    assert_eq!(t.champ, ZED);
    assert!(t.lane_opp && t.kills_on_me == 3);
    assert_eq!(t.damage, "AD");
    assert!(t.flags.contains(&"assassin".to_string()));
    assert!(t.item_gold_diff > 3000 && t.level_diff >= 1, "{t:?}");
    assert!(t.score > 90.0 && t.score <= 100.0);
    assert!(a.threats.windows(2).all(|w| w[0].score >= w[1].score));
    assert!(a.threats[1..].iter().all(|t| !t.lane_opp && t.score < 70.0), "{:?}", a.threats);
    assert!(a.threats.iter().all(|t| (0.0..=100.0).contains(&t.score)));
}

#[test]
fn killing_me_raises_threat() {
    let base = snap(lobby(STANDARD), vec![]);
    let s0 = advise(&base, None).threats.iter().find(|t| t.champ == DARIUS).unwrap().score;
    let killed = snap(lobby(STANDARD), killed_me(5, 2));
    let s1 = advise(&killed, None).threats.iter().find(|t| t.champ == DARIUS).unwrap().score;
    assert!((s1 / s0 - 1.5).abs() < 0.01, "{s0} → {s1}");
}

#[test]
fn jungle_and_support_cs_compared_within_role() {
    // An enemy support with 15 CS is average for a support, not "behind".
    let a = advise(&snap(lobby(STANDARD), vec![]), None);
    let thresh = a.threats.iter().find(|t| t.champ == THRESH).unwrap();
    let darius = a.threats.iter().find(|t| t.champ == DARIUS).unwrap();
    assert!((thresh.score - darius.score).abs() < 1.0, "{thresh:?} {darius:?}");
}

#[test]
fn damage_label_follows_items() {
    let mut p = lobby([(KAYLE, Lane::Top), (LEE_SIN, Lane::Jungle), (ZED, Lane::Middle), (JINX, Lane::Bottom), (THRESH, Lane::Support)]);
    let a = advise(&snap(p.clone(), vec![]), None);
    assert_eq!(a.threats.iter().find(|t| t.champ == KAYLE).unwrap().damage, "Mixed");
    p[5] = p[5].clone().items(&[RABADONS, LUDENS, VOID_STAFF]);
    let a = advise(&snap(p, vec![]), None);
    assert_eq!(a.threats.iter().find(|t| t.champ == KAYLE).unwrap().damage, "AP");
    assert_eq!(a.threats.iter().find(|t| t.champ == THRESH).unwrap().flags, vec!["cc", "tank"]);
}

#[test]
fn true_damage_label() {
    let a = advise(&snap(lobby([(GAREN, Lane::Top), (LEE_SIN, Lane::Jungle), (ZED, Lane::Middle), (VAYNE, Lane::Bottom), (THRESH, Lane::Support)]), vec![]), None);
    let v = a.threats.iter().find(|t| t.champ == VAYNE).unwrap();
    assert_eq!(v.damage, "True");
    assert!(v.flags.contains(&"on-hit".to_string()) && v.flags.contains(&"true".to_string()));
}

#[test]
fn lane_opponent_from_lolalytics_lane_when_positions_unknown() {
    let mut p = lobby(STANDARD);
    p.iter_mut().skip(1).for_each(|x| x.position = None);
    p[0].position = None;
    let a = advise(&snap(p, vec![]), Some(&ahri_stats()));
    let opp: Vec<_> = a.threats.iter().filter(|t| t.lane_opp).map(|t| t.champ).collect();
    assert_eq!(opp, vec![ZED]);
}

#[test]
fn matchups_label_and_lane_opponent_first() {
    let a = advise(&fed_zed(&[]), Some(&ahri_stats()));
    assert_eq!(a.matchups[0].champ, ZED);
    assert!(a.matchups[0].lane_opp);
    assert_eq!(a.matchups[0].label, "even");
    let mut s = fed_zed(&[]);
    s.players[7].champ = SYNDRA;
    let a = advise(&s, Some(&ahri_stats()));
    assert_eq!((a.matchups[0].champ, a.matchups[0].label.as_str()), (SYNDRA, "unfavoured"));
    let mut s = fed_zed(&[]);
    s.players[7].champ = LUX;
    let a = advise(&s, Some(&ahri_stats()));
    assert_eq!(a.matchups[0].label, "favoured");
    // only enemies present in counters
    assert!(a.matchups.iter().all(|m| [LUX, DARIUS, JINX].contains(&m.champ)));
}

#[test]
fn self_stats_vs_benchmark_and_lane_opponent() {
    let mut p = lobby(STANDARD);
    p[0] = p[0].clone().cs(80).lvl(8).items(&[LUDENS]);
    p[0].deaths = 4;
    p[7] = p[7].clone().lvl(10).items(&[YOUMUUS, LONG_SWORD]);
    let mut ev = killed_me(7, 3);
    ev.push(kill(500.0, 6, 0));
    ev.push(GameEvent::Kill { time: 520.0, killer: None, victim: 0, assisters: vec![] });
    ev.push(kill(530.0, 0, 7));
    let mut s = snap(p, ev);
    s.time = 600.0;
    let m = advise(&s, Some(&ahri_stats())).me;
    assert!((m.cs_per_min - 8.0).abs() < 1e-4);
    assert_eq!(m.cs_benchmark, Some(7.4));
    assert!((m.cs_diff.unwrap() - 6.0).abs() < 1e-3);
    assert_eq!(m.item_gold_vs_opp, Some(2900 - 3150));
    assert_eq!(m.level_vs_opp, Some(-2));
    assert_eq!(m.deaths, 4);
    assert_eq!(m.killers, vec![(ZED, 3), (LEE_SIN, 1)]);
    assert_eq!(m.vision_benchmark, Some(8.0), "mid role constant × 10 min");

    s.mode = Mode::Aram;
    let m = advise(&s, None).me;
    assert_eq!((m.cs_benchmark, m.item_gold_vs_opp, m.vision_benchmark), (None, None, None));
}

#[test]
fn objective_timers() {
    let dragon = |time, team| GameEvent::Dragon { time, team: Some(team), kind: "Fire".into() };
    let get = |a: &GameAdvice, n: &str| a.timers.iter().find(|t| t.name == n).map(|t| t.next_spawn);

    let mut s = snap(lobby(STANDARD), vec![]);
    s.time = 200.0;
    let a = advise(&s, None);
    assert_eq!(get(&a, "Dragon"), Some(Some(300.0)));
    assert_eq!(get(&a, "Baron"), Some(Some(1200.0)));
    assert_eq!(get(&a, "Herald"), Some(Some(900.0)));
    assert_eq!(get(&a, "Void Grubs"), Some(Some(480.0)));

    s.time = 600.0;
    s.events = vec![dragon(420.0, Team::Order)];
    let a = advise(&s, None);
    assert_eq!(get(&a, "Dragon"), Some(Some(720.0)));
    assert_eq!(get(&a, "Void Grubs"), Some(None), "grubs up");

    s.time = 1600.0;
    s.events = vec![
        dragon(300.0, Team::Chaos),
        dragon(700.0, Team::Chaos),
        dragon(1050.0, Team::Order),
        dragon(1100.0, Team::Chaos),
        dragon(1500.0, Team::Chaos),
        GameEvent::Baron { time: 1300.0, team: Some(Team::Chaos) },
        GameEvent::Herald { time: 950.0, team: Some(Team::Order) },
    ];
    let a = advise(&s, None);
    assert_eq!(get(&a, "Dragon"), None);
    assert_eq!(get(&a, "Elder"), Some(Some(1860.0)));
    assert_eq!(get(&a, "Baron"), Some(Some(1660.0)));
    assert_eq!(get(&a, "Herald"), None);
    assert_eq!(get(&a, "Void Grubs"), None);

    s.events.push(GameEvent::Dragon { time: 1900.0, team: Some(Team::Order), kind: "Elder".into() });
    s.time = 2000.0;
    assert_eq!(get(&advise(&s, None), "Elder"), Some(Some(2260.0)));

    s.mode = Mode::Aram;
    assert!(advise(&s, None).timers.is_empty());
}

#[test]
fn aram_has_no_lane_opponent_matchups_or_timers() {
    let mut s = fed_zed(&[]);
    s.mode = Mode::Mayhem;
    let a = advise(&s, Some(&ahri_stats()));
    assert!(a.threats.iter().all(|t| !t.lane_opp));
    assert!(a.matchups.is_empty() && a.timers.is_empty());
    assert!(a.buy_next.is_some());
    assert!(a.augments.is_none());
}

#[test]
fn mayhem_offer_picks_highest_ranked_augment() {
    let sd = sd();
    let ranks = ChampAugments {
        champ: AHRI,
        prismatic: (1000..1052).collect(),
        gold: (2000..2040).collect(),
        silver: (3000..3030).collect(),
    };
    let md = MayhemData {
        patch: "16_21".into(),
        augment_names: [(1001, "Jeweled Gauntlet".to_string()), (1005, "Infernal Conduit".into())].into(),
        champ_tiers: Default::default(),
    };
    let mut s = fed_zed(&[]);
    s.mode = Mode::Mayhem;
    let offered = [1005, 1001, 1030, 9999];
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: Some((&ranks, &md)), offered: &offered, gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    let aug = a.augments.unwrap();
    assert_eq!(aug.best, Some(1001));
    assert_eq!(aug.reason, "#2 of 52 Prismatic for Ahri");
    assert_eq!(aug.offered.len(), 4);
    assert_eq!(aug.offered[0].name, "Infernal Conduit");
    assert_eq!((aug.offered[1].rarity, aug.offered[1].rank, aug.offered[1].of), (Some(Rarity::Prismatic), Some(1), 52));
    assert_eq!((aug.offered[3].rarity, aug.offered[3].rank), (None, None));

    // Normalised rank across rarities: #3 of 30 silver (0.067) beats #5 of 40 gold (0.1).
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: Some((&ranks, &md)), offered: &[2004, 3002], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    assert_eq!(a.augments.unwrap().best, Some(3002));

    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: None, augments: Some((&ranks, &md)), offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    assert!(a.augments.is_none());
}

#[test]
fn survives_empty_or_odd_snapshots() {
    let mut s = snap(vec![], vec![]);
    let a = advise(&s, Some(&ahri_stats()));
    assert!(a.buy_next.is_none() && a.threats.is_empty());
    s = snap(lobby(STANDARD), vec![]);
    s.time = 0.0;
    s.players[0].items = vec![99999];
    let a = advise(&s, Some(&ahri_stats()));
    assert!(a.me.cs_per_min.is_finite());
    assert!(a.buy_next.is_some());
}


#[test]
fn game_item_groups_block_items_that_cannot_be_owned_together() {
    // Zhonya's is buy next here; put it in a one-per-inventory group with an owned item.
    let s = fed_zed(&[LUDENS, RABADONS, SORCS]);
    let mut sd = sd();
    sd.item_groups = [(ZHONYAS, vec![0]), (RABADONS, vec![0])].into();
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: Some(&ahri_stats()), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default() });
    assert!(!all_ids(&a).contains(&ZHONYAS), "{:?}", all_ids(&a));
    assert!(a.buy_next.is_some());
}
