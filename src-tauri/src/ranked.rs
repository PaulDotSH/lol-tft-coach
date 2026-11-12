//! Ranked LP log: reads my current rank while idle (once a minute, off the
//! tick) and appends a snapshot whenever it changed, so each ranked game
//! leaves a point. Saved in the app's local dir; the client keeps no LP history.

use crate::account;
use clients::lcu::Lcu;
use model::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EVERY: Duration = Duration::from_secs(60);
/// Snapshots kept per queue.
const KEEP: usize = 200;

#[derive(Default)]
pub struct RankLog(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    snaps: Option<Vec<RankSnapshot>>,
    running: bool,
    last_try: Option<Instant>,
    trends: Vec<RankTrend>,
}

fn path() -> std::path::PathBuf {
    account::dir("ranked").join("snapshots.json")
}

fn same(a: &RankSnapshot, b: &RankSnapshot) -> bool {
    (&a.tier, &a.division, a.lp, a.wins, a.losses) == (&b.tier, &b.division, b.lp, b.wins, b.losses)
}

impl RankLog {
    /// Every tick: a read is due while idle; returns the trends from the log.
    pub fn tick(self: &Arc<Self>, lcu: Option<&Lcu>, phase: Phase) -> Vec<RankTrend> {
        let mut st = self.0.lock().unwrap();
        if st.snaps.is_none() {
            let snaps: Vec<RankSnapshot> = std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            st.trends = coach::rank_trends(&snaps);
            st.snaps = Some(snaps);
        }
        if matches!(phase, Phase::Idle | Phase::PostGame)
            && !st.running
            && st.last_try.is_none_or(|t| t.elapsed() > EVERY)
            && let Some(lcu) = lcu
        {
            st.running = true;
            st.last_try = Some(Instant::now());
            let (me, lcu) = (self.clone(), lcu.clone());
            tokio::spawn(async move {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
                let res = lcu.ranked(now).await;
                let mut st = me.0.lock().unwrap();
                st.running = false;
                let fresh = match res {
                    Ok(v) => v,
                    Err(e) => return tracing::warn!("ranked: {e:#}"),
                };
                let snaps = st.snaps.get_or_insert_with(Vec::new);
                let mut changed = false;
                for s in fresh {
                    if !snaps.iter().rev().find(|x| x.queue == s.queue).is_some_and(|x| same(x, &s)) {
                        tracing::info!("ranked {}: {} {} {} LP ({}W {}L)", s.queue, s.tier, s.division, s.lp, s.wins, s.losses);
                        snaps.push(s);
                        changed = true;
                    }
                }
                if changed {
                    for q in ["solo", "flex"] {
                        let n = snaps.iter().filter(|s| s.queue == q).count();
                        let mut extra = n.saturating_sub(KEEP);
                        snaps.retain(|s| s.queue != q || extra == 0 || {
                            extra -= 1;
                            false
                        });
                    }
                    let _ = std::fs::write(path(), serde_json::to_string(&*snaps).unwrap_or_default());
                    st.trends = coach::rank_trends(st.snaps.as_deref().unwrap_or_default());
                }
            });
        }
        st.trends.clone()
    }
}
