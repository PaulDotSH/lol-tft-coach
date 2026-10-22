//! Shared app state and the background fetch cache.
//!
//! Remote data is fetched in spawned tasks and stored in `Slot`s, so the
//! orchestrator tick never waits on the network: it uses whatever is ready.

use model::*;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const RETRY_AFTER: Duration = Duration::from_secs(60);

pub enum Slot<T> {
    Pending,
    Ready(Arc<T>),
    Failed(Instant),
}

pub type Cache<K, T> = Mutex<HashMap<K, Slot<T>>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatsKey {
    pub champ: ChampKey,
    pub lane: Option<Lane>,
    pub mode: Mode,
}

pub struct Shared {
    pub http: data::Http,
    pub ui: Mutex<AppState>,
    pub sd: RwLock<Option<Arc<StaticData>>>,
    pub mayhem: Cache<(), MayhemData>,
    pub stats: Cache<StatsKey, ChampStats>,
    pub tiers: Cache<Lane, Vec<TierEntry>>,
    pub augments: Cache<ChampKey, ChampAugments>,
    /// Augment pool names for the OCR thread; None = OCR idle.
    pub ocr_names: Mutex<Option<Arc<HashMap<AugmentId, String>>>>,
    /// Last OCR offer and when it was seen.
    pub offered: Mutex<(Vec<AugmentId>, Instant)>,
    pub ocr_state: Mutex<SourceState>,
    /// Last lolalytics / u.gg fetch outcome.
    pub lola_ok: Mutex<Option<bool>>,
    pub ugg_ok: Mutex<Option<bool>>,
    /// Draft: counter-only stats per (champ, lane) and build-team synergy.
    pub lane_stats: Cache<(ChampKey, Option<Lane>), ChampStats>,
    pub team: Cache<(ChampKey, Lane), TeamSynergy>,
}

impl Shared {
    pub fn new(http: data::Http, settings: Settings) -> Shared {
        Shared {
            http,
            ui: Mutex::new(AppState {
                phase: Phase::NoClient,
                sources: Sources {
                    client: SourceState::Off,
                    game: SourceState::Off,
                    lolalytics: SourceState::Off,
                    ugg: SourceState::Off,
                    ocr: SourceState::Off,
                    message: None,
                },
                champ_select: None,
                game: None,
                recap: None,
                settings,
                macro_report: None,
            }),
            sd: RwLock::new(None),
            mayhem: Mutex::default(),
            stats: Mutex::default(),
            tiers: Mutex::default(),
            augments: Mutex::default(),
            ocr_names: Mutex::new(None),
            offered: Mutex::new((vec![], Instant::now())),
            ocr_state: Mutex::new(SourceState::Off),
            lola_ok: Mutex::new(None),
            ugg_ok: Mutex::new(None),
            lane_stats: Mutex::default(),
            team: Mutex::default(),
        }
    }

    pub fn settings(&self) -> Settings {
        self.ui.lock().unwrap().settings.clone()
    }

    pub fn static_data(&self) -> Option<Arc<StaticData>> {
        self.sd.read().unwrap().clone()
    }

    /// Patch for lolalytics ("16.21") from the Data Dragon version.
    pub fn patch(&self) -> Option<String> {
        let sd = self.static_data()?;
        let mut parts = sd.version.split('.');
        Some(format!("{}.{}", parts.next()?, parts.next()?))
    }

    pub fn clear_stats(&self) {
        self.stats.lock().unwrap().clear();
        self.tiers.lock().unwrap().clear();
        self.lane_stats.lock().unwrap().clear();
        self.team.lock().unwrap().clear();
    }
}

/// Return the cached value, or start fetching it in the background.
pub fn ensure<K, T, F>(
    sh: &Arc<Shared>,
    cache: fn(&Shared) -> &Cache<K, T>,
    ok_flag: fn(&Shared) -> &Mutex<Option<bool>>,
    key: K,
    fetch: impl FnOnce() -> F,
) -> Option<Arc<T>>
where
    K: Copy + Eq + Hash + Send + 'static,
    T: Send + Sync + 'static,
    F: Future<Output = anyhow::Result<T>> + Send + 'static,
{
    {
        let mut map = cache(sh).lock().unwrap();
        match map.get(&key) {
            Some(Slot::Ready(v)) => return Some(v.clone()),
            Some(Slot::Pending) => return None,
            Some(Slot::Failed(at)) if at.elapsed() < RETRY_AFTER => return None,
            _ => {}
        }
        map.insert(key, Slot::Pending);
    }
    let fut = fetch();
    let sh = sh.clone();
    tokio::spawn(async move {
        let res = fut.await;
        *ok_flag(&sh).lock().unwrap() = Some(res.is_ok());
        let slot = match res {
            Ok(v) => Slot::Ready(Arc::new(v)),
            Err(e) => {
                tracing::warn!("fetch failed: {e:#}");
                Slot::Failed(Instant::now())
            }
        };
        cache(&sh).lock().unwrap().insert(key, slot);
    });
    None
}
