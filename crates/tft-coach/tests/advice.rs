//! Live advice: target comp fit and stickiness, shop reasons, econ timings, items, augments,
//! contested units from scouting, and call keys.

mod common;

use common::*;
use tft_coach::*;
use tft_model::*;

fn tracker(stage: (u32, u32), units: &[&str]) -> Tracker {
    let mut t = Tracker::default();
    own(&mut t, &set(), stage, units);
    t
}

/// Defender Rengar is the target with these.
const RENGAR_UNITS: [&str; 4] = [RENGAR, TARIC, LEONA, GAREN];
/// Sorcerer Ahri (reroll at 7) is the target with these.
const AHRI_UNITS: [&str; 3] = [AHRI, LUX, MORGANA];

fn scout(t: &mut Tracker, s: &TftSetData, stage: (u32, u32)) {
    for tr in [&[(DEFENDER, 4), (DUELIST, 2), (GREENFATHER, 1)][..], &[(DEFENDER, 2), (DUELIST, 2), (SORCERER, 2), (COVEN, 2), (BRUISER, 1)]] {
        t.observe(&TftSnapshot { viewing_other: true, traits: traits(tr), ..snap(stage, 30) }, s);
    }
}

#[test]
fn the_target_is_the_best_fit_and_sticks_within_a_margin() {
    let (s, m) = (set(), meta());
    let mut t = tracker((3, 1), &[TARIC, LEONA]);
    let now = snap((3, 1), 20);
    let a = advise_with(&now, &t, &s, &m, None);
    assert_eq!(ids(&a.comps), [TARIC_COMP, RENGAR_COMP, AHRI_COMP]);
    assert_eq!((a.comps[0].fit, a.comps[0].have.len(), a.comps[0].missing.len()), (0.5, 2, 2));
    // Last tick's target is kept exactly while it is within the keep margin of the best.
    let input = CoachInput { snap: &now, tracker: &t, set: &s, meta: &m, hooks: &StandardHooks, keep: None };
    let ranked = fit::rank(&input);
    let gap = ranked[0].1 - ranked.iter().find(|c| c.0.id == RENGAR_COMP).unwrap().1;
    let kept = advise_with(&now, &t, &s, &m, Some(RENGAR_COMP)).comps[0].comp == RENGAR_COMP;
    assert_eq!(kept, gap <= fit::keep_margin(Some((3, 1))), "gap {gap}");
    // Two more Rengar units: it clearly wins, the old target goes.
    own(&mut t, &s, (3, 1), &[RENGAR, GAREN]);
    assert_eq!(advise_with(&now, &t, &s, &m, Some(TARIC_COMP)).comps[0].comp, RENGAR_COMP);
    assert!(a.owned.iter().any(|u| u.api == TARIC && u.star == 1));
}

#[test]
fn shop_slots_get_buy_or_skip_with_a_reason() {
    let (s, m) = (set(), meta());
    let mut t = tracker((4, 1), &[GAREN, GAREN, RENGAR, TARIC, LEONA]);
    scout(&mut t, &s, (4, 1));
    let slots = shop(&[Some(GAREN), Some(RENGAR), Some(LUX), Some(AHRI), None]);
    let a = tick(&mut t, &s, &m, &TftSnapshot { shop: slots.clone(), ..snap((4, 1), 30) });
    assert_eq!(a.comps[0].comp, RENGAR_COMP);
    let buys: Vec<bool> = a.shop.iter().map(|x| x.buy).collect();
    assert_eq!(buys, [true, true, false, false]);
    assert!(a.shop[0].reason.starts_with("makes ★2 Garen"), "{}", a.shop[0].reason);
    assert_eq!(a.shop[1].reason, "for Defender Rengar, contested by 2 boards");
    assert_eq!(a.shop[1].contested, 2);
    assert!(a.shop[2].reason.starts_with("not in your comp"));

    // Earlier, a backup comp's units are still worth it; a unit I can't afford is not.
    let mut t = tracker((3, 1), &[GAREN, GAREN, RENGAR, TARIC, LEONA]);
    let a = tick(&mut t, &s, &m, &TftSnapshot { shop: slots, ..snap((3, 1), 3) });
    assert_eq!(a.shop[2].reason, "for Greenfather Taric (backup)");
    assert!(!a.shop[1].buy && a.shop[1].reason.ends_with("but it costs 4 (you have 3)"), "{}", a.shop[1].reason);
}

fn econ(stage: (u32, u32), units: &[&str], f: impl Fn(&mut TftSnapshot)) -> TftAdvice {
    let (s, m) = (set(), meta());
    let mut t = tracker(stage, units);
    let mut now = snap(stage, 0);
    f(&mut now);
    tick2(&mut t, &s, &m, &now)
}

#[test]
fn econ_follows_the_comp_levelling() {
    // A lvl 8 comp levels to 8 at 4-2, then rolls down there.
    let a = econ((4, 2), &RENGAR_UNITS, |n| (n.level, n.xp, n.gold) = (Some(7), Some((20, 36)), Some(50)));
    let e = a.econ.as_ref().unwrap();
    assert_eq!(e.action, EconAction::Level);
    assert!(e.text.starts_with("Level to 8 now: 4-2"), "{}", e.text);
    assert_eq!(keys(&a), ["level-8"]);
    let a = econ((4, 2), &RENGAR_UNITS, |n| (n.level, n.gold) = (Some(8), Some(40)));
    assert_eq!((a.econ.as_ref().unwrap().action, keys(&a)), (EconAction::Roll, vec!["roll-8"]));
    let a = econ((4, 1), &RENGAR_UNITS, |n| (n.level, n.gold) = (Some(7), Some(40)));
    let e = a.econ.unwrap();
    assert_eq!(e.action, EconAction::Save);
    assert!(e.text.contains("level to 8 at 4-2"), "{}", e.text);
    // Low HP: roll down now.
    let a = econ((3, 5), &RENGAR_UNITS, |n| (n.level, n.hp, n.gold) = (Some(6), Some(30), Some(30)));
    assert_eq!((a.econ.as_ref().unwrap().action, keys(&a)), (EconAction::Roll, vec!["roll-hp"]));
    // Interest.
    let a = econ((3, 1), &RENGAR_UNITS, |n| (n.level, n.gold) = (Some(5), Some(27)));
    assert_eq!(a.econ.unwrap().text, "Save: 27 gold, +2 interest; 3 more for +3");
    // Level unread: the team size's cap stands in.
    let a = econ((4, 2), &RENGAR_UNITS, |n| (n.team_size, n.gold) = (Some((7, 7)), Some(50)));
    assert_eq!(keys(&a), ["level-8"]);
    // Gold unread: the last read stands in; never read, no econ advice at all.
    let a = econ((3, 1), &RENGAR_UNITS, |n| (n.level, n.gold) = (Some(5), None));
    assert!(a.econ.is_some());
    let (s, m) = (set(), meta());
    let a = tick(&mut Tracker::default(), &s, &m, &TftSnapshot { level: Some(5), gold: None, ..snap((3, 1), 0) });
    assert!(a.econ.is_none());
}

#[test]
fn reroll_comps_roll_at_their_level() {
    let a = econ((4, 1), &AHRI_UNITS, |n| (n.level, n.gold) = (Some(6), Some(40)));
    assert_eq!(a.comps[0].comp, AHRI_COMP);
    assert_eq!((a.econ.as_ref().unwrap().action, keys(&a)), (EconAction::Level, vec!["level-7"]));
    let a = econ((4, 1), &AHRI_UNITS, |n| (n.level, n.gold) = (Some(7), Some(40)));
    assert_eq!((a.econ.as_ref().unwrap().action, keys(&a)), (EconAction::Roll, vec!["roll-7"]));
    // Later: slow roll the gold above 50, quietly.
    let a = econ((4, 5), &AHRI_UNITS, |n| (n.level, n.gold) = (Some(7), Some(70)));
    let e = a.econ.unwrap();
    assert_eq!(e.action, EconAction::Roll);
    assert!(e.text.contains("20 gold above 50"), "{}", e.text);
    assert!(a.calls.is_empty());
    let a = econ((4, 5), &AHRI_UNITS, |n| (n.level, n.gold) = (Some(7), Some(40)));
    assert_eq!(a.econ.unwrap().action, EconAction::Save);
}

#[test]
fn items_from_components_with_a_holder() {
    let (s, m) = (set(), meta());
    let mut t = tracker((3, 4), &AHRI_UNITS);
    let bench = |stage, hp, i: &[&str]| TftSnapshot { hp: Some(hp), items: items(i), ..snap(stage, 20) };
    let a = tick2(&mut t, &s, &m, &bench((3, 4), 80, &[ROD, ROD, GLOVE]));
    assert_eq!(a.items.len(), 1);
    let it = &a.items[0];
    assert_eq!((it.item.as_str(), it.holder.as_deref(), it.now), (RABADON, Some(AHRI), false));
    assert_eq!(it.from, [ROD, ROD]);
    assert!(it.reason.starts_with("best on Ahri in Sorcerer Ahri"), "{}", it.reason);
    // HP falling over two rounds: slam.
    tick2(&mut t, &s, &m, &bench((3, 5), 70, &[ROD, ROD, GLOVE]));
    let a = tick2(&mut t, &s, &m, &bench((3, 6), 60, &[ROD, ROD, GLOVE]));
    assert!(a.items[0].now && a.items[0].reason.contains("HP falling"), "{}", a.items[0].reason);
    let a = tick2(&mut t, &s, &m, &bench((3, 6), 40, &[ROD, GLOVE]));
    assert_eq!(a.items[0].item, JG);
    assert!(a.items[0].reason.ends_with("slam now: 40 HP"), "{}", a.items[0].reason);
    // A Rabadon's already on a unit: the rod goes into the gauntlet instead.
    let mut worn = bench((3, 6), 80, &[ROD, ROD, GLOVE]);
    worn.units = vec![SeenUnit { x: 900.0, y: 500.0, star: 1, bench: None, full: true, items: items(&[RABADON_RADIANT]), emb: vec![] }];
    assert_eq!(tick2(&mut t, &s, &m, &worn).items[0].item, JG);
    // A finished (radiant) item from loot just gets a holder.
    let a = tick2(&mut t, &s, &m, &bench((3, 6), 60, &[RABADON_RADIANT]));
    let it = &a.items[0];
    assert_eq!((it.item.as_str(), it.holder.as_deref(), it.now, it.from.len()), (RABADON_RADIANT, Some(AHRI), true, 0));
}

#[test]
fn augments_ranked_by_stats_or_comp_lists_or_not_at_all() {
    let (s, mut m) = (set(), meta());
    let mut t = tracker((2, 1), &RENGAR_UNITS);
    let offer = |o: &[&str]| TftSnapshot { augment_offer: items(o), ..snap((2, 1), 10) };
    let order = |a: &TftAdvice| a.augments.iter().map(|r| r.augment.clone()).collect::<Vec<_>>();
    // No stats: the target comp's list.
    let a = tick(&mut t, &s, &m, &offer(&[AUG_TANK, AUG_ECON, AUG_DUEL]));
    assert_eq!(order(&a), [AUG_TANK, AUG_DUEL]);
    assert!(keys(&a).contains(&"augment-DA_Aug_Tank"));
    // Stats: placement first, the comp's list as a bonus.
    m.augments = vec![
        PlaceStat { api: AUG_ECON.into(), games: 2000, avg_place: 3.6, top4: 0.6 },
        PlaceStat { api: AUG_TANK.into(), games: 2000, avg_place: 4.3, top4: 0.5 },
    ];
    let a = tick(&mut t, &s, &m, &offer(&[AUG_TANK, AUG_ECON, AUG_DUEL]));
    assert_eq!(order(&a), [AUG_ECON, AUG_TANK, AUG_DUEL]);
    assert!(a.augments[1].reason.contains("good in Defender Rengar"));
    // No data for what's offered: no ranking, no call.
    m.augments.clear();
    let a = tick(&mut t, &s, &m, &offer(&[AUG_ECON]));
    assert!(a.augments.is_empty() && !keys(&a).iter().any(|k| k.starts_with("augment")));
}

#[test]
fn contested_units_from_scouting_decay() {
    let (s, m) = (set(), meta());
    let mut t = tracker((4, 1), &RENGAR_UNITS);
    scout(&mut t, &s, (4, 1));
    let a = tick(&mut t, &s, &m, &snap((4, 1), 30));
    // Trait panels only (no units read): who plays it, no copies; the pool less my copy.
    assert_eq!(a.contested[0], Contest { unit: RENGAR.into(), boards: 2, held: 0, left: Some(9) });
    assert!(a.contested.contains(&Contest { unit: TARIC.into(), boards: 1, held: 0, left: Some(8) })); // its unique trait shows
    assert!(keys(&a).contains(&"contested-DA_18_Rengar"));
    // Not seen for more than two rounds: forgotten.
    let a = tick(&mut t, &s, &m, &snap((4, 4), 30));
    assert!(a.contested.is_empty() && a.calls.is_empty());
}

#[test]
fn call_keys_stay_the_same_while_the_situation_lasts() {
    let (s, m) = (set(), meta());
    let mut t = tracker((4, 2), &RENGAR_UNITS);
    let lv = |gold, xp| TftSnapshot { level: Some(7), xp: Some(xp), augment_offer: items(&[AUG_TANK, AUG_DUEL]), ..snap((4, 2), gold) };
    let a = tick(&mut t, &s, &m, &lv(50, (20, 36)));
    let b = tick(&mut t, &s, &m, &lv(46, (24, 36)));
    assert_eq!(keys(&a), ["level-8", "augment-DA_Aug_Tank"]);
    assert_eq!(keys(&a), keys(&b));
}
