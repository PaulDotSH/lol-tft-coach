//! Macro coach: what separates my losses from my wins over recent games, per
//! mode. Facts with the numbers behind them, ranked by impact × confidence, so
//! small samples rank low or are not reported at all.

use model::*;
use std::collections::HashMap;

/// Fewest games in a mode before it gets any pattern.
const MIN_GAMES: usize = 8;
/// Fewest losses and wins on each side of a comparison.
const MIN_SIDE: usize = 4;
/// Games at which a comparison gets half its sample-size weight.
const HALF_N: f32 = 8.0;
/// Two-sample z (shares) or Welch t (means) a gap needs (one-sided 95 %); full confidence at FULL_Z.
const MIN_Z: f32 = 1.65;
const FULL_Z: f32 = 3.0;
/// Smallest loss-minus-win share gap worth reporting.
const MIN_SHARE_GAP: f32 = 0.25;
/// Relative gap of a per-minute stat: smallest reported, full impact.
const MIN_REL_GAP: f32 = 0.10;
const FULL_REL_GAP: f32 = 0.4;
const EARLY_S: u32 = 14 * 60;
const EARLY_DEATHS: usize = 2;
const BEHIND_GOLD: i32 = -500;
/// Champion results: fewest games, shrinkage prior (games at the rate of my
/// other champions), raw and shrunk gaps needed, gap for full impact.
const CHAMP_MIN: usize = 8;
const CHAMP_PRIOR: f32 = 5.0;
const CHAMP_RAW_GAP: f32 = 0.12;
const CHAMP_GAP: f32 = 0.07;
const CHAMP_FULL_GAP: f32 = 0.25;
/// Benchmarks: fewest games on the champion, shortfall vs the average.
const BENCH_MIN: usize = 3;
const BENCH_SHORT: f32 = 0.10;
pub const MAX_PATTERNS: usize = 5;

/// `bench`: lolalytics averages per (mode, champion) for the champions I play most.
pub fn report(games: &[PastGame], sd: &StaticData, bench: &HashMap<(Mode, ChampKey), Benchmarks>) -> MacroReport {
    let games: Vec<&PastGame> = games.iter().filter(|g| !g.remake).collect();
    let mut modes = Vec::new();
    let mut found: Vec<(&str, Pattern)> = Vec::new();
    for mode in [Mode::Rift, Mode::Aram, Mode::Mayhem] {
        let gs: Vec<&PastGame> = games.iter().copied().filter(|g| g.mode == mode).collect();
        if gs.is_empty() {
            continue;
        }
        modes.push(ModeRecord { mode, games: gs.len() as u32, wins: gs.iter().filter(|g| g.win).count() as u32 });
        if gs.len() >= MIN_GAMES {
            found.extend(mode_patterns(mode, &gs, sd, bench));
        }
    }
    // Strongest per (mode, family) only: early deaths and deaths/10 min tell one story.
    found.sort_by(|(_, a), (_, b)| b.score.total_cmp(&a.score).then(a.kind.cmp(&b.kind)).then(a.champ.cmp(&b.champ)));
    let mut seen = Vec::new();
    let patterns = found
        .into_iter()
        .filter(|(family, p)| {
            let k = (p.mode, *family);
            !seen.contains(&k) && {
                seen.push(k);
                true
            }
        })
        .map(|(_, p)| p)
        .take(MAX_PATTERNS)
        .collect();

    let mut champs: Vec<ChampRecord> = Vec::new();
    for g in &games {
        match champs.iter_mut().find(|c| c.mode == g.mode && c.champ == g.champ) {
            Some(c) => {
                c.games += 1;
                c.wins += g.win as u32;
            }
            None => champs.push(ChampRecord { mode: g.mode, champ: g.champ, games: 1, wins: g.win as u32 }),
        }
    }
    champs.sort_by(|a, b| b.games.cmp(&a.games).then(b.wins.cmp(&a.wins)).then(a.champ.cmp(&b.champ)));

    MacroReport {
        games: games.len() as u32,
        with_timeline: games.iter().filter(|g| g.death_times.is_some()).count() as u32,
        modes,
        champs,
        patterns,
    }
}

fn minutes(g: &PastGame) -> f32 {
    (g.duration_s as f32 / 60.0).max(1.0)
}

/// From the test statistic and the number of games behind it.
fn confidence(z: f32, n: usize) -> f32 {
    (z / FULL_Z).min(1.0) * n as f32 / (n as f32 + HALF_N)
}

fn pct(x: f32) -> String {
    format!("{:.0}%", x * 100.0)
}

fn pattern(mode: Mode, kind: &str, title: String, evidence: String, focus: String) -> Pattern {
    Pattern { mode, kind: kind.into(), champ: None, title, evidence, focus, bars: vec![], unit: String::new(), games: 0, score: 0.0 }
}

struct Split<'a> {
    losses: Vec<&'a PastGame>,
    wins: Vec<&'a PastGame>,
}

/// An event's share in losses (a of l) vs wins (b of w).
struct Share {
    a: usize,
    l: usize,
    b: usize,
    w: usize,
    score: f32,
}

impl Share {
    fn bars(&self) -> Vec<(String, f32)> {
        vec![("Losses".into(), 100.0 * self.a as f32 / self.l as f32), ("Wins".into(), 100.0 * self.b as f32 / self.w as f32)]
    }
}

/// Over games where `event` is known: reported when clearly more common in losses.
fn share(s: &Split, event: impl Fn(&PastGame) -> Option<bool>) -> Option<Share> {
    let count = |gs: &[&PastGame]| gs.iter().filter_map(|g| event(g)).fold((0, 0), |(h, n), e| (h + e as usize, n + 1));
    let ((a, l), (b, w)) = (count(&s.losses), count(&s.wins));
    if l < MIN_SIDE || w < MIN_SIDE {
        return None;
    }
    let gap = a as f32 / l as f32 - b as f32 / w as f32;
    let p = (a + b) as f32 / (l + w) as f32;
    let se = (p * (1.0 - p) * (1.0 / l as f32 + 1.0 / w as f32)).sqrt();
    if gap < MIN_SHARE_GAP || se <= 0.0 {
        return None;
    }
    let z = gap / se;
    (z >= MIN_Z).then(|| Share { a, l, b, w, score: gap * confidence(z, l + w) })
}

/// Mean of a stat in losses and wins.
struct Means {
    l: f32,
    w: f32,
    n: usize,
    score: f32,
}

fn mean_var(v: &[f32]) -> (f32, f32) {
    let n = v.len() as f32;
    let m = v.iter().sum::<f32>() / n;
    (m, v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / (n - 1.0).max(1.0))
}

/// Reported when the stat is clearly worse in losses (`higher_better` says which way is worse).
fn means(s: &Split, higher_better: bool, value: impl Fn(&PastGame) -> Option<f32>) -> Option<Means> {
    let vals = |gs: &[&PastGame]| gs.iter().filter_map(|g| value(g)).collect::<Vec<f32>>();
    let (vl, vw) = (vals(&s.losses), vals(&s.wins));
    if vl.len() < MIN_SIDE || vw.len() < MIN_SIDE {
        return None;
    }
    let ((ml, sl), (mw, sw)) = (mean_var(&vl), mean_var(&vw));
    let worse = if higher_better { mw - ml } else { ml - mw };
    let base = ml.abs().max(mw.abs());
    if base <= 0.0 || worse / base < MIN_REL_GAP {
        return None;
    }
    let se = (sl / vl.len() as f32 + sw / vw.len() as f32).sqrt().max(base * 0.01);
    let t = worse / se;
    let n = vl.len() + vw.len();
    let score = (worse / base / FULL_REL_GAP).min(1.0) * confidence(t, n);
    (t >= MIN_Z).then_some(Means { l: ml, w: mw, n, score })
}

fn mode_patterns(
    mode: Mode,
    gs: &[&PastGame],
    sd: &StaticData,
    bench: &HashMap<(Mode, ChampKey), Benchmarks>,
) -> Vec<(&'static str, Pattern)> {
    let (losses, wins) = gs.iter().partition(|g| !g.win);
    let s = Split { losses, wins };
    let rift = mode == Mode::Rift;
    let mut out = Vec::new();
    let mut push_share = |family, kind, title: &str, evidence: &dyn Fn(&Share) -> String, focus: &str, sh: Option<Share>| {
        if let Some(sh) = sh {
            let p = pattern(mode, kind, title.into(), evidence(&sh), focus.into());
            out.push((family, Pattern { bars: sh.bars(), unit: "%".into(), games: (sh.l + sh.w) as u32, score: sh.score, ..p }));
        }
    };

    if rift {
        push_share(
            "deaths",
            "early_deaths",
            "Early deaths",
            &|sh| format!("In {} of {} losses you died {EARLY_DEATHS}+ times before 14:00 ({} of {} wins)", sh.a, sh.l, sh.b, sh.w),
            "Your losses tend to start with early deaths; reaching 14:00 with 0–1 deaths is common in your wins.",
            share(&s, |g| Some(g.death_times.as_ref()?.iter().filter(|&&t| t < EARLY_S).count() >= EARLY_DEATHS)),
        );
        let behind = |g: &PastGame| g.gold_diff_15.map(|d| d <= BEHIND_GOLD);
        let deficits: Vec<i32> = s.losses.iter().filter_map(|g| g.gold_diff_15).filter(|&d| d <= BEHIND_GOLD).collect();
        let avg = deficits.iter().sum::<i32>() / (deficits.len() as i32).max(1);
        push_share(
            "lane",
            "gold_15",
            "Gold at 15",
            &|sh| format!("Behind your lane opponent by 500+ gold at 15:00 in {} of {} losses (avg −{} g), {} of {} wins", sh.a, sh.l, -avg, sh.b, sh.w),
            "Most of your losses are already behind in lane by 15:00.",
            share(&s, behind),
        );
        push_share(
            "objectives",
            "first_dragon",
            "First dragon",
            &|sh| format!("Enemy took first dragon in {} of {} losses ({} of {} wins)", sh.a, sh.l, sh.b, sh.w),
            "First dragon goes to the enemy in most losses; lane priority before it spawns decides who contests it.",
            share(&s, |g| g.first_dragon.map(|mine| !mine)),
        );
    }
    push_share(
        "objectives",
        "first_tower",
        "First tower",
        &|sh| format!("Your team lost the first tower in {} of {} losses ({} of {} wins)", sh.a, sh.l, sh.b, sh.w),
        "The first tower falls on your side in most losses, which opens the map for the enemy.",
        share(&s, |g| g.first_tower.map(|mine| !mine)),
    );

    let mut push_means = |family, kind, title: &str, unit: &str, fmt: &dyn Fn(&Means) -> String, focus: &str, m: Option<Means>| {
        if let Some(m) = m {
            let p = pattern(mode, kind, title.into(), fmt(&m), focus.into());
            let bars = vec![("Losses".into(), m.l), ("Wins".into(), m.w)];
            out.push((family, Pattern { bars, unit: unit.into(), games: m.n as u32, score: m.score, ..p }));
        }
    };
    push_means(
        "deaths",
        "deaths",
        "Deaths",
        "/10 min",
        &|m| format!("{:.1} deaths per 10 min in losses vs {:.1} in wins", m.l, m.w),
        "Your death rate climbs in losses; staying alive tracks winning for you.",
        means(&s, false, |g| Some(g.deaths as f32 / minutes(g) * 10.0)),
    );
    push_means(
        "damage",
        "damage",
        "Damage",
        "/min",
        &|m| format!("Damage/min {:.0} in losses vs {:.0} in wins", m.l, m.w),
        "You deal less damage in losses: fewer fights reached, or fights taken from behind.",
        means(&s, true, |g| Some(g.damage_to_champs as f32 / minutes(g))),
    );
    if rift {
        push_means(
            "farm",
            "cs",
            "Farm",
            "/min",
            &|m| format!("CS/min {:.1} in losses vs {:.1} in wins", m.l, m.w),
            "Farm drops in losses: partly from being behind, partly CS left on the map.",
            means(&s, true, |g| Some(g.cs as f32 / minutes(g))),
        );
        push_means(
            "vision",
            "vision",
            "Vision",
            "/min",
            &|m| format!("Vision/min {:.2} in losses vs {:.2} in wins", m.l, m.w),
            "Vision falls in losses; it is partly a symptom of being behind.",
            means(&s, true, |g| Some(g.vision as f32 / minutes(g))),
        );
    }

    out.extend(champion(mode, gs, sd));
    out.extend(benchmarks(mode, gs, sd, bench));
    out
}

/// Champions whose results trail my other champions in this mode.
fn champion(mode: Mode, gs: &[&PastGame], sd: &StaticData) -> Vec<(&'static str, Pattern)> {
    let total = gs.len();
    let total_wins = gs.iter().filter(|g| g.win).count();
    let mut counts: HashMap<ChampKey, (usize, usize)> = HashMap::new();
    for g in gs {
        let c = counts.entry(g.champ).or_default();
        c.0 += 1;
        c.1 += g.win as usize;
    }
    let mut out = Vec::new();
    for (champ, (n, k)) in counts {
        let (rn, rk) = (total - n, total_wins - k);
        if n < CHAMP_MIN || rn < MIN_SIDE {
            continue;
        }
        let rest = rk as f32 / rn as f32;
        let raw = k as f32 / n as f32;
        let shrunk = (k as f32 + CHAMP_PRIOR * rest) / (n as f32 + CHAMP_PRIOR);
        if rest - raw < CHAMP_RAW_GAP || rest - shrunk < CHAMP_GAP {
            continue;
        }
        let name = sd.champ_name(champ);
        let p = pattern(
            mode,
            "champion",
            format!("{name} results"),
            format!("{name}: {k} wins in {n} games ({}) vs {} on your other champions", pct(raw), pct(rest)),
            format!("{name} trails your other picks in this mode so far."),
        );
        let score = ((rest - shrunk) / CHAMP_FULL_GAP).min(1.0) * n as f32 / (n as f32 + HALF_N);
        let bars = vec![(name.to_string(), raw * 100.0), ("Others".into(), rest * 100.0)];
        out.push(("champion", Pattern { champ: Some(champ), bars, unit: "%".into(), games: n as u32, score, ..p }));
    }
    out
}

/// My per-minute numbers on a champion vs the lolalytics average for it.
fn benchmarks(
    mode: Mode,
    gs: &[&PastGame],
    sd: &StaticData,
    bench: &HashMap<(Mode, ChampKey), Benchmarks>,
) -> Vec<(&'static str, Pattern)> {
    let mut out = Vec::new();
    for (&(m, champ), b) in bench {
        let mine: Vec<&&PastGame> = gs.iter().filter(|g| g.champ == champ).collect();
        let n = mine.len();
        if m != mode || n < BENCH_MIN {
            continue;
        }
        let avg = |f: fn(&PastGame) -> u32| mine.iter().map(|g| f(g) as f32 / minutes(g)).sum::<f32>() / n as f32;
        let name = sd.champ_name(champ);
        let stats = [
            ("farm", "bench_cs", "farm", "CS/min", 1, b.cs_per_min.filter(|_| mode == Mode::Rift), avg(|g| g.cs)),
            ("damage", "bench_damage", "damage", "Damage/min", 0, b.damage_per_min, avg(|g| g.damage_to_champs)),
        ];
        for (family, kind, what, label, digits, avg_b, mine_v) in stats {
            let Some(avg_b) = avg_b.filter(|&x| x > 0.0) else { continue };
            let short = 1.0 - mine_v / avg_b;
            if short < BENCH_SHORT {
                continue;
            }
            let p = pattern(
                mode,
                kind,
                format!("{name} {what}"),
                format!("{name} {label} {mine_v:.digits$} over {n} games vs {avg_b:.digits$} average"),
                format!("Average {name} players reach {avg_b:.digits$}/min; your games sit below that."),
            );
            let score = (short / FULL_REL_GAP).min(1.0) * n as f32 / (n as f32 + HALF_N / 2.0);
            let bars = vec![("You".into(), mine_v), ("Average".into(), avg_b)];
            out.push((family, Pattern { champ: Some(champ), bars, unit: "/min".into(), games: n as u32, score, ..p }));
        }
    }
    out
}
