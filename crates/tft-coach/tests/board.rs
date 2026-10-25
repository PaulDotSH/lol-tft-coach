//! My board and bench by HP bars: pieces named by purchases, moves, the trait panel and looks.
//! Reads are repeated (a read counts once the next shows the same places, as live).

mod common;
use common::*;
use tft_coach::board::Named;
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

fn bench(slot: u32, api: &str, star: u32) -> SeenUnit {
    SeenUnit { x: 485.0 + 163.5 * slot as f32, y: 910.0, star, bench: Some(slot), full: true, items: vec![], emb: look(api) }
}

fn hex(x: f32, y: f32, api: &str, star: u32) -> SeenUnit {
    SeenUnit { x, y, star, bench: None, full: true, items: vec![], emb: look(api) }
}

fn read(t: &mut Tracker, s: &TftSetData, snap: TftSnapshot) {
    t.observe(&snap, s);
    t.observe(&snap, s);
}

fn screen(gold: u32, slots: &[Option<&str>], units: Vec<SeenUnit>, tr: &[(&str, u32)]) -> TftSnapshot {
    TftSnapshot { shop: shop(slots), units, traits: traits(tr), ..snap((2, 1), gold) }
}

fn named(t: &Tracker) -> Vec<(Option<String>, Named, Option<u32>, u32)> {
    t.board().pieces().iter().map(|p| (p.api.clone(), p.named, p.bench, p.star)).collect()
}

fn owned(t: &Tracker, api: &str) -> Option<OwnedUnit> {
    t.owned().into_iter().find(|o| o.api == api)
}

/// Garen on the board from the start (seen, not bought), then Vi bought onto bench slot 0.
fn start(t: &mut Tracker, s: &TftSetData) {
    let empty = [None; 5];
    read(t, s, screen(10, &empty, vec![hex(1000.0, 600.0, GAREN, 1)], &[(DEFENDER, 1), (BRUISER, 1)]));
    read(t, s, screen(10, &[Some(VI), None, None, None, None], vec![hex(1000.0, 600.0, GAREN, 1)], &[(DEFENDER, 1), (BRUISER, 1)]));
    read(t, s, screen(8, &empty, vec![hex(1000.0, 600.0, GAREN, 1), bench(0, VI, 1)], &[(DEFENDER, 1), (BRUISER, 1)]));
}

#[test]
fn the_panel_names_a_lone_board_piece_and_a_purchase_names_the_bench() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    let n = named(&t);
    assert!(n.contains(&(Some(GAREN.into()), Named::Panel, None, 1)), "{n:?}");
    assert!(n.contains(&(Some(VI.into()), Named::Bought, Some(0), 1)), "{n:?}");
    let vi = owned(&t, VI).unwrap();
    assert_eq!((vi.copies, vi.bench, vi.board), (1, 1, 0));
}

#[test]
fn a_moved_piece_keeps_its_name_and_the_panel_confirms_it() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    // Vi goes from the bench to the board: Duelist joins the panel.
    let units = vec![hex(1000.0, 600.0, GAREN, 1), hex(800.0, 500.0, VI, 1)];
    read(&mut t, &s, screen(8, &[None; 5], units, &[(DEFENDER, 1), (BRUISER, 2), (DUELIST, 1)]));
    let n = named(&t);
    assert!(n.contains(&(Some(VI.into()), Named::Bought, None, 1)), "{n:?}");
    assert_eq!(owned(&t, VI).unwrap().board, 1);
    // Both names fit the panel: learnt.
    assert!(t.library().classify(&look(VI)).first().is_some_and(|c| c.0 == VI));
}

#[test]
fn a_unit_from_nowhere_is_named_by_what_it_adds_to_the_panel() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    // Ahri shows up on the board (carousel): Sorcerer and Duelist join the panel.
    let units = vec![hex(1000.0, 600.0, GAREN, 1), hex(1200.0, 500.0, AHRI, 1), bench(0, VI, 1)];
    read(&mut t, &s, screen(8, &[None; 5], units, &[(DEFENDER, 1), (BRUISER, 1), (SORCERER, 1), (DUELIST, 1)]));
    let n = named(&t);
    assert!(n.contains(&(Some(AHRI.into()), Named::Panel, None, 1)), "{n:?}");
}

#[test]
fn units_with_the_same_traits_stay_unsure() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    // Lux and Morgana have the same traits: the panel can't tell which joined.
    let units = vec![hex(1000.0, 600.0, GAREN, 1), hex(1200.0, 500.0, LUX, 1), bench(0, VI, 1)];
    read(&mut t, &s, screen(8, &[None; 5], units, &[(DEFENDER, 1), (BRUISER, 1), (SORCERER, 1), (COVEN, 1)]));
    let p = t.board().pieces().iter().find(|p| p.x == 1200.0).unwrap();
    assert!(!p.sure(), "{:?}", named(&t));
}

#[test]
fn a_sold_piece_stops_counting() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    for _ in 0..4 {
        read(&mut t, &s, screen(9, &[None; 5], vec![hex(1000.0, 600.0, GAREN, 1)], &[(DEFENDER, 1), (BRUISER, 1)]));
    }
    assert!(owned(&t, VI).is_none(), "{:?}", t.owned());
}

#[test]
fn a_third_copy_combines_into_two_stars() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    let g = |star| hex(1000.0, 600.0, GAREN, star);
    let panel = [(DEFENDER, 1), (BRUISER, 1)];
    read(&mut t, &s, screen(8, &[Some(GAREN), None, None, None, None], vec![g(1), bench(0, VI, 1)], &panel));
    read(&mut t, &s, screen(7, &[None; 5], vec![g(1), bench(0, VI, 1), bench(1, GAREN, 1)], &panel));
    read(&mut t, &s, screen(7, &[Some(GAREN), None, None, None, None], vec![g(1), bench(0, VI, 1), bench(1, GAREN, 1)], &panel));
    // The third copy: the bench copy goes, the board one turns ★2.
    read(&mut t, &s, screen(6, &[None; 5], vec![g(2), bench(0, VI, 1)], &panel));
    let garen = owned(&t, GAREN).unwrap();
    assert_eq!((garen.copies, garen.star), (3, 2), "{:?}", t.owned());
}

#[test]
fn scouting_and_combat_leave_my_pieces_alone() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    let before = named(&t);
    // Another player's board: their Rengar.
    read(&mut t, &s, TftSnapshot { viewing_other: true, ..screen(8, &[None; 5], vec![hex(900.0, 500.0, RENGAR, 2)], &[(DEFENDER, 1), (DUELIST, 1)]) });
    // Combat: enemy bars, my units mid-fight.
    read(&mut t, &s, TftSnapshot { enemies: 3, ..screen(8, &[None; 5], vec![hex(1300.0, 300.0, GAREN, 1)], &[(DEFENDER, 1), (BRUISER, 1)]) });
    assert_eq!(named(&t), before);
}

#[test]
fn a_moving_bar_is_not_read() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    // Vi dragged: its bar is somewhere new on each read, so no read counts.
    t.observe(&screen(8, &[None; 5], vec![hex(1000.0, 600.0, GAREN, 1), hex(700.0, 800.0, VI, 1)], &[(DEFENDER, 1), (BRUISER, 1)]), &s);
    t.observe(&screen(8, &[None; 5], vec![hex(1000.0, 600.0, GAREN, 1), hex(760.0, 740.0, VI, 1)], &[(DEFENDER, 1), (BRUISER, 1)]), &s);
    assert!(named(&t).contains(&(Some(VI.into()), Named::Bought, Some(0), 1)), "{:?}", named(&t));
}

#[test]
fn looks_learnt_in_one_game_name_units_in_the_next() {
    let s = set();
    let mut t = Tracker::default();
    start(&mut t, &s);
    read(&mut t, &s, screen(8, &[None; 5], vec![hex(1000.0, 600.0, GAREN, 1), hex(800.0, 500.0, VI, 1)], &[(DEFENDER, 1), (BRUISER, 2), (DUELIST, 1)]));
    let mut next = Tracker::default().with_library(t.library().clone());
    // A new game: Vi on the bench from the start, never bought.
    read(&mut next, &s, screen(10, &[None; 5], vec![hex(1000.0, 600.0, GAREN, 1), bench(3, VI, 1)], &[(DEFENDER, 1), (BRUISER, 1)]));
    assert!(named(&next).contains(&(Some(VI.into()), Named::Looks, Some(3), 1)), "{:?}", named(&next));
}

#[test]
fn a_free_slot_is_called_with_the_bench_unit_to_field() {
    let (s, m) = (set(), meta());
    let mut t = Tracker::default();
    start(&mut t, &s);
    let units = vec![hex(1000.0, 600.0, GAREN, 1), bench(0, VI, 1)];
    let now = |team| TftSnapshot { team_size: Some(team), units: units.clone(), traits: traits(&[(DEFENDER, 1), (BRUISER, 1)]), ..snap((2, 1), 8) };
    // One read could be a misread; the second calls it, keyed by the cap.
    assert!(!keys(&tick(&mut t, &s, &m, &now((1, 2)))).iter().any(|k| k.starts_with("free-slot")));
    let a = tick(&mut t, &s, &m, &now((1, 2)));
    let c = a.calls.iter().find(|c| c.key == "free-slot-2").expect("free slot call");
    assert_eq!(c.text, "Free slot on your board (1/2): put Vi on");
    assert_eq!(a.team_size, Some((1, 2)));
    // Filled: no call. In combat the counter hides: the last read stays shown, no call.
    assert!(!keys(&tick(&mut t, &s, &m, &now((2, 2)))).iter().any(|k| k.starts_with("free-slot")));
    let a = tick(&mut t, &s, &m, &TftSnapshot { team_size: None, ..now((1, 2)) });
    assert_eq!((a.team_size, a.calls.iter().any(|c| c.key.starts_with("free-slot"))), (Some((2, 2)), false));
}
