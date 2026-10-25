//! Owned units from shop purchases, ★ from copies, trait-panel correction, sells, unreadable
//! fields, new games and scouted boards.

mod common;

use common::*;
use tft_coach::tracker::*;
use tft_model::*;

fn observe(t: &mut Tracker, s: &TftSetData, stage: (u32, u32), gold: Option<u32>, slots: &[Option<&str>]) {
    t.observe(&TftSnapshot { gold, shop: shop(slots), ..snap(stage, 0) }, s);
}

#[test]
fn buys_from_emptied_slots_and_stars_from_copies() {
    let (s, mut t) = (set(), Tracker::default());
    observe(&mut t, &s, (2, 1), Some(10), &[Some(GAREN), Some(LUX), Some(VI), Some(GAREN), None]);
    observe(&mut t, &s, (2, 1), Some(9), &[None, Some(LUX), Some(VI), Some(GAREN), None]);
    assert_eq!(t.copies(GAREN), 1);
    observe(&mut t, &s, (2, 1), Some(8), &[None, Some(LUX), Some(VI), None, None]);
    assert_eq!(t.copies(GAREN), 2);
    // A reroll: every slot changes, gold drops by 2, nothing bought.
    observe(&mut t, &s, (2, 1), Some(6), &[Some(GAREN), Some(AHRI), Some(LUX), Some(VI), Some(LEONA)]);
    assert_eq!(t.units().len(), 1);
    observe(&mut t, &s, (2, 1), Some(5), &[None, Some(AHRI), Some(LUX), Some(VI), Some(LEONA)]);
    assert!(t.owned().contains(&OwnedUnit { api: GAREN.into(), copies: 3, star: 2 }));
    assert_eq!((star(2), star(3), star(8), star(9)), (1, 2, 2, 3));
}

#[test]
fn several_buys_and_xp_in_one_tick() {
    let (s, mut t) = (set(), Tracker::default());
    observe(&mut t, &s, (3, 2), Some(20), &[Some(VI), Some(AHRI), Some(LUX), None, None]);
    // Vi (2) + Ahri (3) + one XP buy (4) = 9.
    observe(&mut t, &s, (3, 2), Some(11), &[None, None, Some(LUX), None, None]);
    assert_eq!((t.copies(VI), t.copies(AHRI), t.copies(LUX)), (1, 1, 0));
}

#[test]
fn unreadable_fields_are_never_zero() {
    let (s, mut t) = (set(), Tracker::default());
    observe(&mut t, &s, (2, 2), Some(10), &[Some(GAREN), Some(VI), None, None, None]);
    // Gold unread while the slot empties: nothing decided yet, the baseline is kept.
    observe(&mut t, &s, (2, 2), None, &[None, Some(VI), None, None, None]);
    assert_eq!(t.copies(GAREN), 0);
    observe(&mut t, &s, (2, 2), Some(9), &[None, Some(VI), None, None, None]);
    assert_eq!(t.copies(GAREN), 1);
    // Shop unread (empty) while gold drops: no buys.
    t.observe(&TftSnapshot { shop: vec![], ..snap((2, 2), 7) }, &s);
    assert_eq!(t.units().len(), 1);
    // Stage unread: no reset, stage kept.
    t.observe(&TftSnapshot { stage: None, ..snap((2, 2), 9) }, &s);
    assert_eq!((t.stage(), t.copies(GAREN)), (Some((2, 2)), 1));
    // Trait panel unread (empty): the last correction stays.
    t.observe(&TftSnapshot { traits: traits(&[(DEFENDER, 3), (BRUISER, 1)]), ..snap((2, 2), 9) }, &s);
    t.observe(&snap((2, 2), 9), &s);
    assert_eq!(t.unknown(), &[(DEFENDER.to_string(), 2)]);
}

#[test]
fn a_misread_empty_slot_is_undone() {
    let (s, mut t) = (set(), Tracker::default());
    observe(&mut t, &s, (3, 1), Some(10), &[Some(RENGAR), None, None, None, None]);
    // An XP buy (4) while Rengar's slot (cost 4) flickers unread.
    observe(&mut t, &s, (3, 1), Some(6), &[None, None, None, None, None]);
    assert_eq!(t.copies(RENGAR), 1);
    observe(&mut t, &s, (3, 1), Some(6), &[Some(RENGAR), None, None, None, None]);
    assert_eq!(t.copies(RENGAR), 0);
}

#[test]
fn the_trait_panel_names_or_counts_untracked_units() {
    let s = set();
    let mut t = Tracker::default();
    own(&mut t, &s, (3, 1), &[GAREN]);
    // Garen explains one Defender and the Bruiser; the rest is exactly Rengar's traits.
    t.observe(&TftSnapshot { traits: traits(&[(DEFENDER, 2), (BRUISER, 1), (DUELIST, 1)]), ..snap((3, 1), 20) }, &s);
    let rengar = t.units().iter().find(|u| u.api == RENGAR).expect("named from the panel");
    assert!(rengar.seen && rengar.copies == 1);
    assert!(t.unknown().is_empty());

    // Lux and Morgana share their traits: not named, counted as unknown.
    let mut t = Tracker::default();
    t.observe(&TftSnapshot { traits: traits(&[(SORCERER, 1), (COVEN, 1)]), ..snap((2, 1), 4) }, &s);
    assert!(t.units().is_empty());
    let unknown: Vec<OwnedUnit> = t.owned().into_iter().filter(|u| u.star == 0).collect();
    assert_eq!(unknown.len(), 2);
    assert!(unknown.iter().any(|u| u.api == SORCERER && u.copies == 1));
}

#[test]
fn a_sell_is_gold_back_while_its_traits_leave_the_panel() {
    let (s, mut t) = (set(), Tracker::default());
    own(&mut t, &s, (2, 3), &[GAREN, VI]);
    t.observe(&TftSnapshot { traits: traits(&[(DEFENDER, 1), (BRUISER, 2), (DUELIST, 1)]), ..snap((2, 3), 20) }, &s);
    t.observe(&TftSnapshot { traits: traits(&[(BRUISER, 1), (DUELIST, 1)]), ..snap((2, 3), 21) }, &s);
    assert_eq!((t.copies(GAREN), t.copies(VI)), (0, 1));
}

#[test]
fn a_new_game_resets() {
    let (s, mut t) = (set(), Tracker::default());
    own(&mut t, &s, (3, 2), &[GAREN, VI]);
    t.observe(&snap((1, 1), 0), &s);
    assert!(t.owned().is_empty());
    assert_eq!(t.stage(), Some((1, 1)));
}

#[test]
fn scouted_boards_merge_and_decay() {
    let (s, mut t) = (set(), Tracker::default());
    let other = |stage, tr: &[(&str, u32)]| TftSnapshot { viewing_other: true, traits: traits(tr), ..snap(stage, 30) };
    t.observe(&other((3, 1), &[(DEFENDER, 4), (DUELIST, 2), (GREENFATHER, 1)]), &s);
    t.observe(&other((3, 1), &[(DEFENDER, 4), (DUELIST, 2)]), &s); // the same board, read again
    t.observe(&other((3, 3), &[(SORCERER, 4), (COVEN, 2)]), &s);
    assert_eq!(t.boards().len(), 2);
    // Scouting never touches my own units or panel.
    assert!(t.panel().is_empty() && t.units().is_empty());
    t.observe(&snap((3, 4), 30), &s);
    assert_eq!(t.boards().len(), 1);
    assert_eq!(t.boards()[0].traits[0].0, SORCERER);
    t.observe(&snap((3, 6), 30), &s);
    assert!(t.boards().is_empty());
}
