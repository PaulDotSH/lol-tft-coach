//! Other players' units: scouted boards named by looks and their trait panel, my combat
//! opponent's red bars by looks, kept per player; copies held and left in the pool.

mod common;
use common::*;
use tft_coach::*;
use tft_model::*;

const DIM: usize = 8;

/// A unit's looks: its own direction (same unit, same vector).
fn look(api: &str) -> Vec<f32> {
    let k = [GAREN, LUX, VI, AHRI, LEONA, RENGAR, MORGANA, TARIC].iter().position(|u| *u == api).unwrap();
    let mut v = [0.05; DIM];
    v[k] = 1.0;
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter().map(|x| x / n).collect()
}

/// Looks the library has never seen.
fn stranger() -> Vec<f32> {
    vec![1.0 / (DIM as f32).sqrt(); DIM]
}

fn library(units: &[&str]) -> Library {
    let mut l = Library::default();
    for u in units {
        l.learn(u, 1, &look(u));
    }
    l
}

fn hex(x: f32, y: f32, emb: Vec<f32>, star: u32) -> SeenUnit {
    SeenUnit { x, y, star, bench: None, full: true, emb }
}

fn bench(slot: u32, api: &str) -> SeenUnit {
    SeenUnit { x: 485.0 + 163.5 * slot as f32, y: 910.0, star: 1, bench: Some(slot), full: true, emb: look(api) }
}

fn scouting(stage: (u32, u32), who: Option<&str>, units: Vec<SeenUnit>, tr: &[(&str, u32)]) -> TftSnapshot {
    TftSnapshot { viewing_other: true, viewing: who.map(str::to_string), units, traits: traits(tr), ..snap(stage, 30) }
}

fn fight(stage: (u32, u32), who: &str, foes: Vec<SeenUnit>) -> TftSnapshot {
    TftSnapshot { viewing: Some(who.into()), enemies: foes.len() as u32, foes, ..snap(stage, 30) }
}

/// Twice: a read counts once the next shows the same places, as live.
fn read(t: &mut Tracker, s: &TftSetData, snap: &TftSnapshot) {
    t.observe(snap, s);
    t.observe(snap, s);
}

fn held(t: &Tracker, name: &str) -> Vec<(Api, u32)> {
    let mut u = t.scout().players().iter().find(|p| p.name.as_deref() == Some(name)).map(|p| p.units()).unwrap_or_default();
    u.sort();
    u
}

#[test]
fn a_scouted_board_is_named_by_looks_and_its_panel_without_teaching_the_library() {
    let s = set();
    let mut t = Tracker::default().with_library(library(&[AHRI, VI, LUX]));
    // Ahri ★2 and Vi by looks; the panel's Defender and the second Bruiser are a Garen nobody knows by looks.
    let units = vec![hex(1000.0, 500.0, look(AHRI), 2), hex(1200.0, 500.0, look(VI), 1), hex(900.0, 600.0, stranger(), 1), bench(2, LUX)];
    let panel = [(DUELIST, 2), (BRUISER, 2), (SORCERER, 1), (DEFENDER, 1)];
    read(&mut t, &s, &scouting((3, 2), Some("Bob"), units, &panel));
    let want: Vec<(Api, u32)> = vec![(AHRI.into(), 3), (GAREN.into(), 1), (LUX.into(), 1), (VI.into(), 1)];
    assert_eq!(held(&t, "Bob"), want);
    assert_eq!(t.scout().held(AHRI), 3);
    // Their pieces teach nothing, and my own board and panel stay as they were.
    assert_eq!(t.library().len(), 3);
    assert!(t.board().pieces().is_empty() && t.panel().is_empty() && t.units().is_empty());
}

#[test]
fn players_are_kept_by_name_then_by_their_panel_and_forgotten_unseen() {
    let s = set();
    let mut t = Tracker::default();
    let duel = [(DEFENDER, 4), (DUELIST, 2)];
    t.observe(&scouting((3, 1), Some("Bob"), vec![], &duel), &s);
    t.observe(&scouting((3, 1), Some("Ann"), vec![], &duel), &s); // the same traits, another player
    t.observe(&scouting((3, 2), Some("Bob"), vec![], &[(SORCERER, 4), (COVEN, 2)]), &s); // Bob pivoted
    t.observe(&scouting((3, 2), None, vec![], &[(DEFENDER, 4), (DUELIST, 2), (BRUISER, 1)]), &s); // no name: Ann by her panel
    let p = t.scout().players();
    assert_eq!(p.len(), 2, "{p:?}");
    assert_eq!(p[0].traits[0].0, SORCERER);
    assert_eq!(p[1].traits.len(), 3);
    t.observe(&snap((3, 5), 30), &s);
    assert!(t.scout().players().is_empty());
}

#[test]
fn my_opponents_units_come_from_still_combat_reads() {
    let s = set();
    let mut t = Tracker::default().with_library(library(&[RENGAR, TARIC, LUX]));
    let foes = |dx: f32| vec![hex(1000.0 + dx, 300.0, look(RENGAR), 2), hex(1200.0, 300.0, look(TARIC), 1), hex(1300.0, 320.0, stranger(), 1), SeenUnit { y: 120.0, bench: Some(0), ..bench(0, LUX) }];
    // One read alone, then a moved one: the fight may be on, nothing counts.
    t.observe(&fight((4, 1), "Bob", foes(0.0)), &s);
    t.observe(&fight((4, 1), "Bob", foes(40.0)), &s);
    assert_eq!(t.scout().held(RENGAR), 0);
    // Still: named by looks (the stranger stays unnamed), bench included.
    t.observe(&fight((4, 1), "Bob", foes(40.0)), &s);
    assert_eq!(held(&t, "Bob"), vec![(LUX.into(), 1), (RENGAR.into(), 3), (TARIC.into(), 1)]);
    // Later in the fight units die: the best read of the round stays.
    let fewer = foes(40.0).into_iter().take(1).collect::<Vec<_>>();
    read(&mut t, &s, &fight((4, 1), "Bob", fewer));
    assert_eq!(t.scout().held(TARIC), 1);
    assert!(t.board().pieces().is_empty());
}

#[test]
fn advice_says_copies_held_and_left() {
    let (s, m) = (set(), meta());
    let mut t = Tracker::default().with_library(library(&[RENGAR, TARIC]));
    own(&mut t, &s, (4, 1), &[RENGAR, TARIC, LEONA, GAREN]);
    read(&mut t, &s, &fight((4, 1), "Bob", vec![hex(1000.0, 300.0, look(RENGAR), 2)]));
    let units = vec![hex(1000.0, 500.0, look(RENGAR), 1), hex(1200.0, 500.0, look(TARIC), 1)];
    read(&mut t, &s, &scouting((4, 1), Some("Ann"), units, &[(DEFENDER, 2), (DUELIST, 1), (GREENFATHER, 1)]));
    let a = tick(&mut t, &s, &m, &TftSnapshot { shop: shop(&[Some(RENGAR), None, None, None, None]), ..snap((4, 1), 30) });
    assert_eq!(a.comps[0].comp, RENGAR_COMP);
    // Rengar costs 4: a pool of 10, less Bob's ★2, Ann's and mine.
    assert_eq!(a.contested[0], Contest { unit: RENGAR.into(), boards: 2, held: 4, left: Some(5) });
    assert_eq!((a.shop[0].contested, a.shop[0].held, a.shop[0].left), (2, 4, Some(5)));
    assert!(a.shop[0].reason.ends_with("4 held by others, 5 left"), "{}", a.shop[0].reason);
    let call = a.calls.iter().find(|c| c.key == "contested-DA_18_Rengar").expect("called");
    assert!(call.text.starts_with("Rengar: 4 held by others, 5 left"), "{}", call.text);
}

#[test]
fn my_own_panel_and_the_carousel_are_not_scouting() {
    let s = set();
    let mut t = Tracker::default().with_library(library(&[AHRI]));
    let mine = [(SORCERER, 1), (DUELIST, 1)];
    t.observe(&TftSnapshot { traits: traits(&mine), ..snap((3, 2), 30) }, &s);
    // My plate unread over my own board: the panel gives it away.
    read(&mut t, &s, &scouting((3, 2), Some("Bob"), vec![hex(1000.0, 500.0, look(AHRI), 1)], &mine));
    // The carousel shows other plates over my panel too.
    t.observe(&scouting((3, 4), Some("Ann"), vec![], &[(DEFENDER, 2)]), &s);
    assert!(t.scout().players().is_empty(), "{:?}", t.scout().players());
}

#[test]
fn fights_without_a_plate_go_to_whoever_holds_the_same_units_and_pve_is_skipped() {
    let s = set();
    let mut t = Tracker::default().with_library(library(&[RENGAR, TARIC, LUX, VI]));
    let foes = |units: &[(&str, f32)]| units.iter().map(|&(u, x)| hex(x, 300.0, look(u), 1)).collect::<Vec<_>>();
    read(&mut t, &s, &fight((4, 5), "Bob", foes(&[(RENGAR, 1000.0), (TARIC, 1200.0)])));
    // Next round, Bob's plate unread: his Rengar and Taric again (and a new Lux).
    let unnamed = |stage, units| TftSnapshot { viewing: None, ..fight(stage, "", units) };
    read(&mut t, &s, &unnamed((4, 6), foes(&[(RENGAR, 1000.0), (TARIC, 1200.0), (LUX, 900.0)])));
    assert_eq!(t.scout().players().len(), 1);
    assert_eq!(held(&t, "Bob"), vec![(LUX.into(), 1), (RENGAR.into(), 1), (TARIC.into(), 1)]);
    // Someone else's units: another player, nameless.
    read(&mut t, &s, &unnamed((4, 6), foes(&[(VI, 1000.0)])));
    assert_eq!(t.scout().players().len(), 2);
    // Monsters on a PvE round, and units whose bars the fight already touched (no looks): nothing.
    read(&mut t, &s, &unnamed((4, 7), foes(&[(GAREN, 1000.0)])));
    let hit = vec![SeenUnit { full: false, emb: vec![], ..hex(1000.0, 300.0, vec![], 1) }];
    read(&mut t, &s, &fight((4, 7), "Cid", hit));
    assert_eq!(t.scout().players().len(), 2);
    assert_eq!(t.scout().held(GAREN), 0);
}

/// A unit's looks over a floor: its own axis plus a strong background axis (8: my board, 9: an away arena).
fn on_floor(api: Option<&str>, floor: usize) -> Vec<f32> {
    let k = api.map_or(10, |a| [GAREN, LUX, VI, AHRI, LEONA, RENGAR, MORGANA, TARIC].iter().position(|u| *u == a).unwrap());
    let mut v = [0.05; 11];
    (v[k], v[floor]) = (1.0, 2.0);
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter().map(|x| x / n).collect()
}

#[test]
fn away_from_my_board_looks_are_compared_without_the_floor() {
    let s = set();
    let mut l = Library::default();
    for u in [GAREN, LUX, VI, AHRI, LEONA, RENGAR, MORGANA, TARIC] {
        l.learn(u, 1, &on_floor(Some(u), 8));
    }
    let mut t = Tracker::default().with_library(l);
    // On another arena the floor dominates: as is, nothing is alike enough; centred, Rengar and
    // Taric stand out and the stranger (a unit I never had) stays unnamed.
    let foes = vec![hex(1000.0, 300.0, on_floor(Some(RENGAR), 9), 2), hex(1200.0, 300.0, on_floor(Some(TARIC), 9), 1), hex(1400.0, 300.0, on_floor(None, 9), 1)];
    read(&mut t, &s, &fight((4, 1), "Bob", foes));
    assert_eq!(held(&t, "Bob"), vec![(RENGAR.into(), 3), (TARIC.into(), 1)]);
    assert_eq!(t.scout().players()[0].held().iter().filter(|h| h.api.is_none()).count(), 1);
}
