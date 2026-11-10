mod common;

use coach::{GameInput, game_advice};
use common::*;
use model::*;

fn bases(mut sd: StaticData) -> StaticData {
    for c in sd.champs.values_mut() {
        let ranged = c.classes.iter().any(|k| matches!(k, ChampClass::Mage | ChampClass::Marksman));
        c.base = BaseStats {
            hp: 600.0,
            hp_per_level: 100.0,
            armor: 35.0,
            armor_per_level: 4.5,
            mr: 32.0,
            mr_per_level: 2.0,
            attack_range: if ranged { 550.0 } else { 125.0 },
            attack_speed: 0.65,
        };
    }
    sd
}

fn live() -> ChampionStats {
    ChampionStats {
        ability_power: 520.0,
        attack_damage: 90.0,
        attack_range: 550.0,
        attack_speed: 0.8,
        armor: 70.0,
        magic_resist: 50.0,
        max_health: 2100.0,
        ability_haste: 40.0,
        magic_pen_flat: 12.0,
        ..Default::default()
    }
}

fn stats() -> ChampStats {
    let mut s = ahri_stats();
    s.champ = SYNDRA;
    s.slots[3] = vec![row(VOID_STAFF, 3_000, 55.1), row(RABADONS, 3_000, 54.5), row(MORELLO, 1_500, 49.0)];
    s
}



fn game(laner: bool, fed: bool, live_stats: bool) -> GameSnapshot {
    let enemies = if laner {
        [(DARIUS, Lane::Top), (LEE_SIN, Lane::Jungle), (MALPHITE, Lane::Middle), (JINX, Lane::Bottom), (THRESH, Lane::Support)]
    } else {
        [(MALPHITE, Lane::Top), (LEE_SIN, Lane::Jungle), (ZED, Lane::Middle), (JINX, Lane::Bottom), (THRESH, Lane::Support)]
    };
    let mut p = lobby(enemies);
    for x in &mut p[5..] {
        *x = x.clone().lvl(13).cs(150).items(&[RANDUINS, RANDUINS]);
    }
    p[0].champ = SYNDRA;
    p[0] = p[0].clone().items(&[LUDENS, ZHONYAS, BANSHEES, SORCS]);
    let malphite = p.iter().position(|x| x.champ == MALPHITE).unwrap();
    p[malphite] = if fed {
        p[malphite].clone().kda(9, 1, 6).lvl(16).cs(230).items(&[SPIRIT_VISAGE; 6])
    } else {
        p[malphite].clone().kda(0, 7, 2).lvl(11).cs(90).items(&[SPIRIT_VISAGE; 2])
    };
    let mut s = snap(p, vec![]);
    s.my_stats = live_stats.then(live);
    s.my_abilities = Some([5, 2, 3, 2]);
    s
}

fn advise(s: &GameSnapshot) -> GameAdvice {
    let sd = bases(sd());
    let st = stats();
    game_advice(&GameInput { snap: s, sd: &sd, stats: Some(&st), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None })
}

fn scores(a: &GameAdvice) -> Vec<(ItemId, f32)> {
    a.buy_next.iter().chain(&a.alternatives).map(|p| (p.item, p.score)).collect()
}

fn order(a: &GameAdvice) -> Vec<ItemId> {
    a.buy_next.iter().chain(&a.alternatives).map(|p| p.item).filter(|i| [RABADONS, VOID_STAFF].contains(i)).collect()
}

#[test]
fn a_fed_laner_with_high_mr_pulls_void_staff_above_rabadons() {
    let a = advise(&game(true, true, true));
    assert_eq!(order(&a), [VOID_STAFF, RABADONS], "{:?} {:?}", scores(&a), a.what_if.iter().map(|w| (w.item, w.damage)).collect::<Vec<_>>());
    let w = a.what_if.iter().find(|w| w.item == VOID_STAFF).unwrap();
    assert!(w.per_enemy[0].champ == MALPHITE && w.per_enemy[0].lane, "the laner leads the table");
    assert!(w.per_enemy[1..].iter().all(|e| !e.lane));
    assert!(!w.rough && w.note.is_none());
}

#[test]
fn the_same_enemy_behind_and_not_my_laner_does_not() {
    let a = advise(&game(false, false, true));
    assert_eq!(order(&a), [RABADONS, VOID_STAFF], "{:?} {:?}", scores(&a), a.what_if.iter().map(|w| (w.item, w.damage)).collect::<Vec<_>>());
    let mal = a.threats.iter().find(|t| t.champ == MALPHITE).unwrap();
    assert!(!mal.lane_opp && mal.score < 50.0, "{mal:?}");
}

#[test]
fn without_live_stats_the_order_is_todays() {
    for (laner, fed) in [(true, true), (false, false)] {
        let a = advise(&game(laner, fed, false));
        assert!(a.what_if.is_empty());
        assert_eq!(order(&a), [VOID_STAFF, RABADONS], "laner {laner}");
    }
}

#[test]
fn what_if_covers_buy_next_and_the_alternatives() {
    let a = advise(&game(true, true, true));
    let ids: Vec<ItemId> = a.buy_next.iter().chain(&a.alternatives).map(|p| p.item).collect();
    assert_eq!(a.what_if.iter().map(|w| w.item).collect::<Vec<_>>(), ids);
    let rab = a.what_if.iter().find(|w| w.item == RABADONS).unwrap();
    assert!(rab.damage > 0.2 && rab.ehp.abs() < 1e-6, "{rab:?}");
    assert_eq!(rab.damage, rab.burst, "Syndra leads with burst");
}

#[test]
fn a_champion_without_a_parsed_kit_uses_its_class_profile_and_is_rough() {
    let mut sd = bases(sd());
    sd.champs.get_mut(&SYNDRA).unwrap().id = "Ryze".into();
    let st = stats();
    let s = game(true, true, true);
    let a = game_advice(&GameInput { snap: &s, sd: &sd, stats: Some(&st), augments: None, offered: &[], gold_history: &[], my_augments: &[], enemy_augments: Default::default(), keep: None, matchup: None });
    let rab = a.what_if.iter().find(|w| w.item == RABADONS).unwrap();
    assert!(rab.rough && rab.damage > 0.2, "{rab:?}");
}

#[test]
fn mayhem_says_augment_procs_are_not_counted() {
    let mut s = game(true, true, true);
    s.mode = Mode::Mayhem;
    let a = advise(&s);
    assert!(!a.what_if.is_empty() && a.what_if.iter().all(|w| w.note.as_deref() == Some("Augment procs not counted")));
}
