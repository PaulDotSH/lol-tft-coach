//! Mode rules, augment-aware items, picked-augment detection, gold sinks, resists.

mod common;

use coach::{AugmentTracker, GameInput, game_advice};
use common::*;
use model::*;

const NASHORS: ItemId = 3115;
const WITS_END: ItemId = 3091;
const RITE_OF_RUIN: ItemId = 123430;
const STAT_ANVIL: ItemId = 6032;
const ELIXIR_SORCERY: ItemId = 2139;
const ELIXIR_WRATH: ItemId = 2140;

// Augment ids (u.gg / CommunityDragon).
const WITCHFUL: AugmentId = 1097; // +70 AP
const JUICED: AugmentId = 2016; // on-hit, no instant stats
const DEFT: AugmentId = 1022; // +60 % attack speed
const RECURSION: AugmentId = 1068; // +60 haste
const EUREKA: AugmentId = 1030; // haste from AP (no fixed value)
const SCOPED: AugmentId = 1170; // +50 range
const SCOPIER: AugmentId = 1071; // +100 range
const WOOGLETS: AugmentId = 1156; // grants a Needlessly Large Rod
const LEG_DAY: AugmentId = 1305;
const TITANS: AugmentId = 1001; // tenacity

fn extra_items(sd: &mut StaticData) {
    let mk = |id: ItemId, name: &str, cost: u32, stats: ItemStats, tags: &[&str], completed: bool| Item {
        id,
        name: name.into(),
        cost,
        from: vec![],
        into: vec![],
        stats,
        answers: Profile::default(),
        boots: false,
        completed,
        rift: true,
        aram: true,
        tags: tags.iter().map(|t| t.to_string()).collect(),
    };
    let s = ItemStats::default;
    for it in [
        mk(NASHORS, "Nashor's Tooth", 2900, ItemStats { ap: 80.0, attack_speed: 0.5, ability_haste: 15.0, ..s() }, &["OnHit"], true),
        mk(WITS_END, "Wit's End", 2800, ItemStats { mr: 45.0, attack_speed: 0.55, ..s() }, &["OnHit"], true),
        {
            let mut i = mk(RITE_OF_RUIN, "Rite of Ruin", 3000, ItemStats { ap: 55.0, crit: 0.25, ability_haste: 10.0, ..s() }, &[], true);
            i.rift = false;
            i
        },
        mk(STAT_ANVIL, "Stat Bonus", 750, s(), &["Consumable"], false),
        mk(ELIXIR_SORCERY, "Elixir of Sorcery", 500, s(), &["Consumable"], false),
        mk(ELIXIR_WRATH, "Elixir of Wrath", 500, s(), &["Consumable"], false),
    ] {
        sd.items.insert(it.id, it);
    }
}

fn sd_x() -> StaticData {
    let mut sd = sd();
    extra_items(&mut sd);
    sd
}

fn advise(sd: &StaticData, snap: &GameSnapshot, stats: Option<&ChampStats>, mine: &[MyAugment]) -> GameAdvice {
    game_advice(&GameInput { snap, sd, stats, augments: None, offered: &[], gold_history: &[], my_augments: mine, enemy_augments: Default::default() })
}

fn picked(id: AugmentId) -> MyAugment {
    MyAugment { id: Some(id), candidates: vec![id], time: 300.0 }
}

fn ids(a: &GameAdvice) -> Vec<ItemId> {
    a.buy_next.iter().chain(&a.alternatives).chain(&a.path).map(|p| p.item).collect()
}

fn mayhem(mut s: GameSnapshot) -> GameSnapshot {
    s.mode = Mode::Mayhem;
    s.map = 12;
    s
}

fn live(ap: f32) -> ChampionStats {
    ChampionStats {
        ability_power: ap,
        ability_haste: 10.0,
        attack_damage: 60.0,
        attack_range: 550.0,
        attack_speed: 0.7,
        armor: 40.0,
        magic_resist: 35.0,
        max_health: 1500.0,
        move_speed: 340.0,
        ..Default::default()
    }
}

fn set_base(sd: &mut StaticData, champ: ChampKey, armor: f32, armor_lvl: f32, mr: f32, range: f32) {
    let c = sd.champs.get_mut(&champ).unwrap();
    c.base = BaseStats { hp: 600.0, hp_per_level: 100.0, armor, armor_per_level: armor_lvl, mr, mr_per_level: 2.0, attack_range: range, attack_speed: 0.65 };
}

// ---- Q4 mode rules ----

#[test]
fn mayhem_combo_breaker_lowers_tenacity_boots() {
    // Rift: this CC + AP lobby makes Mercury's Treads buy next (game.rs). Mayhem halves tenacity need.
    let mut p = lobby([(MALPHITE, Lane::Top), (THRESH, Lane::Jungle), (LUX, Lane::Middle), (SYNDRA, Lane::Bottom), (LEONA, Lane::Support)]);
    p[0] = p[0].clone().items(&[LUDENS]);
    p[7] = p[7].clone().kda(8, 1, 4).lvl(11).items(&[LUDENS]);
    let sd = sd_x();
    let rift = snap(p, killed_me(7, 2));
    let score = |a: &GameAdvice| a.buy_next.iter().chain(&a.alternatives).find(|x| x.item == MERCS).map(|x| x.score);
    let r = advise(&sd, &rift, Some(&ahri_stats()), &[]);
    let m = advise(&sd, &mayhem(rift.clone()), Some(&ahri_stats()), &[]);
    assert_eq!(r.buy_next.as_ref().unwrap().item, MERCS);
    assert!(score(&m).unwrap_or(0.0) < score(&r).unwrap(), "{:?}", m.buy_next);
}

#[test]
fn mayhem_only_items_offered_in_mayhem_not_aram() {
    let mut sd = sd_x();
    sd.items.get_mut(&RITE_OF_RUIN).unwrap().answers.anti_burst = 1.0; // make it attractive vs Zed
    let mut s = snap(lobby(STANDARD), killed_me(7, 3));
    s.players[7] = s.players[7].clone().kda(12, 1, 3).lvl(11).items(&[YOUMUUS, EDGE_OF_NIGHT]);
    s.mode = Mode::Aram;
    assert!(!ids(&advise(&sd, &s, None, &[])).contains(&RITE_OF_RUIN));
    assert!(ids(&advise(&sd, &mayhem(s), None, &[])).contains(&RITE_OF_RUIN));
}

#[test]
fn crit_past_100_is_wasted_on_the_rift_but_not_in_mayhem() {
    let sd = sd_x();
    let mut p = lobby(HEALERS);
    p[0] = player(CAITLYN, Team::Order, Some(Lane::Bottom)).items(&[INFINITY_EDGE, BERSERKERS]);
    p[3] = player(AHRI, Team::Order, Some(Lane::Middle));
    let mut s = snap(p, vec![]);
    s.my_stats = Some(ChampionStats { crit_chance: 1.0, attack_speed: 1.2, max_health: 2000.0, ..Default::default() });
    let score = |s: &GameSnapshot, crit: f32| {
        let mut s = s.clone();
        s.my_stats.as_mut().unwrap().crit_chance = crit;
        let a = advise(&sd, &s, None, &[]);
        // Same decision (the next slot) at both crit levels.
        a.buy_next.iter().chain(&a.alternatives).find(|x| x.item == MORTAL_REMINDER).map_or(0.0, |x| x.score)
    };
    // Mortal Reminder's 25 % crit: wasted at 100 % on the Rift, converted in Mayhem.
    assert!(score(&s, 1.0) < score(&s, 0.5) - 0.02, "{} {}", score(&s, 1.0), score(&s, 0.5));
    let m = mayhem(s);
    assert!((score(&m, 1.0) - score(&m, 0.5)).abs() < 1e-6);
}

#[test]
fn attack_speed_past_the_cap_is_penalised() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0] = s.players[0].clone().items(&[LUDENS, SORCS]);
    let nashors = |a: &GameAdvice| a.buy_next.iter().chain(&a.alternatives).chain(&a.path).find(|x| x.item == NASHORS).map(|x| x.score);
    let juiced = [picked(JUICED)]; // keeps Nashor's in the path
    s.my_stats = Some(ChampionStats { attack_speed: 1.0, ..live(300.0) });
    let low = nashors(&advise(&sd, &s, None, &juiced)).unwrap();
    s.my_stats = Some(ChampionStats { attack_speed: 5.0, ..live(300.0) });
    let capped = nashors(&advise(&sd, &s, None, &juiced)).unwrap_or(0.0);
    assert!(capped < low, "{capped} {low}");
}

// ---- V2 augment-aware items ----

#[test]
fn on_hit_augment_raises_nashors_and_names_it() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0] = s.players[0].clone().items(&[LUDENS, SORCS]);
    let pos = |a: &GameAdvice| a.path.iter().position(|p| p.item == NASHORS);
    let before = advise(&sd, &s, None, &[]);
    let after = advise(&sd, &s, None, &[picked(JUICED)]);
    assert!(pos(&after).unwrap() < pos(&before).unwrap_or(99), "{:?} vs {:?}", after.path, before.path);
    let n = after.path.iter().find(|p| p.item == NASHORS).unwrap();
    assert!(n.reason.contains("with Juiced (on-hit)"), "{}", n.reason);
}

#[test]
fn haste_augment_lowers_haste_items() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0] = s.players[0].clone().items(&[LUDENS, SORCS]);
    // Nashor's has 15 haste: Juiced raises it, Recursion's haste cancels that.
    let score = |a: &GameAdvice| a.path.iter().find(|x| x.item == NASHORS).map_or(0.0, |x| x.score);
    let before = advise(&sd, &s, None, &[picked(JUICED)]);
    let after = advise(&sd, &s, None, &[picked(JUICED), picked(RECURSION)]);
    assert!(score(&after) < score(&before) - 0.02, "{} {}", score(&after), score(&before));
}

#[test]
fn tenacity_augment_drops_mercs() {
    let sd = sd_x();
    let mut p = lobby([(MALPHITE, Lane::Top), (THRESH, Lane::Jungle), (LUX, Lane::Middle), (SYNDRA, Lane::Bottom), (LEONA, Lane::Support)]);
    p[0] = p[0].clone().items(&[LUDENS]);
    let s = snap(p, vec![]);
    let mercs = |a: &GameAdvice| a.buy_next.iter().chain(&a.alternatives).find(|x| x.item == MERCS).map(|x| x.score);
    let base = advise(&sd, &s, Some(&ahri_stats()), &[]);
    let titan = advise(&sd, &s, Some(&ahri_stats()), &[picked(TITANS)]);
    assert!(mercs(&titan).unwrap_or(0.0) < mercs(&base).unwrap(), "{:?}", titan.buy_next);
}

#[test]
fn quest_augment_raises_its_items_and_keeps_a_slot() {
    let sd = sd_x();
    let s = mayhem(snap(lobby(STANDARD), vec![]));
    let a = advise(&sd, &s, None, &[picked(WOOGLETS)]);
    let path: Vec<_> = a.path.iter().map(|p| p.item).collect();
    assert_eq!(path.len(), 5, "one slot kept for Wooglet's Witchcap: {path:?}");
    assert!(path[..3].contains(&RABADONS) && path[..3].contains(&ZHONYAS), "{path:?}");
    let r = a.path.iter().find(|p| p.item == RABADONS).unwrap();
    assert!(r.reason.contains("Wooglet's Witchcap (quest item)"), "{}", r.reason);
}

#[test]
fn unresolved_pick_with_shared_tag_still_counts() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0] = s.players[0].clone().items(&[LUDENS, SORCS]);
    let unsure = MyAugment { id: None, candidates: vec![JUICED, 1087], time: 300.0 }; // Juiced or Typhoon
    let a = advise(&sd, &s, None, &[unsure]);
    let n = a.path.iter().find(|p| p.item == NASHORS).unwrap();
    assert!(n.reason.contains("your on-hit augment"), "{}", n.reason);
}

#[test]
fn augment_matching_owned_items_gets_tie_break_reason() {
    let sd = sd_x();
    let gold = [WITCHFUL, JUICED].into_iter().chain(9000..9038).collect();
    let ranks = ChampAugments { champ: AHRI, prismatic: vec![], gold, silver: vec![] };
    let md = MayhemData { patch: "16_21".into(), augment_names: [(JUICED, "Juiced".into()), (WITCHFUL, "Witchful Thinking".into())].into(), champ_tiers: Default::default() };
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    let run = |s: &GameSnapshot| {
        game_advice(&GameInput { snap: s, sd: &sd, stats: None, augments: Some((&ranks, &md)), offered: &[JUICED, WITCHFUL], gold_history: &[], my_augments: &[], enemy_augments: Default::default() })
            .augments
            .unwrap()
    };
    assert_eq!(run(&s).best, Some(WITCHFUL), "rank order without items");
    s.players[0] = s.players[0].clone().items(&[WITS_END]);
    let a = run(&s);
    let j = a.offered.iter().find(|r| r.id == JUICED).unwrap();
    assert_eq!(j.synergy.as_deref(), Some("fits your Wit's End (on-hit)"));
    assert_eq!(a.offered.iter().find(|r| r.id == WITCHFUL).unwrap().synergy, None);
    assert_eq!(j.rank, Some(1), "rank shown unchanged");
    assert_eq!(a.best, Some(JUICED));
    assert!(a.reason.ends_with("for Ahri; fits your Wit's End (on-hit)"), "{}", a.reason);
}

// ---- V3 picked-augment detection ----

/// Offer shown, then gone after `change` was applied to my snapshot.
fn pick(offer: &[AugmentId], change: impl Fn(&mut GameSnapshot)) -> Vec<MyAugment> {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0].level = 3;
    s.my_gold = 1400.0;
    s.my_stats = Some(live(100.0));
    let mut t = AugmentTracker::default();
    t.observe(offer, &s, &sd);
    t.observe(offer, &s, &sd);
    change(&mut s);
    t.observe(&[], &s, &sd);
    t.picks
}

#[test]
fn flat_stat_gain_identifies_the_pick() {
    let p = pick(&[JUICED, WITCHFUL, DEFT], |s| s.my_stats.as_mut().unwrap().ability_power += 70.0);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].id, Some(WITCHFUL));
    let p = pick(&[JUICED, WITCHFUL, DEFT], |s| s.my_stats.as_mut().unwrap().attack_speed += 0.6 * 0.65);
    assert_eq!(p[0].id, Some(DEFT), "{p:?}");
}

#[test]
fn no_stat_change_rules_out_stat_cards_only() {
    let p = pick(&[JUICED, WITCHFUL, DEFT], |_| {});
    assert_eq!((p[0].id, p[0].candidates.clone()), (None, vec![JUICED]), "Juiced is the only card without a visible stat");
    let p = pick(&[WITCHFUL, DEFT, RECURSION], |_| {});
    assert_eq!(p[0].id, None);
    assert_eq!(p[0].candidates, vec![WITCHFUL, DEFT, RECURSION], "nothing fits: shop closed without picking?");
}

#[test]
fn rerolled_away_card_is_not_a_candidate() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0].level = 3;
    s.my_stats = Some(live(100.0));
    let mut t = AugmentTracker::default();
    t.observe(&[JUICED, WITCHFUL, DEFT], &s, &sd);
    t.observe(&[WITCHFUL, DEFT], &s, &sd); // reroll flip, read mid-animation
    t.observe(&[RECURSION, WITCHFUL, DEFT], &s, &sd);
    t.observe(&[], &s, &sd);
    assert!(!t.picks[0].candidates.contains(&JUICED), "{:?}", t.picks);
    assert!(t.picks[0].candidates.contains(&RECURSION), "{:?}", t.picks);
}

#[test]
fn closer_magnitude_breaks_a_tie() {
    let p = pick(&[SCOPED, SCOPIER, JUICED], |s| s.my_stats.as_mut().unwrap().attack_range += 100.0);
    assert_eq!(p[0].id, Some(SCOPIER));
}

#[test]
fn rival_tag_only_card_keeps_it_ambiguous() {
    // Eureka also gives stats on the same family (haste from AP) without a fixed value.
    let p = pick(&[RECURSION, EUREKA, JUICED], |s| s.my_stats.as_mut().unwrap().ability_haste += 60.0);
    assert_eq!(p[0].id, None);
    assert_eq!(p[0].candidates, vec![RECURSION, EUREKA]);
}

#[test]
fn item_bought_during_the_offer_blurs_its_stats() {
    let p = pick(&[JUICED, WITCHFUL, DEFT], |s| {
        s.players[0].items.push(RABADONS);
        s.my_gold -= 3500.0;
        s.my_stats.as_mut().unwrap().ability_power += 130.0 + 70.0;
    });
    assert_eq!(p[0].id, None);
    assert!(p[0].candidates.contains(&WITCHFUL) && !p[0].candidates.contains(&DEFT), "{p:?}");
}

#[test]
fn granted_item_identifies_a_quest_pick() {
    let p = pick(&[JUICED, WOOGLETS, DEFT], |s| s.players[0].items.push(ROD));
    assert_eq!(p[0].id, Some(WOOGLETS));
    // Bought the Rod for 1200 instead: not a grant.
    let p = pick(&[JUICED, WOOGLETS, DEFT], |s| {
        s.players[0].items.push(ROD);
        s.my_gold -= 1200.0;
    });
    assert_ne!(p[0].id, Some(WOOGLETS));
}

#[test]
fn reopened_offer_undoes_the_unresolved_close_and_caps_by_level() {
    let sd = sd_x();
    let mut s = mayhem(snap(lobby(STANDARD), vec![]));
    s.players[0].level = 3;
    s.my_stats = Some(live(100.0));
    let mut t = AugmentTracker::default();
    let offer = [WITCHFUL, DEFT, RECURSION];
    t.observe(&offer, &s, &sd);
    t.observe(&[], &s, &sd); // shop closed, nothing picked
    assert_eq!(t.picks.len(), 1);
    t.observe(&offer, &s, &sd); // same offer again
    assert!(t.picks.is_empty());
    s.my_stats.as_mut().unwrap().ability_haste += 60.0;
    t.observe(&[], &s, &sd);
    assert_eq!(t.picks.iter().map(|p| p.id).collect::<Vec<_>>(), vec![Some(RECURSION)]);

    // A different offer closing at level 3 cannot be a second pick: replaces the last.
    t.observe(&[JUICED, SCOPED], &s, &sd);
    t.observe(&[], &s, &sd);
    assert_eq!(t.picks.len(), 1);
    s.players[0].level = 7;
    t.observe(&[LEG_DAY, DEFT], &s, &sd);
    s.my_stats.as_mut().unwrap().move_speed += 50.0;
    t.observe(&[], &s, &sd);
    assert_eq!(t.picks.len(), 2);
    assert_eq!(t.picks[1].id, Some(LEG_DAY));
}

#[test]
fn no_live_stats_means_unknown() {
    let p = pick(&[JUICED, WITCHFUL], |s| s.my_stats = None);
    assert_eq!(p[0].id, None);
}

// ---- V4 gold sinks ----

fn full_build(level: u32) -> GameSnapshot {
    let mut s = mayhem(snap(lobby(STANDARD), killed_me(7, 3)));
    s.players[7] = s.players[7].clone().kda(12, 1, 3).lvl(13).items(&[YOUMUUS, EDGE_OF_NIGHT, LONG_SWORD]);
    s.players[0] = s.players[0].clone().lvl(level).items(&[LUDENS, RABADONS, VOID_STAFF, MORELLO, BANSHEES, SORCS]);
    s.my_gold = 3000.0;
    s
}

#[test]
fn full_build_offers_anvil_elixir_and_a_swap() {
    let sd = sd_x();
    let a = advise(&sd, &full_build(11), Some(&ahri_stats()), &[]);
    assert!(a.buy_next.is_none() && a.path.is_empty());
    let kinds: Vec<&str> = a.gold_sinks.iter().map(|g| g.kind.as_str()).collect();
    assert_eq!(kinds, ["anvil", "elixir", "swap"], "{:#?}", a.gold_sinks);
    assert_eq!((a.gold_sinks[0].item, a.gold_sinks[0].cost), (Some(STAT_ANVIL), 750));
    assert_eq!(a.gold_sinks[1].item, Some(ELIXIR_SORCERY), "AP champion");
    let swap = &a.gold_sinks[2];
    assert_eq!(swap.item, Some(ZHONYAS), "stasis vs a fed Zed: {swap:?}");
    assert!(swap.sell.is_some() && swap.reason.contains("Zed"), "{swap:?}");

    let a = advise(&sd, &full_build(7), Some(&ahri_stats()), &[]);
    assert!(a.gold_sinks.iter().all(|g| g.kind == "swap"), "anvils and elixirs from level 9");
    let mut rift = full_build(11);
    rift.mode = Mode::Rift;
    assert!(advise(&sd, &rift, Some(&ahri_stats()), &[]).gold_sinks.iter().all(|g| g.kind != "anvil"));
}

#[test]
fn move_speed_augment_suggests_selling_boots() {
    let sd = sd_x();
    let a = advise(&sd, &full_build(11), Some(&ahri_stats()), &[picked(LEG_DAY)]);
    let b = a.gold_sinks.iter().find(|g| g.kind == "sell_boots").expect("sell boots");
    assert_eq!(b.sell, Some(SORCS));
    assert!(b.reason.starts_with("Leg Day covers move speed: sell Sorcerer's Shoes for"), "{}", b.reason);
    assert_eq!(b.cost, sd.items[&b.item.unwrap()].cost - 770);
}

#[test]
fn no_sinks_before_the_build_is_full() {
    let sd = sd_x();
    let mut s = full_build(11);
    s.players[0].items.truncate(3);
    assert!(advise(&sd, &s, Some(&ahri_stats()), &[]).gold_sinks.is_empty());
}

// ---- V5 resists and EHP ----

fn resist_lobby(sd: &mut StaticData) -> GameSnapshot {
    for (c, armor, range) in [(DARIUS, 39.0, 175.0), (LEE_SIN, 36.0, 125.0), (ZED, 32.0, 125.0), (JINX, 26.0, 525.0), (THRESH, 33.0, 450.0)] {
        set_base(sd, c, armor, 4.5, 30.0, range);
    }
    let mut s = snap(lobby(STANDARD), vec![]);
    for p in s.players.iter_mut().skip(5) {
        p.level = 11;
        p.items.clear();
    }
    s
}

#[test]
fn enemy_resists_from_level_items_and_mode_bonus() {
    let mut sd = sd_x();
    let mut s = resist_lobby(&mut sd);
    s.players[5].items = vec![THORNMAIL];
    s.my_stats = Some(live(200.0));
    let d = advise(&sd, &s, None, &[]).defense.unwrap();
    let darius = d.enemies.iter().find(|e| e.champ == DARIUS).unwrap();
    let grown = 39.0 + 4.5 * 10.0 * (0.7025 + 0.0175 * 10.0);
    assert!((darius.armor - (grown + 75.0)).abs() < 0.01, "{darius:?}");
    let s = mayhem(s);
    let d = advise(&sd, &s, None, &[]).defense.unwrap();
    let get = |c| d.enemies.iter().find(|e| e.champ == c).unwrap().armor;
    assert!((get(DARIUS) - (grown + 75.0 + 15.0)).abs() < 0.01, "melee +15 in Mayhem");
    let jinx = 26.0 + 4.5 * 10.0 * (0.7025 + 0.0175 * 10.0);
    assert!((get(JINX) - jinx).abs() < 0.01, "ranged get none (wiki: bug)");
}

#[test]
fn ehp_split_and_enemy_damage_mix() {
    let mut sd = sd_x();
    let mut s = resist_lobby(&mut sd);
    s.my_stats = Some(live(200.0)); // 1500 HP, 40 armor, 35 MR
    let d = advise(&sd, &s, None, &[]).defense.unwrap();
    assert_eq!(d.ehp_physical.map(f32::round), Some(2100.0));
    assert_eq!(d.ehp_magic.map(f32::round), Some(2025.0));
    assert!(d.enemy_magic < 0.3 && d.enemy_magic + d.enemy_true <= 1.0, "STANDARD lobby is mostly AD: {d:?}");
    s.my_stats = None;
    let d = advise(&sd, &s, None, &[]).defense.unwrap();
    assert_eq!((d.ehp_physical, d.ehp_magic), (None, None));
}

#[test]
fn pen_note_and_choice_follow_enemy_armor() {
    let mut sd = sd_x();
    let mut s = resist_lobby(&mut sd);
    s.players[0] = player(CAITLYN, Team::Order, Some(Lane::Bottom)).items(&[INFINITY_EDGE, BERSERKERS]);
    s.players[3] = player(AHRI, Team::Order, Some(Lane::Middle));
    s.players.iter_mut().skip(5).for_each(|p| p.level = 3); // armor ~35: under the ~51 break-even
    let squishy = advise(&sd, &s, None, &[]).defense.unwrap();
    assert_eq!(squishy.pen_notes.len(), 1, "physical champion: armor note only {:?}", squishy.pen_notes);
    assert!(squishy.pen_notes[0].ends_with("lethality beats % armor pen"), "{:?}", squishy.pen_notes);
    for p in s.players.iter_mut().skip(5) {
        p.level = 18;
        p.items = vec![THORNMAIL, RANDUINS];
    }
    let a = advise(&sd, &s, None, &[]);
    let tanky = a.defense.unwrap();
    assert!(tanky.pen_notes[0].ends_with("% armor pen beats lethality"), "{:?}", tanky.pen_notes);
    let ld = a.buy_next.iter().chain(&a.alternatives).chain(&a.path).find(|p| p.item == LORD_DOMINIKS).expect("LDR vs armor stacking");
    assert!(ld.reason.contains("armor"), "{}", ld.reason);
}

#[test]
fn unknown_base_stats_keep_static_pen_answers() {
    let sd = sd_x();
    let s = snap(lobby(STANDARD), vec![]);
    let d = advise(&sd, &s, None, &[]).defense.unwrap();
    assert!(d.enemies.is_empty() && d.pen_notes.is_empty());
}

