//! Build tables from the crawled games: the review's Build section and the in-game pace, on a
//! table cut from a real one (`fixtures/coach/builds_cut.json`).

mod common;

use coach::builds::{Table, pace, review};
use common::*;
use model::*;

const XAYAH: ChampKey = 498;
const START_A: ItemId = 2523;
const YUN_TAL: ItemId = 3032;
const STATIKK: ItemId = 3087;
const RUNAAN: ItemId = 3085;
const FIRECANNON: ItemId = 6675;
const CULL: ItemId = 1083;
const MIXED: [ChampKey; 5] = [19, 106, 24, 36, 102];
const PHYSICAL: [ChampKey; 5] = [92, 91, 119, 86, 254];
const MAGIC: [ChampKey; 5] = [69, 9, 3, 30, 136];

fn table() -> Table {
    Table::parse(include_str!("../../../fixtures/coach/builds_cut.json")).unwrap()
}

fn data() -> StaticData {
    let mut sd = sd();
    let base = sd.items[&INFINITY_EDGE].clone();
    for (id, name) in [(START_A, "Bloodsong Edge"), (YUN_TAL, "Yun Tal Wildarrows"), (STATIKK, "Statikk Shiv"), (RUNAAN, "Runaan's Hurricane"), (FIRECANNON, "Rapid Firecannon"), (CULL, "Cull")] {
        sd.items.insert(id, Item { id, name: name.into(), ..base.clone() });
    }
    let jinx = sd.champs[&JINX].clone();
    sd.champs.insert(XAYAH, Champion { key: XAYAH, name: "Xayah".into(), ..jinx });
    sd
}

fn game(champ: ChampKey, queue_id: i64, mode: Mode) -> PastGame {
    PastGame {
        game_id: 1,
        created: 0,
        queue_id,
        mode,
        champ,
        lane: Some(Lane::Bottom),
        win: true,
        remake: false,
        duration_s: 1800,
        kills: 5,
        deaths: 2,
        assists: 5,
        cs: 250,
        gold: 12000,
        damage_to_champs: 20000,
        vision: 15,
        death_times: None,
        gold_diff_10: None,
        gold_diff_15: None,
        xp_diff_15: None,
        ours: None,
        theirs: None,
        first_dragon: None,
        first_tower: None,
        first_blood: None,
        cs_10: None,
        augments: vec![],
        death_spots: vec![],
        enemies: vec![],
        opponent: None,
        killed_by: vec![],
        control_wards: None,
    }
}

fn line(champ: ChampKey, ally: bool, lane: Lane) -> MatchPlayer {
    MatchPlayer {
        champ,
        name: String::new(),
        ally,
        me: false,
        lane: Some(lane),
        level: 15,
        kills: 3,
        deaths: 3,
        assists: 3,
        cs: 150,
        gold: 10000,
        damage: 15000,
        taken: 15000,
        vision: 20,
        items: vec![],
        wards_placed: 10,
        wards_killed: 2,
        control_wards: 1,
    }
}

const LANES: [Lane; 5] = [Lane::Top, Lane::Jungle, Lane::Middle, Lane::Bottom, Lane::Support];
const ME: usize = 3;
const OPP: usize = 8;

/// Me at index 3 (bottom), the given enemies at 5..10; my gold per minute against my opponent's.
fn detail(champ: ChampKey, enemies: [ChampKey; 5], buys: &[(f32, ItemId)], gold: (u32, u32)) -> MatchDetail {
    let mut players: Vec<MatchPlayer> = LANES.iter().enumerate().map(|(i, l)| line(if i == ME { champ } else { GAREN }, true, *l)).collect();
    players.extend(LANES.iter().zip(enemies).map(|(l, c)| line(c, false, *l)));
    players[ME].me = true;
    let frames = (0..=30)
        .map(|m| Frame {
            time: m as f32 * 60.0,
            players: (0..10)
                .map(|i| FrameStat {
                    total_gold: 500 + m * if i == ME { gold.0 } else if i == OPP { gold.1 } else { 400 },
                    current_gold: if i == ME { 300 + 50 * m } else { 0 },
                    ..FrameStat::default()
                })
                .collect(),
        })
        .collect();
    let purchases = buys.iter().map(|&(time, item)| Purchase { time, item }).collect();
    MatchDetail { game: game(champ, 420, Mode::Rift), me: ME, players, frames, kills: vec![], marks: vec![], purchases }
}

fn usual_jinx(first: ItemId, second: ItemId) -> Vec<(f32, ItemId)> {
    vec![(0.0, 1055), (200.0, 1036), (680.0, first), (720.0, BERSERKERS), (1050.0, second), (1350.0, INFINITY_EDGE)]
}

fn tips(d: &MatchDetail, coachless: Option<&ChampStats>) -> Vec<String> {
    review(d, &table(), coachless, &data()).unwrap().tips
}

fn coachless(slot: usize, item: ItemId) -> ChampStats {
    let mut s = ahri_stats();
    s.coachless = Some("16.22".into());
    s.slots = vec![vec![]; 3];
    s.slots[slot] = vec![row(item, 1000, 51.0)];
    s
}

#[test]
fn the_table_parses_and_finds_a_champion_and_role() {
    let t = table();
    assert_eq!((t.patch.as_str(), t.games, t.damage_cuts), ("16.22", 15327, [0.4386, 0.5533]));
    assert!(t.pair(JINX, Lane::Bottom).is_some() && t.pair(JINX, Lane::Top).is_none() && t.pair(AHRI, Lane::Middle).is_none());
    assert!(t.finished.contains(&INFINITY_EDGE) && !t.finished.contains(&1036) && t.boots.contains(&BERSERKERS));
    assert!(Table::parse("{}").is_err() && Table::parse("<html>").is_err());
}

#[test]
fn a_usual_game_shows_rows_beside_both_bands_and_no_tip() {
    let r = review(&detail(JINX, PHYSICAL, &usual_jinx(START_A, RUNAAN), (400, 400)), &table(), None, &data()).unwrap();
    assert_eq!((r.patch.as_str(), r.games, r.top_games), ("16.22", 950, Some(950)));
    type Row<'a> = (&'a str, ItemId, u32, Option<(u32, u32)>);
    let rows: Vec<Row> = r.items.iter().map(|i| (i.slot.as_str(), i.item, i.time, i.all)).collect();
    assert_eq!(
        rows,
        [
            ("first item", START_A, 680, Some((589, 778))),
            ("second item", RUNAAN, 1050, Some((951, 1147))),
            ("third item", INFINITY_EDGE, 1350, Some((1261, 1469))),
            ("boots", BERSERKERS, 720, Some((552, 853))),
        ],
        "the first item's own times, the slot's for the rest"
    );
    assert_eq!(r.items[0].top, Some((589, 778)));
    let back = r.back.unwrap();
    assert_eq!((back.time, back.gold, back.all, back.all_gold, back.usual), (200, 450, Some((164, 240)), Some((317, 921)), Some(CULL)), "gold in pocket from the frame before");
    assert!(r.tips.is_empty(), "{:?}", r.tips);
}

#[test]
fn a_rare_first_item_is_an_order_tip_unless_coachless_has_it() {
    let d = detail(JINX, PHYSICAL, &usual_jinx(STATIKK, RUNAAN), (400, 400));
    assert_eq!(tips(&d, None), ["Statikk Shiv first is rare on Jinx bottom; Emerald+ start Bloodsong Edge (59 %) or Yun Tal Wildarrows (36 %)."]);
    let r = review(&d, &table(), None, &data()).unwrap();
    assert_eq!(r.items[0].all, Some((592, 778)), "an unlisted first item gets the slot's times");
    assert!(tips(&d, Some(&coachless(0, STATIKK))).is_empty());
    let mut lolalytics = coachless(0, STATIKK);
    lolalytics.coachless = None;
    assert_eq!(tips(&d, Some(&lolalytics)).len(), 1, "only a Coachless build counts");
}

#[test]
fn a_rare_second_item_after_a_listed_start_is_an_order_tip() {
    let d = detail(JINX, PHYSICAL, &usual_jinx(START_A, STATIKK), (400, 400));
    assert_eq!(tips(&d, None), ["Statikk Shiv second after Bloodsong Edge is rare on Jinx bottom; Emerald+ go Runaan's Hurricane (63 %) or Infinity Edge (13 %)."]);
    assert!(tips(&d, Some(&coachless(1, STATIKK))).is_empty());
}

#[test]
fn the_overall_item_in_a_listed_situation_is_a_situation_tip() {
    let buys = [(0.0, 1055), (660.0, YUN_TAL), (1030.0, INFINITY_EDGE)];
    let mixed = detail(XAYAH, MIXED, &buys, (300, 400));
    assert_eq!(tips(&mixed, None), ["Into mixed damage, Emerald+ go Rapid Firecannon second (45 %), not Infinity Edge."]);
    let even = detail(XAYAH, PHYSICAL, &buys, (400, 400));
    assert_eq!(tips(&even, None), ["When even in gold, Emerald+ go Rapid Firecannon second (50 %), not Infinity Edge."]);
    assert!(tips(&detail(XAYAH, MAGIC, &buys, (300, 400)), None).is_empty(), "behind into magic damage is no listed situation");
    assert!(tips(&mixed, Some(&coachless(1, INFINITY_EDGE))).is_empty());
    let took_it = [(0.0, 1055), (660.0, YUN_TAL), (1030.0, FIRECANNON)];
    assert!(tips(&detail(XAYAH, MIXED, &took_it, (300, 400)), None).is_empty());
}

#[test]
fn a_late_item_is_a_timing_tip_against_its_own_times() {
    let mut buys = usual_jinx(START_A, RUNAAN);
    buys[2].0 = 850.0;
    assert_eq!(tips(&detail(JINX, PHYSICAL, &buys, (400, 400)), None), ["First item at 14:10; Emerald+ finish it between 9:49 and 12:58."]);
    let mut buys = usual_jinx(START_A, RUNAAN);
    buys[3].0 = 900.0;
    assert_eq!(tips(&detail(JINX, PHYSICAL, &buys, (400, 400)), None), ["Boots at 15:00; Emerald+ finish them between 9:12 and 14:13."]);
}

#[test]
fn a_late_first_back_with_gold_in_pocket_is_a_tip_and_two_tips_at_most() {
    let mut buys = usual_jinx(START_A, RUNAAN);
    buys[1].0 = 410.0;
    let d = detail(JINX, PHYSICAL, &buys, (400, 400));
    let mut poor = d.clone();
    poor.frames.iter_mut().for_each(|f| f.players[ME].current_gold = 800);
    assert!(tips(&poor, None).is_empty(), "late but spending all");
    let mut rich = d.clone();
    rich.frames[6].players[ME].current_gold = 1650;
    assert_eq!(tips(&rich, None), ["First back at 6:50 with 1,650 gold; Emerald+ go back between 2:44 and 4:00, most often for Cull."]);
    let mut all = rich.clone();
    all.purchases[2] = Purchase { time: 850.0, item: STATIKK };
    let t = tips(&all, None);
    assert_eq!(t.len(), 2);
    assert!(t[0].starts_with("Statikk Shiv first is rare") && t[1].starts_with("First item at 14:10"), "{t:?}");
}

#[test]
fn no_section_without_the_pair_in_swiftplay_or_off_the_rift() {
    let (t, sd) = (table(), data());
    let d = detail(JINX, PHYSICAL, &usual_jinx(START_A, RUNAAN), (400, 400));
    assert!(review(&d, &t, None, &sd).is_some());
    let mut ahri = detail(AHRI, PHYSICAL, &usual_jinx(START_A, RUNAAN), (400, 400));
    ahri.game.champ = AHRI;
    assert!(review(&ahri, &t, None, &sd).is_none());
    let swift = MatchDetail { game: game(JINX, 480, Mode::Rift), ..d.clone() };
    assert!(review(&swift, &t, None, &sd).is_none());
    let aram = MatchDetail { game: game(JINX, 450, Mode::Aram), ..d.clone() };
    assert!(review(&aram, &t, None, &sd).is_none());
    let mut laneless = d.clone();
    laneless.players[ME].lane = None;
    assert!(review(&laneless, &t, None, &sd).is_none());
    let none = review(&MatchDetail { purchases: vec![], ..d }, &t, None, &sd).unwrap();
    assert!(none.items.is_empty() && none.back.is_none() && none.tips.is_empty());
}

#[test]
fn the_pace_is_the_slot_of_the_next_finished_item() {
    let t = table();
    let at = |held: &[ItemId]| pace(&t, JINX, Some(Lane::Bottom), Mode::Rift, false, held).map(|p| (p.slot, p.early, p.median, p.late));
    assert_eq!(at(&[]), Some(("first item".into(), 592, 685, 778)));
    assert_eq!(at(&[START_A, BERSERKERS, 1036, 2003]), Some(("second item".into(), 951, 1052, 1147)), "boots and components do not count");
    assert_eq!(at(&[START_A, RUNAAN]), Some(("third item".into(), 1261, 1361, 1469)));
    assert_eq!(at(&[START_A, RUNAAN, INFINITY_EDGE]), None, "the table stops at the third");
}

#[test]
fn no_pace_without_a_table_entry_in_swiftplay_or_off_the_rift() {
    let t = table();
    assert!(pace(&t, AHRI, Some(Lane::Middle), Mode::Rift, false, &[]).is_none());
    assert!(pace(&t, JINX, None, Mode::Rift, false, &[]).is_none());
    assert!(pace(&t, JINX, Some(Lane::Bottom), Mode::Rift, true, &[]).is_none());
    assert!(pace(&t, JINX, Some(Lane::Bottom), Mode::Aram, false, &[]).is_none());
}
