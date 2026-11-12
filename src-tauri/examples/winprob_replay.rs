//! Win probability over the recorded live snapshots (`%LOCALAPPDATA%\lol-coach\fixtures`): one
//! line per game and how often the predicted side won.
//! `cargo run -p lol-coach --example winprob_replay -- [table.toml] [recordings dir]`
//! Without a table it reads the built-in `winprob.toml`, and while that has no fit the hand-set
//! test table. The champions' curves are not fetched: the outlook counts 0 and every line is rough.

use coach::winprob::{Input, Model, win_prob};
use model::*;
use std::collections::HashMap;
use std::path::PathBuf;

const HANDSET: &str = include_str!("../../fixtures/coach/winprob_handset.toml");
/// A gap this long between two recordings (s) starts a new game.
const GAP: u64 = 600;
const AT_MIN: [f32; 3] = [10.0, 15.0, 20.0];
const BINS: usize = 10;

struct Game {
    started: u64,
    mode: Mode,
    swiftplay: bool,
    champ: ChampKey,
    roster: Vec<String>,
    last: f32,
    /// (game time, p) per recording.
    line: Vec<(f32, f32)>,
    /// From `GameEnd`, else from a side losing both nexus turrets (`sure` false).
    win: Option<bool>,
    sure: bool,
    history: coach::GoldHistory,
}

fn stamp(p: &std::path::Path) -> Option<u64> {
    p.file_name()?.to_str()?.strip_prefix("allgamedata-")?.strip_suffix(".json")?.parse().ok()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (model, label) = match args.first() {
        Some(path) => (Model::parse(&std::fs::read_to_string(path)?)?, path.clone()),
        None if Model::shipped().games > 0 => (Model::shipped().clone(), "winprob.toml".to_string()),
        None => (Model::parse(HANDSET)?, "HAND-SET test table (winprob.toml has no fit)".to_string()),
    };
    let dir = args.get(1).map_or_else(|| dirs::data_local_dir().unwrap_or_default().join("lol-coach").join("fixtures"), PathBuf::from);
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter_map(|p| Some((stamp(&p)?, p))).collect();
    files.sort();
    let http = data::Http::new(std::env::temp_dir().join("lol-coach-smoke-cache"))?;
    let sd = data::ddragon::load(&http).await?;
    println!("table: {label}, patch {}, {} games; {} recordings in {}", model.patch, model.games, files.len(), dir.display());

    let curves = HashMap::new();
    let (mut games, mut prev, mut unparsed, mut no_table): (Vec<Game>, u64, usize, usize) = (Vec::new(), 0, 0, 0);
    for (at, path) in &files {
        let Some(snap) = serde_json::from_str(&std::fs::read_to_string(path)?).ok().and_then(|j| clients::live::parse(&j, &sd)) else {
            unparsed += 1;
            continue;
        };
        // Names only: a Viego's champion changes while he holds a body.
        let mut roster: Vec<String> = snap.players.iter().map(|p| p.riot_id.clone()).collect();
        roster.sort();
        if games.last().is_none_or(|g| g.roster != roster || snap.time < g.last || at - prev > GAP) {
            let champ = snap.players[snap.me].champ;
            games.push(Game { started: *at, mode: snap.mode, swiftplay: snap.swiftplay, champ, roster, last: 0.0, line: vec![], win: None, sure: false, history: Default::default() });
        }
        prev = *at;
        let g = games.last_mut().unwrap();
        g.last = snap.time;
        match win_prob(&Input { model: &model, snap: &snap, sd: &sd, curves: &curves, history: &g.history.samples }) {
            Some(w) => {
                g.line.push((snap.time, w.p));
                g.history.push(GoldSample { time: snap.time, team: 0, lane: None, win: Some(w.base) });
            }
            None => no_table += 1,
        }
        let my_team = snap.players[snap.me].team;
        let nexus = |team: Team| snap.events.iter().filter(|e| matches!(e, GameEvent::Tower { team: t, tier: 4, .. } if *t == team)).count();
        if let Some(win) = snap.events.iter().find_map(|e| if let GameEvent::GameEnd { win, .. } = e { Some(*win) } else { None }) {
            (g.win, g.sure) = (Some(win), true);
        } else if !g.sure {
            g.win = [Team::Order, Team::Chaos].into_iter().find(|t| nexus(*t) >= 2).map(|lost| lost != my_team);
        }
    }

    println!("\n{:<11} {:<9} {:<14} {:>5} {:>5} {:>5} {:>5} {:>5}  result", "game", "mode", "champion", "min", "p@10", "p@15", "p@20", "end");
    let pct = |p: Option<f32>| p.map_or("-".to_string(), |p| format!("{:.0}", p * 100.0));
    for g in &games {
        let at = |min: f32| g.line.iter().find(|(t, _)| *t >= min * 60.0).map(|x| x.1);
        let mode = if g.swiftplay { "Swiftplay".to_string() } else { format!("{:?}", g.mode) };
        let result = match (g.win, g.sure) {
            (Some(w), true) => if w { "win" } else { "loss" },
            (Some(w), false) => if w { "win~" } else { "loss~" },
            _ => "",
        };
        let [a, b, c] = AT_MIN.map(|m| pct(at(m)));
        println!("{:<11} {:<9} {:<14} {:>5.1} {a:>5} {b:>5} {c:>5} {:>5}  {result}", g.started, mode, sd.champ_name(g.champ), g.last / 60.0, pct(g.line.last().map(|x| x.1)));
    }

    let known: Vec<&Game> = games.iter().filter(|g| g.win.is_some() && !g.line.is_empty()).collect();
    let mut bins = [(0usize, 0.0f32, 0usize); BINS];
    for g in &known {
        for &(_, p) in &g.line {
            let b = &mut bins[((p * BINS as f32) as usize).min(BINS - 1)];
            (b.0, b.1, b.2) = (b.0 + 1, b.1 + p, b.2 + usize::from(g.win == Some(true)));
        }
    }
    let ended = games.iter().filter(|g| g.sure).count();
    println!("\n{} games ({} recordings unparsed, {} without a table); result known for {} ({} from GameEnd, the rest (~) from a side losing both nexus turrets)", games.len(), unparsed, no_table, known.len(), ended);
    println!("reliability over their {} snapshots:\n{:>9} {:>6} {:>7} {:>6}", bins.iter().map(|b| b.0).sum::<usize>(), "p", "n", "mean p", "won");
    for (i, (n, sum, won)) in bins.iter().enumerate().filter(|(_, b)| b.0 > 0) {
        println!("{:>4}–{:<4} {n:>6} {:>6.0}% {:>5.0}%", i * 100 / BINS, (i + 1) * 100 / BINS, 100.0 * sum / *n as f32, 100.0 * *won as f32 / *n as f32);
    }
    let right = known.iter().filter(|g| g.line.last().is_some_and(|x| (x.1 > 0.5) == (g.win == Some(true)))).count();
    println!("last snapshot on the winner's side: {right} of {}", known.len());
    Ok(())
}
