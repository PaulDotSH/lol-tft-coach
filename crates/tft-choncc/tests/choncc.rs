//! Choncc or Treat: the starting 4/5-cost unit, comps that use it, loot order, team size and
//! the Golden Orb.

#[path = "../../tft-coach/tests/common/mod.rs"]
mod common;

use common::*;
use tft_choncc::*;
use tft_coach::*;
use tft_model::*;

fn choncc(snap: &TftSnapshot, t: &Tracker, s: &TftSetData, m: &TftMeta) -> TftAdvice {
    advise(&CoachInput { snap, tracker: t, set: s, meta: m, hooks: &ChonccHooks, keep: None })
}

fn panel_at_start(tr: &[(&str, u32)]) -> Tracker {
    let mut t = Tracker::default();
    t.observe(&TftSnapshot { traits: traits(tr), ..snap((1, 1), 0) }, &set());
    t
}

#[test]
fn the_starter_shows_on_the_first_trait_panel() {
    let s = set();
    // Exactly Taric's traits: named by the tracker itself.
    assert_eq!(starter(&panel_at_start(&[(GREENFATHER, 1), (DEFENDER, 1)]), &s).as_deref(), Some(TARIC));
    // Lux and Morgana share traits; only Morgana costs 4+.
    assert_eq!(starter(&panel_at_start(&[(SORCERER, 1), (COVEN, 1)]), &s).as_deref(), Some(MORGANA));
    // Bought early counts; bought later doesn't.
    let mut t = Tracker::default();
    own(&mut t, &s, (2, 1), &[MORGANA]);
    assert_eq!(starter(&t, &s).as_deref(), Some(MORGANA));
    let mut t = Tracker::default();
    own(&mut t, &s, (3, 2), &[MORGANA]);
    assert_eq!(starter(&t, &s), None);
}

#[test]
fn comps_that_use_the_starter_come_first() {
    let (s, m) = (set(), meta());
    let t = panel_at_start(&[(GREENFATHER, 1), (DEFENDER, 1)]);
    let now = snap((1, 1), 0);
    let a = choncc(&now, &t, &s, &m);
    assert_eq!(a.mode, TftMode::Choncc);
    let c = a.choncc.as_ref().unwrap();
    assert_eq!(c.starter.as_deref(), Some(TARIC));
    let mut got = ids(&c.comps);
    got.sort();
    assert_eq!(got, [RENGAR_COMP, TARIC_COMP]);
    assert!([RENGAR_COMP, TARIC_COMP].contains(&a.comps[0].comp.as_str()));
    let input = CoachInput { snap: &now, tracker: &t, set: &s, meta: &m, hooks: &ChonccHooks, keep: None };
    assert_eq!((ChonccHooks.comp_bonus(&m.comps[0], &input), ChonccHooks.comp_bonus(&m.comps[2], &input)), (0.1, 0.0));
    // The standard coach has no Choncc part.
    assert!(advise_with(&now, &t, &s, &m, None).choncc.is_none());
}

#[test]
fn loot_goes_radiant_artifact_emblem_spatula_components() {
    let (s, m) = (set(), meta());
    let l = loot(&items(&[SWORD, DEF_EMBLEM, ARTIFACT, RABADON_RADIANT, SPATULA]), &s, Some(&m.comps[0]));
    assert_eq!(
        l,
        [
            "Loot orbs: radiant > artifact > emblem for Defender Rengar > components",
            "Radiant Rabadon's Deathcap (radiant): on Rengar",
            "Deathfire Grasp (artifact): on Rengar",
            "Defender Emblem: adds Defender for Defender Rengar",
            "Spatula: make a Defender emblem",
            "B.F. Sword: component",
        ]
    );
    assert_eq!(loot(&[], &s, None), ["Loot orbs: radiant > artifact > emblem > components"]);
}

#[test]
fn team_size_and_golden_orb_notes() {
    let (s, m) = (set(), meta());
    let mut t = panel_at_start(&[(GREENFATHER, 1), (DEFENDER, 1)]);
    let now = TftSnapshot { level: Some(8), team_size: Some((7, 9)), hp: Some(45), ..snap((4, 2), 20) };
    t.observe(&now, &s);
    let a = choncc(&now, &t, &s, &m);
    let notes = &a.choncc.as_ref().unwrap().notes;
    assert!(notes.contains(&"Field 9 units: 2 slots open".to_string()), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("Golden Orb")));
    assert!(keys(&a).contains(&"field-9"));
}
