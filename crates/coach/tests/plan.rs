//! Game-plan card: team edge by game length, scalers and the card's lines.

mod common;

use coach::{Curve, GamePlanInput, game_plan, roster_draft};
use common::*;
use model::*;
use std::collections::HashMap;

const ALLIES: [ChampKey; 5] = [AHRI, ZED, JINX, LEONA, GAREN];
const ENEMIES: [ChampKey; 5] = [KAYLE, MALPHITE, LUX, SYNDRA, LEE_SIN];
const FROM: [u32; 7] = [0, 15, 20, 25, 30, 35, 40];

/// 1,000 games in each bucket at these win rates.
fn curve(wr: [f32; 7]) -> Curve {
    (None, FROM.iter().zip(wr).map(|(&from_min, w)| LengthWin { from_min, games: 1000, wins: (w * 10.0) as u32 }).collect())
}

fn flat() -> Curve {
    curve([50.0; 7])
}

fn lengths(f: impl Fn(ChampKey) -> Curve) -> HashMap<ChampKey, Curve> {
    ALLIES.iter().chain(&ENEMIES).map(|&c| (c, f(c))).collect()
}

fn roster(c: &[ChampKey]) -> Vec<(ChampKey, Option<Lane>)> {
    c.iter().map(|&c| (c, None)).collect()
}

fn note(level6: bool, wins: &str) -> ChampNote {
    ChampNote {
        level2: false,
        level6,
        engage: false,
        poke: false,
        sustain: false,
        wins: wins.into(),
        watch: String::new(),
        spike: String::new(),
        trade: String::new(),
    }
}

struct Case {
    allies: Vec<(ChampKey, Option<Lane>)>,
    enemies: Vec<(ChampKey, Option<Lane>)>,
    length: HashMap<ChampKey, Curve>,
    draft: Option<DraftReport>,
    matchup: Option<MatchupPlan>,
    memory: Vec<MatchupMemory>,
    notes: LaneNotes,
    sd: StaticData,
    mode: Mode,
}

impl Case {
    fn new(length: HashMap<ChampKey, Curve>) -> Case {
        Case {
            allies: roster(&ALLIES),
            enemies: roster(&ENEMIES),
            length,
            draft: None,
            matchup: None,
            memory: vec![],
            notes: LaneNotes::default(),
            sd: sd(),
            mode: Mode::Rift,
        }
    }

    fn plan(&self) -> Option<GamePlan> {
        game_plan(&GamePlanInput {
            allies: &self.allies,
            enemies: &self.enemies,
            length: &self.length,
            draft: self.draft.as_ref(),
            matchup: self.matchup.as_ref(),
            memory: &self.memory,
            notes: &self.notes,
            sd: &self.sd,
            mode: self.mode,
        })
    }
}

fn line<'a>(p: &'a GamePlan, kind: &str) -> Option<&'a str> {
    p.lines.iter().find(|l| l.kind == kind).map(|l| l.text.as_str())
}

fn kayle_late() -> HashMap<ChampKey, Curve> {
    lengths(|c| if c == KAYLE { curve([40.0, 40.0, 40.0, 50.0, 55.0, 60.0, 60.0]) } else { flat() })
}

#[test]
fn late_scaling_enemy_team() {
    let p = Case::new(kayle_late()).plan().unwrap();
    let late = &p.phases[2];
    assert_eq!((late.name.as_str(), late.from_min, late.to_min), ("Late", 35, None));
    assert!(late.edge < -0.7 && !late.even, "{late:?}");
    assert!(p.phases[0].edge > 0.7, "Kayle is weak early: {:?}", p.phases[0]);
    let kayle = p.scalers.iter().find(|s| s.champ == KAYLE).unwrap();
    assert!(!kayle.ally && kayle.late > 1.5 && kayle.early < 0.0, "{kayle:?}");
    let s = line(&p, "scaling").unwrap();
    assert!(s.starts_with("They are stronger late (+"), "{s}");
    assert!(s.contains("after 35 min: Kayle)") && s.contains("You are stronger early"), "{s}");
    assert_eq!(p.fold, "Stronger early · they scale: Kayle");
    assert_eq!((p.partial, p.curves, p.champs, p.source.as_str()), (false, 10, 10, "lolalytics"));
    assert_eq!(p.show_s, 120.0);
}

#[test]
fn all_even_has_no_clear_edge() {
    let p = Case::new(lengths(|_| flat())).plan().unwrap();
    assert!(p.phases.iter().all(|ph| ph.even && ph.edge.abs() < 1e-4));
    assert!(p.scalers.is_empty());
    assert_eq!(line(&p, "scaling"), Some("No clear edge by game length."));
    assert_eq!(p.fold, "No clear edge by game length");
}

#[test]
fn missing_curve_is_partial_and_none_is_no_card() {
    let mut l = kayle_late();
    l.remove(&LUX);
    l.remove(&AHRI);
    let p = Case::new(l).plan().unwrap();
    assert_eq!((p.partial, p.curves, p.champs), (true, 8, 10));
    assert!(Case::new(HashMap::new()).plan().is_none(), "lolalytics down: no card");
    let only_mine = ALLIES.iter().map(|&c| (c, flat())).collect();
    assert!(Case::new(only_mine).plan().is_none(), "no curve for a whole side");
}

#[test]
fn small_buckets_merge_into_a_neighbour() {
    // 60 games won 55 times under 15 min: merged into 15–20, so no wild early swing.
    let mut short = flat();
    short.1[0] = LengthWin { from_min: 0, games: 60, wins: 55 };
    let p = Case::new(lengths(|c| if c == KAYLE { short.clone() } else { flat() })).plan().unwrap();
    assert!(p.scalers.iter().all(|s| s.champ != KAYLE), "{:?}", p.scalers);
    let mut unmerged = flat();
    unmerged.1[0] = LengthWin { from_min: 0, games: 600, wins: 550 };
    let q = Case::new(lengths(|c| if c == KAYLE { unmerged.clone() } else { flat() })).plan().unwrap();
    assert!(q.scalers.iter().any(|s| s.champ == KAYLE && s.early > 1.5), "{:?}", q.scalers);
}

#[test]
fn aram_boundaries() {
    let mut c = Case::new(lengths(|c| if c == KAYLE { curve([40.0, 45.0, 60.0, 60.0, 60.0, 60.0, 60.0]) } else { flat() }));
    c.mode = Mode::Aram;
    let p = c.plan().unwrap();
    let b: Vec<(u32, Option<u32>)> = p.phases.iter().map(|ph| (ph.from_min, ph.to_min)).collect();
    assert_eq!(b, [(0, Some(15)), (15, Some(20)), (20, None)]);
    assert!(line(&p, "scaling").unwrap().contains("after 20 min: Kayle"), "{:?}", p.lines);
    c.mode = Mode::Mayhem;
    let m = c.plan().unwrap();
    assert!(line(&m, "scaling").unwrap().starts_with("ARAM data"));
    assert_eq!(m.source, "lolalytics ARAM data");
}

#[test]
fn off_lane_data_is_marked() {
    let mut c = Case::new(kayle_late());
    c.enemies[0].1 = Some(Lane::Support);
    let p = c.plan().unwrap();
    assert!(p.scalers.iter().find(|s| s.champ == KAYLE).unwrap().off_lane);
    c.length.get_mut(&KAYLE).unwrap().0 = Some(Lane::Support);
    assert!(!c.plan().unwrap().scalers.iter().find(|s| s.champ == KAYLE).unwrap().off_lane);
}

#[test]
fn comp_spikes_lane_and_record_lines() {
    let mut c = Case::new(kayle_late());
    let sd = sd();
    c.draft = Some(roster_draft(&sd, &ALLIES, &ENEMIES));
    let comp = line(&c.plan().unwrap(), "comp").unwrap().to_string();
    let d = c.draft.as_ref().unwrap();
    assert!(comp.starts_with(&format!("Engage: you {}, them {}. Frontline: you {}, them {}", d.allies.engage, d.enemies.engage, d.allies.frontline, d.enemies.frontline)), "{comp}");

    let mut report = d.clone();
    report.enemies.physical = 0.85;
    report.allies.gaps = vec!["Little CC".into(), "No frontline".into()];
    report.enemies.gaps = vec!["Mostly physical (85%)".into()];
    c.draft = Some(report);
    let comp = line(&c.plan().unwrap(), "comp").unwrap().to_string();
    assert!(comp.contains("Their damage is 85% physical") && comp.ends_with("Their damage is 85% physical. You: little CC."), "{comp}");

    c.notes.champs.insert("Malphite".into(), note(true, "Poke with Q"));
    c.notes.champs.insert("Zed".into(), note(true, "All-in"));
    assert_eq!(line(&c.plan().unwrap(), "spikes"), Some("Level 6 spikes on their side: Malphite."));

    let card = MatchupCard {
        champ: SYNDRA,
        vs_wr: Some(48.2),
        games: 2140,
        tags: vec![],
        note: Some(note(false, "Q poke and E stun")),
        pair: None,
        spells: vec![],
        record: None,
    };
    c.matchup = Some(MatchupPlan { lane: Lane::Middle, allies: vec![AHRI], partner: None, cards: vec![card], build: None, runes: vec![] });
    let p = c.plan().unwrap();
    assert_eq!(line(&p, "lane"), Some("Lane vs Syndra: 48.2% for you (2.1k games). Syndra: Q poke and E stun."));
    assert!(line(&p, "record").is_none(), "no record without memory");
    c.mode = Mode::Aram;
    assert!(line(&c.plan().unwrap(), "lane").is_none(), "no lane line outside the Rift");
    c.mode = Mode::Rift;

    let mem = |champ, games, wins| MatchupMemory {
        champ,
        games,
        wins,
        lane_games: 0,
        gold_15: None,
        killed_me: 0,
        early_kills: 0,
        same_games: 0,
        same_wins: 0,
        last: vec![],
    };
    c.memory = vec![mem(SYNDRA, 3, 1), mem(KAYLE, 7, 2), mem(LUX, 0, 0)];
    let p = c.plan().unwrap();
    assert_eq!(line(&p, "record"), Some("Your record: 2–5 vs Kayle, 1–2 vs Syndra."));
    let kinds: Vec<&str> = p.lines.iter().map(|l| l.kind.as_str()).collect();
    assert_eq!(kinds, ["scaling", "comp", "spikes", "lane", "record"]);
}
