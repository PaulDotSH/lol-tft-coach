//! Macro coach plumbing: syncs my match history in a background task (at start
//! and after each game, never inside the tick) and turns it into a `MacroReport`.

use crate::state::Shared;
use crate::{orchestrator, settings};
use clients::lcu::Lcu;
use model::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A finished game can take a while to reach the history list: retry this often, this many times.
const RETRY_EVERY: Duration = Duration::from_secs(30);
const MAX_TRIES: u32 = 10;
/// Most played champions per mode that get lolalytics benchmarks, and their fewest games.
const BENCH_CHAMPS: usize = 3;
const BENCH_GAMES: u32 = 3;

#[derive(Default)]
pub struct MacroSync(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    games: Option<Arc<Vec<PastGame>>>,
    running: bool,
    last_try: Option<Instant>,
    /// Waiting for the game that just ended to show up.
    want_new: bool,
    tries: u32,
    prev_phase: Option<Phase>,
    report: Option<MacroReport>,
}

fn newest(games: &Option<Arc<Vec<PastGame>>>) -> Option<u64> {
    games.as_ref()?.first().map(|g| g.game_id)
}

impl MacroSync {
    /// Every tick: starts a sync when one is due (Idle / PostGame only) and
    /// returns the latest report, recomputed while idle.
    pub fn tick(self: &Arc<Self>, sh: &Arc<Shared>, sd: &StaticData, lcu: Option<&Lcu>, phase: Phase) -> Option<MacroReport> {
        let mut st = self.0.lock().unwrap();
        if phase == Phase::PostGame && st.prev_phase != Some(Phase::PostGame) {
            st.want_new = true;
            st.tries = 0;
        }
        st.prev_phase = Some(phase);
        let quiet = matches!(phase, Phase::Idle | Phase::PostGame);
        let due = st.games.is_none() || st.want_new;
        if quiet
            && due
            && !st.running
            && st.last_try.is_none_or(|t| t.elapsed() > RETRY_EVERY)
            && let Some(lcu) = lcu
        {
            st.running = true;
            st.last_try = Some(Instant::now());
            let (me, lcu) = (self.clone(), lcu.clone());
            tokio::spawn(async move {
                let res = clients::history::sync(&lcu, &settings::local_dir("history")).await;
                let mut st = me.0.lock().unwrap();
                st.running = false;
                match res {
                    Ok(games) => {
                        let games = Some(Arc::new(games));
                        st.tries += 1;
                        if newest(&games) != newest(&st.games) || st.tries >= MAX_TRIES {
                            st.want_new = false;
                        }
                        st.games = games;
                    }
                    Err(e) => tracing::warn!("match history: {e:#}"),
                }
            });
        }
        if quiet && let Some(games) = st.games.clone() {
            st.report = Some(report(sh, sd, &games));
        }
        st.report.clone()
    }
}

/// Report with lolalytics benchmarks for my most played champions (fetched in
/// the background on first use, so they join a later report).
fn report(sh: &Arc<Shared>, sd: &StaticData, games: &[PastGame]) -> MacroReport {
    let plain = coach::macro_report(games, sd, &HashMap::new());
    let mut bench = HashMap::new();
    for mode in [Mode::Rift, Mode::Aram, Mode::Mayhem] {
        for c in plain.champs.iter().filter(|c| c.mode == mode && c.games >= BENCH_GAMES).take(BENCH_CHAMPS) {
            let mut lanes: HashMap<Lane, u32> = HashMap::new();
            for l in games.iter().filter(|g| g.mode == mode && g.champ == c.champ).filter_map(|g| g.lane) {
                *lanes.entry(l).or_default() += 1;
            }
            let lane = lanes.into_iter().max_by_key(|&(l, n)| (n, l as u8)).map(|(l, _)| l);
            if let Some(s) = orchestrator::stats(sh, sd, c.champ, lane, mode) {
                bench.insert((mode, c.champ), s.benchmarks.clone());
            }
        }
    }
    if bench.is_empty() { plain } else { coach::macro_report(games, sd, &bench) }
}
