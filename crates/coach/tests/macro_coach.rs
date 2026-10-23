//! Macro coach behaviour over synthetic match histories.

mod common;

use common::*;
use model::*;
use std::collections::HashMap;

/// A 30-minute game where wins and losses look alike, with a little jitter so means have variance.
fn game(i: usize, mode: Mode, champ: ChampKey, win: bool) -> PastGame {
    let j = (i % 3) as u32;
    PastGame {
        game_id: 1000 + i as u64,
        created: 1_792_000_000_000 - i as u64 * 3_600_000,
        queue_id: match mode {
            Mode::Rift => 420,
            Mode::Aram => 450,
            Mode::Mayhem => 2400,
        },
        mode,
        champ,
        lane: (mode == Mode::Rift).then_some(Lane::Middle),
        win,
        remake: false,
        duration_s: 1800,
        kills: 5,
        deaths: 5 + j,
        assists: 6,
        cs: 200 + 5 * j,
        gold: 11000,
        damage_to_champs: 24000 + 500 * j,
        vision: 24 + j,
        death_times: Some(vec![600, 1000 + 100 * j, 1500]),
        gold_diff_10: Some(0),
        gold_diff_15: Some(100 * j as i32 - 100),
        xp_diff_15: Some(0),
        ours: None,
        theirs: None,
        first_dragon: Some(i.is_multiple_of(2)),
        first_tower: Some(!i.is_multiple_of(2)),
        first_blood: None,
        cs_10: None,
        augments: vec![],
        death_spots: vec![],
    }
}

/// `losses` then `wins` games of `champ`.
fn history(mode: Mode, champ: ChampKey, losses: usize, wins: usize) -> Vec<PastGame> {
    (0..losses + wins).map(|i| game(i, mode, champ, i >= losses)).collect()
}

fn report(games: &[PastGame]) -> MacroReport {
    coach::macro_report(games, &sd(), &HashMap::new())
}

fn kinds(r: &MacroReport) -> Vec<&str> {
    r.patterns.iter().map(|p| p.kind.as_str()).collect()
}

fn find<'a>(r: &'a MacroReport, kind: &str) -> &'a Pattern {
    r.patterns.iter().find(|p| p.kind == kind).unwrap_or_else(|| panic!("no {kind} in {:?}", kinds(r)))
}

#[test]
fn no_patterns_when_wins_and_losses_look_alike() {
    let r = report(&history(Mode::Rift, AHRI, 10, 10));
    assert_eq!(r.patterns, vec![]);
    assert_eq!((r.games, r.with_timeline), (20, 20));
    assert_eq!(r.modes, vec![ModeRecord { mode: Mode::Rift, games: 20, wins: 10 }]);
    assert_eq!(r.champs, vec![ChampRecord { mode: Mode::Rift, champ: AHRI, games: 20, wins: 10 }]);
}

#[test]
fn early_deaths_in_losses() {
    let mut h = history(Mode::Rift, AHRI, 9, 8);
    // 7 of 9 losses and 2 of 8 wins with two deaths before 14:00.
    for (i, g) in h.iter_mut().enumerate() {
        if i < 7 || (9..11).contains(&i) {
            g.death_times = Some(vec![300, 700, 1500]);
        }
    }
    let r = report(&h);
    let p = find(&r, "early_deaths");
    assert_eq!(p.evidence, "In 7 of 9 losses you died 2+ times before 14:00 (2 of 8 wins)");
    assert_eq!((p.mode, p.games, p.unit.as_str()), (Mode::Rift, 17, "%"));
    assert_eq!(p.bars, vec![("Losses".to_string(), 700.0 / 9.0), ("Wins".to_string(), 25.0)]);
    assert!(p.score > 0.15 && p.score <= 1.0, "{}", p.score);
}

#[test]
fn one_pattern_per_family() {
    let mut h = history(Mode::Rift, AHRI, 9, 9);
    for g in h.iter_mut().filter(|g| !g.win) {
        g.deaths += 4;
        g.death_times = Some(vec![200, 400, 700, 1500]);
    }
    let r = report(&h);
    let deaths = kinds(&r).into_iter().filter(|k| ["early_deaths", "deaths"].contains(k)).count();
    assert_eq!(deaths, 1, "{:?}", kinds(&r));
}

#[test]
fn farm_and_first_dragon() {
    let mut h = history(Mode::Rift, AHRI, 10, 10);
    for (i, g) in h.iter_mut().enumerate() {
        if !g.win {
            g.cs = 172 + 2 * (i % 3) as u32; // ~5.8/min
        } else {
            g.cs = 211 + 2 * (i % 3) as u32; // ~7.1/min
        }
        // Enemy first dragon in 8 of 10 losses, 3 of 10 wins.
        g.first_dragon = Some(if g.win { i >= 13 } else { i >= 8 });
    }
    let r = report(&h);
    assert_eq!(find(&r, "cs").evidence, "CS/min 5.8 in losses vs 7.1 in wins");
    assert_eq!(find(&r, "first_dragon").evidence, "Enemy took first dragon in 8 of 10 losses (3 of 10 wins)");
    assert!(r.patterns.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn gold_behind_at_15() {
    let mut h = history(Mode::Rift, AHRI, 8, 7);
    for (i, g) in h.iter_mut().enumerate() {
        g.gold_diff_15 = Some(match (g.win, i % 8) {
            (false, 0..=5) => -700 - 100 * (i as i32 % 3),
            (true, 0..=1) => -600,
            _ => 300,
        });
    }
    let p = report(&h).patterns.into_iter().find(|p| p.kind == "gold_15").expect("gold_15");
    assert_eq!(p.evidence, "Behind your lane opponent by 500+ gold at 15:00 in 6 of 8 losses (avg −800 g), 2 of 7 wins");
}

#[test]
fn small_samples_report_nothing() {
    // Huge gaps, but only 4 games.
    let mut h = history(Mode::Rift, AHRI, 2, 2);
    for g in h.iter_mut().filter(|g| !g.win) {
        g.deaths = 15;
        g.cs = 60;
        g.first_dragon = Some(false);
    }
    assert_eq!(report(&h).patterns, vec![]);

    // Enough games, but a single win: no loss-vs-win comparisons.
    let mut h = history(Mode::Rift, AHRI, 7, 1);
    for g in h.iter_mut().filter(|g| !g.win) {
        g.deaths = 15;
        g.first_dragon = Some(false);
    }
    assert_eq!(report(&h).patterns, vec![]);

    // 2 of 4 losses vs 0 of 4 wins is too weak a signal; 4 of 4 is not.
    let mut h = history(Mode::Rift, AHRI, 4, 4);
    for g in h.iter_mut().take(2) {
        g.death_times = Some(vec![100, 200]);
    }
    assert!(!kinds(&report(&h)).contains(&"early_deaths"));
    for g in h.iter_mut().take(4) {
        g.death_times = Some(vec![100, 200]);
    }
    assert!(kinds(&report(&h)).contains(&"early_deaths"));
}

#[test]
fn champion_trailing_other_picks() {
    // Ahri 5/13, others 9/17.
    let mut h: Vec<PastGame> = (0..13).map(|i| game(i, Mode::Rift, AHRI, i < 5)).collect();
    h.extend((13..30).map(|i| game(i, Mode::Rift, if i.is_multiple_of(2) { SYNDRA } else { LUX }, i < 22)));
    let r = report(&h);
    let p = find(&r, "champion");
    assert_eq!(p.champ, Some(AHRI));
    assert_eq!(p.evidence, "Ahri: 5 wins in 13 games (38%) vs 53% on your other champions");
    assert_eq!(r.champs[0], ChampRecord { mode: Mode::Rift, champ: AHRI, games: 13, wins: 5 });

    // 1 win in 6 is not enough games to call.
    let mut h: Vec<PastGame> = (0..6).map(|i| game(i, Mode::Rift, AHRI, i < 1)).collect();
    h.extend((6..20).map(|i| game(i, Mode::Rift, SYNDRA, i < 14)));
    assert!(!kinds(&report(&h)).contains(&"champion"));
}

#[test]
fn modes_are_separate() {
    // ARAM losses: many more deaths and less damage; Rift is even.
    let mut h = history(Mode::Aram, LUX, 6, 6);
    for g in h.iter_mut().filter(|g| !g.win) {
        g.deaths += 6;
        g.damage_to_champs -= 9000;
        g.cs = 20; // farm is not compared in ARAM
        g.death_times = Some(vec![60, 120, 180]);
    }
    h.extend(history(Mode::Rift, AHRI, 5, 5).into_iter().map(|g| PastGame { game_id: g.game_id + 100, ..g }));
    let mut remake = game(99, Mode::Mayhem, AHRI, false);
    remake.remake = true;
    h.push(remake);

    let r = report(&h);
    assert_eq!(r.games, 22, "remake excluded");
    assert_eq!(r.modes.iter().map(|m| m.mode).collect::<Vec<_>>(), [Mode::Rift, Mode::Aram]);
    assert!(r.patterns.iter().all(|p| p.mode == Mode::Aram), "{:?}", kinds(&r));
    let mut k = kinds(&r);
    k.sort();
    assert_eq!(k, ["damage", "deaths"], "no early deaths or farm outside the Rift");
}

#[test]
fn below_benchmark() {
    let mut h = history(Mode::Rift, AHRI, 4, 4);
    for g in &mut h {
        g.cs = 174; // 5.8/min
    }
    let bench = |cs| HashMap::from([((Mode::Rift, AHRI), Benchmarks { cs_per_min: Some(cs), damage_per_min: Some(700.0), ..Default::default() })]);
    let r = coach::macro_report(&h, &sd(), &bench(7.5));
    let p = find(&r, "bench_cs");
    assert_eq!(p.evidence, "Ahri CS/min 5.8 over 8 games vs 7.5 average");
    assert_eq!(p.champ, Some(AHRI));
    assert!(!kinds(&r).contains(&"bench_damage"), "damage is above average");

    // Close to the average: nothing. Benchmarks for another mode don't apply.
    assert_eq!(coach::macro_report(&h, &sd(), &bench(6.1)).patterns, vec![]);
    let aram = HashMap::from([((Mode::Aram, AHRI), Benchmarks { cs_per_min: Some(9.0), damage_per_min: Some(2000.0), ..Default::default() })]);
    assert_eq!(coach::macro_report(&h, &sd(), &aram).patterns, vec![]);
}

#[test]
fn at_most_five_patterns() {
    let mut h = history(Mode::Rift, AHRI, 12, 12);
    for (i, g) in h.iter_mut().enumerate().filter(|(_, g)| !g.win) {
        g.deaths += 5;
        g.cs -= 50;
        g.vision -= 12;
        g.damage_to_champs -= 9000;
        g.gold_diff_15 = Some(-900);
        g.first_dragon = Some(false);
        g.first_tower = Some(false);
        g.death_times = Some(vec![100, 200 + i as u32]);
    }
    let r = report(&h);
    assert_eq!(r.patterns.len(), coach::MAX_PATTERNS);
    assert!(r.patterns.windows(2).all(|w| w[0].score >= w[1].score));
}
