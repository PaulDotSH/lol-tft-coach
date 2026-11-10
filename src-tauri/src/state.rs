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

/// Coachless build for my champion: lane, lane opponent, my first two completed items.
pub type CoachlessKey = (ChampKey, Lane, Option<ChampKey>, Option<ItemId>, Option<ItemId>);

/// Set by the orchestrator each tick while an early-game Rift match with gank warnings runs;
/// None parks the minimap thread.
#[derive(Clone)]
pub struct GankCtx {
    /// Game time at `at`; the thread counts on from it.
    pub time: f32,
    pub at: Instant,
    pub blue: bool,
    pub lane: Option<Lane>,
    pub enemies: Arc<coach::GankEnemies>,
}

/// League's minimap settings are re-read at most this often.
const MINIMAP_CFG_EVERY: Duration = Duration::from_secs(30);

pub struct Shared {
    pub http: data::Http,
    pub ui: Mutex<AppState>,
    pub sd: RwLock<Option<Arc<StaticData>>>,
    pub mayhem: Cache<(), MayhemData>,
    pub stats: Cache<StatsKey, ChampStats>,
    /// Game-plan card: win rate by game length per champion (build page) and the lane it is for.
    pub lengths: Cache<StatsKey, coach::Curve>,
    pub tiers: Cache<Lane, Vec<TierEntry>>,
    pub augments: Cache<ChampKey, ChampAugments>,
    /// Augment pool names for the OCR thread; None = OCR idle.
    pub ocr_names: Mutex<Option<Arc<HashMap<AugmentId, String>>>>,
    /// Augment names in the League client's language (LCU), added to `ocr_names` when they
    /// differ from the English ones.
    pub local_augment_names: Mutex<Option<Arc<HashMap<AugmentId, String>>>>,
    /// Last OCR offer (its cards in screen order) and when it was seen.
    pub offered: Mutex<(Vec<ocr::offer::Card>, Instant)>,
    /// A Mayhem offer should be on screen (an augment round reached, no pick since): the OCR thread
    /// may read the wider band when the strip shows nothing.
    pub offer_expected: Mutex<bool>,
    pub ocr_state: Mutex<SourceState>,
    /// Last lolalytics / u.gg fetch outcome.
    pub lola_ok: Mutex<Option<bool>>,
    pub ugg_ok: Mutex<Option<bool>>,
    /// Draft: counter-only stats per (champ, lane) and build-team synergy.
    pub lane_stats: Cache<(ChampKey, Option<Lane>), ChampStats>,
    pub team: Cache<(ChampKey, Lane), TeamSynergy>,
    /// Matchup card: my counters against an opposing lane, and matchup pages.
    pub vs_counters: Cache<(ChampKey, Lane, Lane), Vec<Matchup>>,
    pub matchups: Cache<(ChampKey, ChampKey, Lane, Lane), MatchupBuild>,
    /// ARAM tier list (all champions) for the ARAM / Mayhem bench.
    pub aram_tiers: Cache<(), Vec<TierEntry>>,
    /// Data Dragon R cooldowns per champion, and the last fetch outcome.
    pub ults: Cache<ChampKey, UltCooldowns>,
    /// Data Dragon Q W E R per champion (matchup cards).
    pub spells: Cache<ChampKey, Vec<SpellInfo>>,
    pub ddragon_ok: Mutex<Option<bool>>,
    /// Coachless sign-in, my champion's Coachless builds and the last fetch outcome.
    pub coachless_auth: data::coachless::Auth,
    pub coachless: Cache<CoachlessKey, ChampStats>,
    /// My Coachless rune page against my lane opponent: (me, lane, them); None = too few games.
    pub coachless_runes: Cache<(ChampKey, Lane, ChampKey), Option<RuneData>>,
    pub coachless_ok: Mutex<Option<bool>>,
    /// Mayhem Tab scoreboard reader.
    pub tab: crate::tab::TabShared,
    /// Minimap thread: what to watch for, its gank warnings and when they were made, its state.
    pub gank_ctx: Mutex<Option<GankCtx>>,
    pub gank_calls: Mutex<(Vec<Call>, Instant)>,
    pub sights: Mutex<coach::SightLog>,
    pub minimap_state: Mutex<SourceState>,
    /// Enemy champion portraits for naming minimap blips.
    pub portraits: Cache<ChampKey, ocr::minimap::Portrait>,
    /// League install folder (from the client), and the minimap settings read from it.
    pub league_install: Mutex<Option<std::path::PathBuf>>,
    minimap_cfg: Mutex<Option<(clients::league_cfg::MinimapSettings, Instant)>>,
    /// Where the screen readers found their elements (`calib.json`) and how they are doing.
    pub calib: Mutex<crate::calib::Calib>,
    pub health: Mutex<crate::health::Health>,
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
                    minimap: SourceState::Off,
                    coachless: SourceState::Off,
                    coachless_premium: false,
                    message: None,
                },
                champ_select: None,
                game: None,
                recap: None,
                settings,
                macro_report: None,
                trends: None,
                goal_progress: vec![],
                history: vec![],
                ranked: vec![],
                readers: vec![],
                plan: None,
            }),
            sd: RwLock::new(None),
            mayhem: Mutex::default(),
            stats: Mutex::default(),
            lengths: Mutex::default(),
            tiers: Mutex::default(),
            augments: Mutex::default(),
            ocr_names: Mutex::new(None),
            local_augment_names: Mutex::new(None),
            offered: Mutex::new((vec![], Instant::now())),
            offer_expected: Mutex::new(false),
            ocr_state: Mutex::new(SourceState::Off),
            lola_ok: Mutex::new(None),
            ugg_ok: Mutex::new(None),
            lane_stats: Mutex::default(),
            team: Mutex::default(),
            vs_counters: Mutex::default(),
            matchups: Mutex::default(),
            aram_tiers: Mutex::default(),
            ults: Mutex::default(),
            spells: Mutex::default(),
            ddragon_ok: Mutex::new(None),
            coachless_auth: data::coachless::Auth::load(crate::settings::local_dir("").join("coachless.json")),
            coachless: Mutex::default(),
            coachless_runes: Mutex::default(),
            coachless_ok: Mutex::new(None),
            tab: Default::default(),
            gank_ctx: Mutex::new(None),
            gank_calls: Mutex::new((vec![], Instant::now())),
            sights: Mutex::default(),
            minimap_state: Mutex::new(SourceState::Off),
            portraits: Mutex::default(),
            league_install: Mutex::new(None),
            minimap_cfg: Mutex::new(None),
            calib: Mutex::new(crate::calib::Calib::load(&crate::settings::local_dir("").join("calib.json"))),
            health: Mutex::default(),
        }
    }

    /// Calibration key of a `w`×`h` League window under the player's HUD scale.
    pub fn calib_key(&self, w: u32, h: u32) -> String {
        crate::calib::key(w, h, self.minimap_settings().hud_scale)
    }

    /// Unix ms now, for reader health.
    pub fn now_ms() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
    }

    /// League's minimap scale and flip, from the install folder when known; cached.
    pub fn minimap_settings(&self) -> clients::league_cfg::MinimapSettings {
        let mut c = self.minimap_cfg.lock().unwrap();
        if let Some((s, at)) = *c
            && at.elapsed() < MINIMAP_CFG_EVERY
        {
            return s;
        }
        let dir = self.league_install.lock().unwrap().clone();
        let s = clients::league_cfg::minimap_settings(dir.as_deref());
        *c = Some((s, Instant::now()));
        s
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
        self.lengths.lock().unwrap().clear();
        self.tiers.lock().unwrap().clear();
        self.lane_stats.lock().unwrap().clear();
        self.aram_tiers.lock().unwrap().clear();
        self.team.lock().unwrap().clear();
        self.vs_counters.lock().unwrap().clear();
        self.matchups.lock().unwrap().clear();
        self.coachless.lock().unwrap().clear();
        self.coachless_runes.lock().unwrap().clear();
        *self.coachless_ok.lock().unwrap() = None;
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
